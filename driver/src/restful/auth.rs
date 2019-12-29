//! This module handles client authentication to the judge board server.
//!

use std::sync::Mutex;

use openssl::pkey::Private as PrivateKey;
use openssl::rsa::{Rsa, Padding as RsaPadding};

use reqwest::Client as HttpClient;
use reqwest::{Url, Response};

use serde::Deserialize;

use super::pipeline::{Error, ErrorKind, Result};
use super::pipeline::{Middleware, PipelineContext};

// TODO: enhancement: multiple authentication requests might be sent in a short period of time
// TODO: under concurrent environment.

/// A HTTP pipeline middleware for authenticating this judge node.
pub struct Authenticator {
    /// The JWT.
    jwt: Mutex<Option<String>>,

    /// URL to the authentication server.
    auth_server: Url,

    /// The RSA private key used for challenging during authentication.
    rsa_key: Rsa<PrivateKey>,
}

impl Authenticator {
    /// Create a new `Authenticator` object.
    pub fn new<T>(auth_server: T, rsa_key: Rsa<PrivateKey>) -> Self
        where T: Into<Url> {
        Authenticator {
            jwt: Mutex::new(None),
            auth_server: auth_server.into(),
            rsa_key,
        }
    }

    fn get_post_auth_url(&self) -> Url {
        let mut url = self.auth_server.clone();
        url.set_path("/auth");
        url
    }

    fn get_patch_auth_url<T>(&self, session_id: T) -> Url
        where T: Into<String> {
        let mut url = self.auth_server.clone();
        url.set_path(&format!("/auth/{}", session_id.into()));
        url
    }

    /// Authenticate this session. `reauth` indicate whether to update the JWT by force.
    fn authenticate(&self, reauth: bool) -> Result<String> {
        let mut jwt_lock = self.jwt.lock().expect("failed to lock mutex");
        if jwt_lock.is_some() && !reauth {
            return Ok((*jwt_lock).clone().unwrap());
        }

        // Drain the jwt lock.
        jwt_lock.take();

        let client = HttpClient::new();
        let challenge = client.post(self.get_post_auth_url())
            .send()?
            .json::<ChallengeInfo>()?;
        let challenge_data = challenge.get_challenge_bytes()?;

        let mut decrypted_challenge: Vec<u8> = vec![0u8; self.rsa_key.size() as usize];
        let decrypted_challenge_size =
            self.rsa_key.private_decrypt(
                &challenge_data, &mut decrypted_challenge, RsaPadding::PKCS1)
            .map_err(|e| Error::from(ErrorKind::MiddlewareError(
                format!("failed to decrypt challenge using RSA private key: {}", e))))?;
        decrypted_challenge.resize_with(decrypted_challenge_size, Default::default);
        let decrypted_challenge_base64 = base64::encode(&decrypted_challenge);

        let auth = client.patch(self.get_patch_auth_url(challenge.id))
            .body(decrypted_challenge_base64)
            .send()?
            .json::<AuthenticationInfo>()?;

        jwt_lock.replace(auth.jwt);
        Ok((*jwt_lock).clone().unwrap())
    }

    fn handle_with_jwt(mut context: PipelineContext<'_>, jwt: &str) -> Result<Response> {
        context.map_request(|req| req.bearer_auth(jwt));
        context.invoke_next()
    }
}

impl Middleware for Authenticator {
    fn handle(&self, context: PipelineContext<'_>) -> Result<Response> {
        let jwt = match self.authenticate(false) {
            Ok(jwt) => jwt,
            Err(e) => return Err(Error::from(ErrorKind::MiddlewareError(
                format!("failed to authenticate: {}", e))))
        };

        let saved_context = context.try_clone();

        let response = Self::handle_with_jwt(context, &jwt)?;
        let status_code = response.status().as_u16();
        if status_code == 401 || status_code == 403 {
            if saved_context.is_none() {
                // The clone of context was failed and there is no way to re-execute the request.
                // So we just return with an error.
                return Err(Error::from(ErrorKind::MiddlewareError(
                    String::from("failed to re-execute request after authentication failed."))));
            }

            // Authorization failed. Re-authenticate this session.
            let fresh_jwt = self.authenticate(true)?;
            Self::handle_with_jwt(saved_context.unwrap(), &fresh_jwt)
        } else {
            Ok(response)
        }
    }
}

#[derive(Deserialize)]
struct ChallengeInfo {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "challenge")]
    challenge: String,
}

impl ChallengeInfo {
    fn get_challenge_bytes(&self) -> Result<Vec<u8>> {
        base64::decode(&self.challenge)
            .map_err(|e| Error::from(ErrorKind::MiddlewareError(
                format!("failed to decode base64: {}: {}", self.challenge, e))))
    }
}

#[derive(Deserialize)]
struct AuthenticationInfo {
    #[serde(rename = "jwt")]
    jwt: String,
}
