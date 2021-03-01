use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::Url;
use serde_json::{self, Value};
use std::io::{self, Read, Write};

use crate::api::auth_plus::AccessToken;
use crate::error::{Error, Result};
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use reqwest::header::HeaderMap;

/// Convenience methods for making simple HTTP requests.
pub trait HttpMethods {
    fn get(url: impl AsRef<str>, token: Option<AccessToken>) -> Result<Response> {
        Http::send(Client::new().get(Url::parse(url.as_ref())?), token)
    }
    fn post(url: impl AsRef<str>, token: Option<AccessToken>) -> Result<Response> {
        Http::send(Client::new().post(Url::parse(url.as_ref())?), token)
    }
    fn put(url: impl AsRef<str>, token: Option<AccessToken>) -> Result<Response> {
        Http::send(Client::new().put(Url::parse(url.as_ref())?), token)
    }
    fn delete(url: impl AsRef<str>, token: Option<AccessToken>) -> Result<Response> {
        Http::send(Client::new().delete(Url::parse(url.as_ref())?), token)
    }
}

pub struct PrintableH {
    inner: Box<dyn PrintableResponse>,
}

impl From<Response> for PrintableH {
    fn from(r: Response) -> Self {
        PrintableH { inner: Box::new(r) }
    }
}

impl From<TableResponse> for PrintableH {
    fn from(r: TableResponse) -> Self {
        PrintableH { inner: Box::new(r) }
    }
}

pub struct TableResponse {
    pub headers: HeaderMap,
    pub str: String
}

impl PrintableResponse for TableResponse {
    fn headers(&mut self) -> HashMap<String, String> {
        HashMap::new() // TODO: Wrong
    }

    fn read(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        Ok(buf.write(self.str.as_bytes())?)
    }
}

trait PrintableResponse {
    fn headers(&mut self) -> HashMap<String, String>;

    fn read(&mut self, buf: &mut Vec<u8>) -> Result<usize>;
}

impl PrintableH {
    fn headers(&mut self) -> HashMap<String, String> {
        self.inner.headers()
    }

    fn read(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        self.inner.read(buf)
    }
}

impl PrintableResponse for Response {
    fn headers(&mut self) -> HashMap<String, String> {
        let mut res: HashMap<String, String> = HashMap::new();

        for (k, v) in Response::headers(self).iter() {
            res.insert(k.to_string(), v.to_str().unwrap_or("").to_owned());
        }

        res
    }

    fn read(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        Ok(self.read_to_end(buf)?)
    }
}

/// Make HTTP requests to server endpoints.
pub struct Http;

impl HttpMethods for Http {}

impl Http {
    /// Send an HTTP request with an optional bearer token.
    pub fn send(mut builder: RequestBuilder, token: Option<AccessToken>) -> Result<Response> {
        if let Some(token) = token {
            debug!("request with token scopes: {:?}", token);
            builder = builder.bearer_auth(token.access_token.clone());

            match token.namespace() {
                Ok(name) => builder = builder.header("x-ats-namespace", name),
                Err(err) => {
                    error!("reading token namespace: {}", err)
                }
            }
        }

        let req = builder.build()?;
        if req.headers().len() > 0 {
            info!("request headers:\n{:#?}", req.headers());
        }
        if let Some(body) = req.body() {
            info!("request body:\n{:#?}\n", body);
        }

        Client::new().execute(req).map_err(Error::Http)
    }

    /// Print the HTTP response to stdout.
    pub fn print_response(mut resp: PrintableH) -> Result<()> {
        let mut body = Vec::new();
        debug!("response headers:\n{:#?}", resp.headers());
        debug!("response length: {}\n", resp.read(&mut body)?);

        let out = if let Ok(json) = serde_json::from_slice::<Value>(&body) {
            serde_json::to_vec_pretty(&json)?
        } else {
            body
        };

        let _ = io::copy(&mut out.as_slice(), &mut io::stdout())?;
        Ok(())
    }
}
