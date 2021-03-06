use crate::api::director::TargetFormat;
use crate::command::{CommandResult, TableResult};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::http::{Http, HttpMethods};
use clap::ArgMatches;
use comfy_table::Table;
use reqwest::blocking::multipart::Form;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::{collections::HashMap, fs, path::Path};
use toml;
use url::Url;
use urlencoding;

#[derive(Deserialize)]
struct TargetRole {
    signed: Targets,
}

#[derive(Deserialize)]
struct Targets {
    targets: HashMap<String, Target>,
}

#[derive(Deserialize)]
struct Target {
    custom: Custom,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Custom {
    name: String,
    version: String,
    hardware_ids: Vec<String>,
    uri: Option<Url>,
    updated_at: String,
    target_format: TargetFormat,
}

/// Available TUF Reposerver API methods.
pub trait ReposerverApi {
    fn add_package(_: &mut Config, package: TufPackage) -> Result<CommandResult>;
    fn get_package(_: &mut Config, name: &str, version: &str) -> Result<CommandResult>;
    fn list_packages(_: &mut Config) -> Result<CommandResult>;
}

/// Make API calls to the TUF Reposerver.
pub struct Reposerver;

impl ReposerverApi for Reposerver {
    fn add_package(config: &mut Config, package: TufPackage) -> Result<CommandResult> {
        let entry = format!("{}-{}", package.name, package.version);
        debug!("adding package with entry name {}", entry);
        let req = Client::new()
            .put(&format!("{}api/v1/user_repo/targets/{}", config.reposerver, entry))
            .query(&[
                ("name", urlencoding::encode(&package.name)),
                ("version", urlencoding::encode(&package.version)),
                ("hardwareIds", package.hardware.join(",")),
                ("targetFormat", format!("{}", package.format)),
            ])
            .multipart(match package.target {
                RepoTarget::Path(path) => Form::new().file("file", path)?,
                RepoTarget::Url(url) => Form::new().file("fileUri", url.as_str())?,
            });
        Ok(Http::send(req, config.token()?)?.into())
    }

    fn get_package(config: &mut Config, name: &str, version: &str) -> Result<CommandResult> {
        let entry = format!("{}_{}", name, version);
        debug!("fetching package with entry name {}", entry);
        Ok(Http::get(&format!("{}api/v1/user_repo/targets/{}", config.reposerver, entry), config.token()?)?.into())
    }

    fn list_packages(config: &mut Config) -> Result<CommandResult> {
        let mut res = Http::get(&format!("{}api/v1/user_repo/targets.json", config.reposerver), config.token()?)?;
        let h = res.headers().to_owned();
        let mut str_resp: Vec<u8> = vec![];
        res.read_to_end(&mut str_resp)?;

        let v: TargetRole = serde_json::from_slice(&str_resp)?;

        let mut table = Table::new();

        table.set_header(vec![
            "name",
            "name",
            "version",
            "hardware ids",
            "uri",
            "target_format",
            "updated at",
        ]);

        for (k, v) in v.signed.targets {
            let hwids = format!("{}", v.custom.hardware_ids.join(", "));
            let uri = v.custom.uri.map(|u| u.to_string()).unwrap_or("None".to_owned());
            let target_format = format!("{:?}", v.custom.target_format);
            table.add_row(vec![
                k,
                v.custom.name,
                v.custom.version,
                hwids,
                uri,
                target_format,
                v.custom.updated_at,
            ]);
        }

        Ok(TableResult::new(h, str_resp, table).into())
    }
}

impl Reposerver {
    /// Upload multiple packages (without batching), returning the final response.
    pub fn add_packages(config: &mut Config, packages: TufPackages) -> Result<CommandResult> {
        let mut responses = packages
            .packages
            .into_iter()
            .map(|package| Self::add_package(config, package))
            .collect::<Result<Vec<_>>>()?;
        let last = responses.len() - 1;
        Ok(responses.remove(last))
    }
}

/// Parsed TOML package metadata.
#[derive(Serialize, Deserialize)]
pub struct PackageMetadata {
    format: TargetFormat,
    hardware: Vec<String>,
    path: Option<String>,
    url: Option<String>,
}

/// A parsed mapping from package names to versions to metadata.
#[derive(Serialize, Deserialize)]
pub struct TargetPackages {
    pub packages: HashMap<String, HashMap<String, PackageMetadata>>,
}

impl TargetPackages {
    /// Parse a toml file into `TargetPackages`.
    pub fn from_file(input: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            packages: toml::from_str(&fs::read_to_string(input)?)?,
        })
    }
}

