//! This module provides a thin wrapper around the `reqwest` crate, providing direct access to the
//! judge board server's REST APIs.
//!

use std::io::Write;
use std::sync::Once;

use reqwest::{Client as HttpClient, Response, Url};
use serde::Serialize;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        IoError(::std::io::Error);
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

/// Get full request URL to the judge board server. The given path should be an absolute path that
/// can be concatenated after the host part of the URL, e.g. `/judges`.
fn get_full_request_url<T>(path: T) -> Result<reqwest::Url>
    where T: AsRef<str> {
    let config = crate::config::app_config();
    let full_path_str = format!("{}{}", config.cluster.judge_board_url, path.as_ref());
    Url::parse(&full_path_str)
        .map_err(|e| Error::from(e))
}

/// Send a GET request to the judge board server.
fn get<T>(path: T) -> Result<Response>
    where T: AsRef<str> {
    let request_url = get_full_request_url(path)?;

    with_http_client(|http| {
        http.get(request_url)
            .send()
            .map_err(|e| Error::from(e))
    })?
}

/// Send a GET request to the judge board server, saving the content of the response to the given
/// output device.
fn download<T1, T2>(path: T1, output: &mut T2) -> Result<()>
    where T1: AsRef<str>, T2: ?Sized + Write {
    let mut response = get(path)?;
    std::io::copy(&mut response, output)?;

    Ok(())
}

/// Send a PATCH request to the judge board server, requesting the given path. The body of the
/// request will be populated by the payload in JSON format.
fn patch<T, U>(path: T, payload: &U) -> Result<()>
    where T: AsRef<str>, U: ?Sized + Serialize {
    let request_url = get_full_request_url(path)?;

    let response = with_http_client(|http| {
        http.patch(request_url)
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

/// Provide a trait for heartbeat values.
pub trait Heartbeat : Serialize { }

/// Send a heartbeat packet to the judge board.
pub fn patch_heartbeat<H>(hb: &H) -> Result<()>
    where H: Heartbeat {
    patch("/judges", hb)
}

/// Download the given test archive and save to the given output device.
pub fn download_archive<T1, T2>(archive_id: T1, output: &mut T2) -> Result<()>
    where T1: ToString, T2: ?Sized + Write {
    let path = format!("/archives/{}", archive_id.to_string());
    download(path, output)
}
