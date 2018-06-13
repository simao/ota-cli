use reqwest;
use serde_json;
use std::{self,
          fmt::{self, Debug, Display, Formatter}};
use toml;
use url;
use uuid;
use zip;

pub enum Error {
    Action(String),
    Checksum(String),
    Http(reqwest::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
    Toml(toml::de::Error),
    Url(url::ParseError),
    Uuid(uuid::ParseError),
    Zip(zip::result::ZipError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Error::Action(ref err) => format!("Unknown campaign action: {}", err),
                Error::Checksum(ref err) => format!("Unknown checksum method: {}", err),
                Error::Http(ref err) => format!("HTTP: {}", err),
                Error::Io(ref err) => format!("I/O: {}", err),
                Error::Json(ref err) => format!("JSON parsing: {}", err),
                Error::Toml(ref err) => format!("TOML parsing: {}", err),
                Error::Url(ref err) => format!("URL parsing: {}", err),
                Error::Uuid(ref err) => format!("UUID parsing: {}", err),
                Error::Zip(ref err) => format!("Zip/Unzip: {}", err),
            }
        )
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        "ota-cli error"
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        Error::Http(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Error {
        Error::Json(err)
    }
}

impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Error {
        Error::Toml(err)
    }
}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Error {
        Error::Url(err)
    }
}

impl From<uuid::ParseError> for Error {
    fn from(err: uuid::ParseError) -> Error {
        Error::Uuid(err)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Error {
        Error::Zip(err)
    }
}
