//! This module provides a thin wrapper around the `reqwest` crate, providing direct access to the
//! judge board server's REST APIs.
//!

use std::sync::Once;

use reqwest::{Client as HttpClient, Url};
use serde::Serialize;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        SerdeJsonError(::serde_json::Error);
        ReqwestUrlError(::reqwest::UrlError);
        ReqwestError(::reqwest::Error);
    }

    errors {
        NonSuccessfulStatusCode(status_code: u16) {
            description("remote responses with unsuccessful status code")
            display("remote responses with unsuccessful status code: {}", status_code)
        }
    }
}

/// The application wide singleton of HTTP client object used for communicating with the judge board
/// server. The initialization of this singleton object is protected by `HTTP_CLIENT_ONCE`.
static mut HTTP_CLIENT: Option<HttpClient> = None;
static HTTP_CLIENT_ONCE: Once = Once::new();

/// Invokes the given function on the statically allocated HTTP client object. This function returns
/// whatever the given function returns, if no errors occur.
fn with_http_client<F, R>(func: F) -> Result<R>
    where F: FnOnce(&'static HttpClient) -> R {
    // Initialize the HTTP client if not done yet.
    HTTP_CLIENT_ONCE.call_once(|| {
        unsafe {
            HTTP_CLIENT = Some(HttpClient::new());
        }
    });

    unsafe {
        Ok(func(HTTP_CLIENT.as_ref().unwrap()))
    }
}

pub fn patch<T, U>(path: T, payload: &U) -> Result<()>
    where T: AsRef<str>, U: ?Sized + Serialize {
    let config = crate::config::app_config();
    let full_path_str = format!("{}{}", config.judge_board_url, path.as_ref());
    let full_path = Url::parse(&full_path_str)?;

    let response = with_http_client(|http| {
        http.patch(full_path)
            .json(payload)
            .send()
    })??;

    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        Err(Error::from(ErrorKind::NonSuccessfulStatusCode(status.as_u16())))
    }
}