/// A package target for uploading to the TUF Reposerver.
#[derive(Serialize, Deserialize)]
pub struct TufPackage {
    name: String,
    version: String,
    format: TargetFormat,
    hardware: Vec<String>,
    target: RepoTarget,
}

impl<'a> TufPackage {
    /// Parse CLI arguments into a `TufPackage`.
    pub fn from_args(args: &ArgMatches<'a>) -> Result<Self> {
        Ok(TufPackage {
            name: args.value_of("name").expect("--name").into(),
            version: args.value_of("version").expect("--version").into(),
            format: TargetFormat::from_args(&args)?,
            hardware: args.values_of("hardware").expect("--hardware").map(String::from).collect(),
            target: RepoTarget::from_args(&args)?,
        })
    }
}

/// A collection of TUF packages for uploading.
#[derive(Serialize, Deserialize)]
pub struct TufPackages {
    pub packages: Vec<TufPackage>,
}

impl TufPackages {
    /// Convert `TargetPackages` to `TufPackages`.
    pub fn from(targets: TargetPackages) -> Result<Self> {
        Ok(Self {
            packages: targets
                .packages
                .into_iter()
                .map(|(name, versions)| Self::to_packages(name, versions))
                .collect::<Result<Vec<Vec<_>>>>()?
                .into_iter()
                .flat_map(|packages| packages.into_iter())
                .collect(),
        })
    }

    fn to_packages(name: String, versions: HashMap<String, PackageMetadata>) -> Result<Vec<TufPackage>> {
        Ok(versions
            .into_iter()
            .map(|(version, meta)| Self::to_package(name.clone(), version, meta))
            .collect::<Result<Vec<_>>>()?)
    }

    fn to_package(name: String, version: String, meta: PackageMetadata) -> Result<TufPackage> {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        Ok(TufPackage {
            name,
            version,
            format:   meta.format,
            hardware: meta.hardware,
            target: match (meta.path, meta.url) {
                (Some(path), None) => RepoTarget::Path(path),
                (None, Some(url))  => RepoTarget::Url(url.parse()?),
                (None, None)       => Err(Error::Parse("One of `path` or `url` required.".into()))?,
                (Some(_), Some(_)) => Err(Error::Parse("Either `path` or `url` expected. Not both.".into()))?,
            },
        })
    }
}

/// Target data pointed to by either filesystem path or remote URL.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum RepoTarget {
    Path(String),
    Url(Url),
}

impl<'a> RepoTarget {
    pub fn from_args(args: &ArgMatches<'a>) -> Result<Self> {
        if let Some(path) = args.value_of("path") {
            Ok(RepoTarget::Path(path.into()))
        } else if let Some(url) = args.value_of("url") {
            Ok(RepoTarget::Url(url.parse()?))
        } else {
            Err(Error::Args("Either --path or --url flag is required".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_example_packages() {
        let targets = TargetPackages::from_file("examples/packages.toml").expect("parse toml");
        let mut packages = TufPackages::from(targets).expect("convert package").packages;
        assert_eq!(packages.len(), 2);
        packages.sort_unstable_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(packages[0].name, "foo".to_string());
        assert_eq!(packages[0].version, "1".to_string());
        assert_eq!(packages[0].hardware, vec!["acme-ecu-1".to_string()]);
        assert_eq!(
            packages[0].target,
            RepoTarget::Url("https://acme.org/downloads/foo".parse().expect("parse url"))
        );
        assert_eq!(packages[0].format, TargetFormat::Binary);

        assert_eq!(packages[1].name, "my-branch".to_string());
        assert_eq!(packages[1].version, "1234".to_string());
        assert_eq!(packages[1].hardware, vec!["qemux86-64".to_string()]);
        assert_eq!(packages[1].target, RepoTarget::Path("/ota/my-branch-01234".into()));
        assert_eq!(packages[1].format, TargetFormat::Ostree);
    }
}
