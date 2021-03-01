use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::Url;

use crate::api::auth_plus::AccessToken;
use crate::error::{Error, Result};

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
            debug!("request headers:\n{:#?}", req.headers());
        }
        if let Some(body) = req.body() {
            debug!("request body:\n{:#?}\n", body);
        }

        Client::new().execute(req).map_err(Error::Http)
    }
}
