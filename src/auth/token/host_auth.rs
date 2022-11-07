use super::JwtToken;
use crate::auth::{from_encoded, TokenClaim, TokenRole, TokenType};
use crate::errors::Result;
use derive_getters::Getters;
use std::str;
use std::str::FromStr;
use uuid::Uuid;

/// The claims of the token to be stored (encrypted) on the client side
#[derive(Debug, serde::Deserialize, serde::Serialize, Getters)]
pub struct HostAuthToken {
    id: Uuid,
    exp: i64,
    token_type: TokenType,
    role: TokenRole,
}

#[tonic::async_trait]
impl JwtToken for HostAuthToken {
    fn get_expiration(&self) -> i64 {
        self.exp
    }

    fn get_id(&self) -> Uuid {
        self.id
    }

    fn new(claim: TokenClaim) -> Self {
        let data = claim.data.unwrap_or_default();
        let def = &"service".to_string();
        let role =
            TokenRole::from_str(data.get("role").unwrap_or(def)).unwrap_or(TokenRole::Service);

        Self {
            id: claim.id,
            exp: claim.exp,
            token_type: TokenType::HostAuth,
            role,
        }
    }

    fn token_type(&self) -> TokenType {
        self.token_type
    }
}

impl FromStr for HostAuthToken {
    type Err = super::TokenError;

    fn from_str(encoded: &str) -> Result<Self, Self::Err> {
        from_encoded::<HostAuthToken>(encoded, TokenType::HostAuth)
    }
}

impl super::Identifier for HostAuthToken {
    fn get_id(&self) -> Uuid {
        self.id
    }
}
