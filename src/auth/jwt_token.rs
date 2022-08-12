use axum::http::header::AUTHORIZATION;
use axum::http::Request as HttpRequest;
use base64::{decode as base64_decode, DecodeError};
use jsonwebtoken::{
    decode, encode, errors::Error as JwtError, Algorithm, DecodingKey, EncodingKey, Header,
    Validation,
};
use serde::{Deserialize, Serialize};
use std::env::VarError;
use std::str::{FromStr, Utf8Error};
use std::{env, str};
use thiserror::Error;
use tonic::Request as GrpcRequest;
use uuid::Uuid;

pub type TokenResult<T> = Result<T, TokenError>;

pub trait Identifier {
    fn get_id(&self) -> Uuid;
}

#[derive(Error, Debug)]
pub enum TokenError {
    #[error("Token is empty")]
    Empty,
    #[error("Token has expired")]
    Expired,
    #[error("Token couldn't be decoded: {0:?}")]
    EnDeCoding(#[from] JwtError),
    #[error("Env var not defined: {0:?}")]
    EnvVar(#[from] VarError),
    #[error("UTF-8 error: {0:?}")]
    Utf8(#[from] Utf8Error),
    #[error("JWT decoding error: {0:?}")]
    JwtDecoding(#[from] DecodeError),
}

/// Type of user holding the token, i.e. gets authenticated
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum TokenHolderType {
    Host,
    User,
}

/// The claims of the token to be stored (encrypted) on the client side
#[derive(Debug, Deserialize, Serialize)]
pub struct JwtToken {
    id: Uuid,
    exp: i64,
    holder_type: TokenHolderType,
}

impl JwtToken {
    pub fn new(id: Uuid, exp: i64, holder_type: TokenHolderType) -> Self {
        Self {
            id,
            exp,
            holder_type,
        }
    }

    pub fn token_holder(self) -> TokenHolderType {
        self.holder_type
    }

    /// Encode this instance to a JWT token string
    pub fn encode(&self) -> TokenResult<String> {
        let secret = Self::get_secret()?;
        let header = Header::new(Algorithm::HS512);

        match encode(&header, self, &EncodingKey::from_secret(secret.as_ref())) {
            Ok(token_str) => Ok(token_str),
            Err(e) => Err(TokenError::EnDeCoding(e)),
        }
    }

    /// Create new JWT token from given request
    /// TODO: refactor me
    pub fn new_for_request<B>(request: &HttpRequest<B>) -> TokenResult<Self> {
        let token = request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|hv| hv.to_str().ok())
            .and_then(|hv| {
                let words = hv.split("Bearer").collect::<Vec<&str>>();

                words.get(1).map(|w| w.trim())
            })
            .unwrap_or("");
        let clear_token = base64_decode(token)?;
        let token = str::from_utf8(&clear_token)?;

        JwtToken::from_str(token)
    }

    /// Create new JWT token from given gRPC request
    /// TODO: refactor me
    pub fn new_for_grpc_request<B>(request: &GrpcRequest<B>) -> TokenResult<Self> {
        let token = request
            .metadata()
            .get("authorization")
            .and_then(|mv| mv.to_str().ok())
            .and_then(|mv| {
                let words = mv.split("Bearer").collect::<Vec<&str>>();

                words.get(1).map(|w| w.trim())
            })
            .unwrap_or("");
        let clear_token = base64_decode(token)?;
        let token = str::from_utf8(&clear_token)?;

        JwtToken::from_str(token)
    }

    /// Get JWT_SECRET from env vars
    fn get_secret() -> TokenResult<String> {
        match env::var("JWT_SECRET") {
            Ok(secret) => {
                assert!(!secret.is_empty());

                Ok(secret)
            }
            Err(e) => Err(TokenError::EnvVar(e)),
        }
    }
}

impl FromStr for JwtToken {
    type Err = TokenError;

    fn from_str(encoded: &str) -> Result<Self, Self::Err> {
        let secret = Self::get_secret()?;
        let mut validation = Validation::new(Algorithm::HS512);

        validation.validate_exp = true;

        match decode::<JwtToken>(
            encoded,
            &DecodingKey::from_secret(secret.as_bytes()),
            &validation,
        ) {
            Ok(token) => Ok(token.claims),
            Err(e) => Err(TokenError::EnDeCoding(e)),
        }
    }
}

impl Identifier for JwtToken {
    fn get_id(&self) -> Uuid {
        self.id
    }
}
