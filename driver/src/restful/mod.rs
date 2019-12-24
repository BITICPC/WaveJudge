//! This module provides a thin wrapper around the `reqwest` crate, providing direct access to the
//! judge board server's REST APIs.
//!

pub mod entities;

use std::io::Write;
use std::sync::Mutex;

use reqwest::{Client as HttpClient, Response, Url};
use serde::Serialize;

use entities::{ObjectId, Heartbeat, ProblemInfo, SubmissionInfo};

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

/// Provide a REST client to the judge board server.
pub struct RestfulClient {
    /// The underlying `HttpClient` object.
    http: Mutex<HttpClient>,

    /// The URL to the judge board server.
    judge_board_url: String,
}

impl RestfulClient {
    /// Create a new `RestfulClient` instance.
    pub fn new<U>(judge_board_url: U) -> Self
        where U: Into<String> {
        RestfulClient {
            http: Mutex::new(HttpClient::new()),
            judge_board_url: judge_board_url.into()
        }
    }

    /// Get full request URL to the judge board server. The given path should be an absolute path that
    /// can be concatenated after the host part of the URL, e.g. `/judges`.
    fn get_full_request_url<T>(&self, path: T) -> Result<reqwest::Url>
        where T: AsRef<str> {
        let full_path_str = format!("{}{}", self.judge_board_url, path.as_ref());
        Url::parse(&full_path_str)
            .map_err(|e| Error::from(e))
    }

    /// Invokes the given function on the statically allocated HTTP client object. This function returns
    /// whatever the given function returns, if no errors occur.
    fn with_http_client<F, R>(&self, func: F) -> R
        where F: FnOnce(&HttpClient) -> R {
        let lock = self.http.lock().expect("failed to lock mutex");
        func(&*lock)
    }

    /// Send a GET request to the judge board server.
    fn get<T>(&self, path: T) -> Result<Response>
        where T: AsRef<str> {
        let request_url = self.get_full_request_url(path)?;

        self.with_http_client(|http| {
            http.get(request_url)
                .send()
                .map_err(|e| Error::from(e))
        })
    }

    /// Send a GET request to the judge board server, saving the content of the response to the given
    /// output device.
    fn download<T1, T2>(&self, path: T1, output: &mut T2) -> Result<()>
        where T1: AsRef<str>, T2: ?Sized + Write {
        let mut response = self.get(path)?;
        std::io::copy(&mut response, output)?;

        Ok(())
    }

    /// Send a PATCH request to the judge board server, requesting the given path. The body of the
    /// request will be populated by the payload in JSON format.
    fn patch<T, U>(&self, path: T, payload: &U) -> Result<()>
        where T: AsRef<str>, U: ?Sized + Serialize {
        let request_url = self.get_full_request_url(path)?;

        let response = self.with_http_client(|http| {
            http.patch(request_url)
                .json(payload)
                .send()
        })?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(Error::from(ErrorKind::NonSuccessfulStatusCode(status.as_u16())))
        }
    }

    /// Send a heartbeat packet to the judge board.
    pub fn patch_heartbeat(&self, hb: &Heartbeat) -> Result<()> {
        self.patch("/judges", hb)
    }

    /// Download the given test archive and save to the given output device.
    pub fn download_archive<O>(&self, archive_id: ObjectId, output: &mut O) -> Result<()>
        where O: ?Sized + Write {
        let path = format!("/archives/{}", archive_id.to_string());
        self.download(path, output)
    }

    /// Get problem information.
    pub fn get_problem_info(&self, problem_id: ObjectId) -> Result<ProblemInfo> {
        let path = format!("/problems/{}", problem_id.to_string());
        self.get(path)?.json().map_err(Error::from)
    }

    /// Get an unjudged submission from the judge board server.
    pub fn get_submission(&self) -> Result<Option<SubmissionInfo>> {
        let mut response = self.get("/submissions")?;
        if response.status() == 200 {
            let submission: SubmissionInfo = response.json()?;
            Ok(Some(submission))
        } else if response.status() == 204 {
            Ok(None)
        } else {
            Err(Error::from(ErrorKind::NonSuccessfulStatusCode(response.status().as_u16())))
        }
    }
}
