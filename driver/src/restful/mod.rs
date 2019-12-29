//! This module provides a thin wrapper around the `reqwest` crate, providing direct access to the
//! judge board server's REST APIs.
//!

mod auth;
pub mod entities;
mod pipeline;

use std::io::Write;

use reqwest::{
    Client as HttpClient,
    RequestBuilder,
    Method as HttpMethod,
    Response,
    Url
};

use serde::Serialize;

use openssl::pkey::Private as PrivateKey;
use openssl::rsa::Rsa;

use entities::{ObjectId, Heartbeat, ProblemInfo, SubmissionInfo, SubmissionJudgeResult};
use pipeline::Pipeline;
use auth::Authenticator;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    links {
        PipelineError(pipeline::Error, pipeline::ErrorKind);
    }

    foreign_links {
        IoError(::std::io::Error);
        SerdeJsonError(::serde_json::Error);
        ReqwestUrlError(::reqwest::UrlError);
        ReqwestError(::reqwest::Error);
    }

    errors {
        UnsuccessfulStatusCode(status_code: u16) {
            description("remote responses with unsuccessful status code")
            display("remote responses with unsuccessful status code: {}", status_code)
        }
    }
}

/// Provide a REST client to the judge board server.
pub struct RestfulClient {
    /// The URL to the judge board server.
    judge_board_url: Url,

    /// The request pipeline.
    pipeline: Pipeline,

    /// The http client.
    http: HttpClient,
}

impl RestfulClient {
    /// Create a new `RestfulClient` instance.
    pub fn new<U>(judge_board_url: U, auth_key: Rsa<PrivateKey>) -> Self
        where U: Into<Url> {
        let judge_board_url = judge_board_url.into();
        let authenticator = Authenticator::new(judge_board_url.clone(), auth_key);

        let mut pipeline = Pipeline::new();
        pipeline.add_middleware(Box::new(authenticator));

        RestfulClient {
            judge_board_url,
            pipeline,
            http: HttpClient::new(),
        }
    }

    /// Get full request URL to the judge board server. The given path should be an absolute path
    /// that can be concatenated after the host part of the URL, e.g. `/judges`.
    fn get_full_request_url<T>(&self, path: &T) -> Url
        where T: ?Sized + AsRef<str> {
        let mut full_path = self.judge_board_url.clone();
        full_path.set_path(path.as_ref());
        full_path
    }

    /// Execute the given request and get the response. This function will return error if the
    /// status of the response is not 2XX.
    fn request(&self, req: RequestBuilder) -> Result<Response> {
        let response = self.pipeline.execute(req).map_err(Error::from)?;
        if response.status().is_success() {
            Ok(response)
        } else {
            Err(Error::from(ErrorKind::UnsuccessfulStatusCode(response.status().as_u16())))
        }
    }

    /// Send a GET request to the judge board server.
    fn get<T>(&self, path: &T) -> Result<Response>
        where T: ?Sized + AsRef<str> {
        let request_url = self.get_full_request_url(path);
        let request = self.http.request(HttpMethod::GET, request_url);
        self.request(request)
    }

    /// Send a GET request to the judge board server, saving the content of the response to the given
    /// output device.
    fn download<T1, T2>(&self, path: &T1, output: &mut T2) -> Result<()>
        where T1: ?Sized + AsRef<str>, T2: ?Sized + Write {
        let mut response = self.get(path)?;
        std::io::copy(&mut response, output)?;

        Ok(())
    }

    /// Send a PATCH request to the judge board server, requesting the given path. The body of the
    /// request will be populated by the payload in JSON format.
    fn patch<T, U>(&self, path: &T, payload: &U) -> Result<()>
        where T: ?Sized + AsRef<str>,
              U: ?Sized + Serialize {
        let request_url = self.get_full_request_url(path);
        let request = self.http.request(HttpMethod::PATCH, request_url)
            .json(payload);
        self.request(request)?;

        Ok(())
    }

    /// Send a heartbeat packet to the judge board.
    pub fn patch_heartbeat(&self, hb: &Heartbeat) -> Result<()> {
        self.patch("/judges", hb)
    }

    /// Download the given test archive and save to the given output device.
    pub fn download_archive<O>(&self, archive_id: ObjectId, output: &mut O) -> Result<()>
        where O: ?Sized + Write {
        let path = format!("/archives/{}", archive_id);
        self.download(&path, output)
    }

    /// Get problem information.
    pub fn get_problem_info(&self, problem_id: ObjectId) -> Result<ProblemInfo> {
        let path = format!("/problems/{}", problem_id);
        self.get(&path)?.json().map_err(Error::from)
    }

    /// Get the timestamp of the specified problem.
    pub fn get_problem_timestamp(&self, problem_id: ObjectId) -> Result<u64> {
        let path = format!("/problems/{}/timestamp", problem_id);
        self.get(&path)?.json().map_err(Error::from)
    }

    /// Get an unjudged submission from the judge board server.
    pub fn get_submission(&self) -> Result<Option<SubmissionInfo>> {
        let mut response = self.get("/submissions")?;
        if response.status() == 200 {
            let submission: SubmissionInfo = response.json()?;
            Ok(Some(submission))
        } else {
            // Note that the status code returned by `self.get` must be 2XX.
            Ok(None)
        }
    }

    /// Patch the given submission judge result.
    pub fn patch_judge_result(&self,
        submission_id: ObjectId,
        result: &SubmissionJudgeResult) -> Result<()> {
        let path = format!("/submissions/{}", submission_id);
        self.patch(&path, result)
    }
}
