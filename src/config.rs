use clap::ArgMatches;
use dirs;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, ErrorKind, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
};
use url::Url;
use zip::ZipArchive;

use crate::api::auth_plus::{AccessToken, AuthPlus, AuthPlusApi, Credentials};
use crate::error::{Error, Result};

const CONFIG_FILE: &str = ".ota.conf";

/// Config values passed to API methods for making HTTP requests.
#[derive(Serialize, Deserialize)]
pub struct Config {
    pub credentials_zip: PathBuf,
    #[serde(skip)]
    pub credentials: Option<Credentials>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<AccessToken>,

    pub campaigner: Url,
    pub director: Url,
    pub registry: Url,
    pub reposerver: Url,
}

impl<'a> Config {
    /// Initialize a new config from CLI arguments.
    pub fn init_from_args(args: &ArgMatches<'a>) -> Result<()> {
        let credentials: PathBuf = args.value_of("credentials").expect("--credentials").into();
        let campaigner = args.value_of("campaigner").expect("--campaigner").parse()?;
        let director = args.value_of("director").expect("--director").parse()?;
        let registry = args.value_of("registry").expect("--registry").parse()?;

        let reposerver = match args.value_of("reposerver") {
            Some(s) => s.parse()?,
            None => Self::reposerver_url(&credentials)?,
        };

        Self::init(credentials, campaigner, director, registry, reposerver)
    }

    /// Initialize a new config file.
    pub fn init(credentials_zip: PathBuf, campaigner: Url, director: Url, registry: Url, reposerver: Url) -> Result<()> {
        Config {
            credentials_zip,
            credentials: None,
            token: None,
            campaigner,
            director,
            registry,
            reposerver,
        }
        .save_default()
    }

    /// Save the default config file.
    pub fn save_default(&self) -> Result<()> {
        self.save(Self::default_path())
    }

    /// Load the default config file.
    pub fn load_default() -> Result<Self> {
        Self::load(Self::default_path())
    }

    /// Save the current config.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let mut file = OpenOptions::new().create(true).write(true).truncate(true).open(path)?;
        Ok(file.write_all(&serde_json::to_vec_pretty(&self)?)?)
    }

    /// Load a previously saved config.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        fs::read(path)
            .or_else(|err| match err.kind() {
                ErrorKind::NotFound => Err(Error::NotFound("Config file".into(), Some("Please run `ota init` first.".into()))),
                _ => Err(err.into()),
            })
            .and_then(|file| Ok(serde_json::from_slice(&file)?))
    }

    /// Parse `Credentials` or return an existing reference.
    pub fn credentials(&mut self) -> Result<&Credentials> {
        if let None = self.credentials {
            self.credentials = Some(Credentials::parse(&self.credentials_zip)?);
        }
        Ok(self.credentials.as_ref().unwrap())
    }

    /// Refresh an `AccessToken` or return existing.
    pub fn token(&mut self) -> Result<Option<AccessToken>> {
        match self.token {
            Some(_) => debug!("using cached access token..."),
            None => {
                if let Some(token) = AuthPlus::refresh_token(self)? {
                    self.token = Some(token);
                    self.save_default()?;
                }
            }
        }
        Ok(self.token.clone())
    }

    /// Return the default config path.
    fn default_path() -> PathBuf {
        let mut path = PathBuf::new();
        path.push(dirs::home_dir().expect("couldn't read home directory path"));
        path.push(CONFIG_FILE);
        path
    }

    /// Parse credentials.zip and return the TUF Reposerver URL.
    fn reposerver_url(credentials_zip: impl AsRef<Path>) -> Result<Url> {
        debug!("reading tufrepo.url from credentials.zip");
        let file = File::open(credentials_zip)?;
        let mut archive = ZipArchive::new(BufReader::new(file))?;
        let mut tufrepo = archive.by_name("tufrepo.url")?;
        let mut contents = String::new();
        let _ = tufrepo.read_to_string(&mut contents)?;
        Ok(Url::from_str(&contents)?)
    }
}
