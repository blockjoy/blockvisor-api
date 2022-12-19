use crate::auth::TokenType;
use anyhow::anyhow;
use derive_getters::Getters;
use std::fmt::{Display, Formatter};
use std::fs;
use thiserror::Error;

pub type KeyProviderResult = Result<KeyValue, KeyProviderError>;

#[derive(Error, Debug)]
pub enum KeyProviderError {
    #[error("Key is empty")]
    Empty,
    #[error("Env var couldn't be loaded: {0}")]
    DotenvError(#[from] dotenv::Error),
    #[error("Unexpected error: {0}")]
    UnexpectedError(#[from] anyhow::Error),
}

#[derive(Getters)]
pub struct KeyValue {
    value: String,
}

impl KeyValue {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

impl Display for KeyValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

pub struct KeyProvider;

impl KeyProvider {
    pub fn get_secret(token_type: TokenType) -> KeyProviderResult {
        let key_retriever = match Self::get_env_value("SECRETS_ROOT") {
            Ok(_) => Self::get_key_value,
            Err(_) => Self::get_env_value,
        };
        let key = match token_type {
            TokenType::UserAuth => key_retriever("JWT_SECRET"),
            TokenType::UserRefresh => key_retriever("REFRESH_SECRET"),
            TokenType::HostAuth => key_retriever("JWT_SECRET"),
            TokenType::HostRefresh => key_retriever("REFRESH_SECRET"),
            TokenType::RegistrationConfirmation => key_retriever("CONFIRMATION_SECRET"),
            TokenType::PwdReset => key_retriever("PWD_RESET_SECRET"),
        };

        let key = key?;

        if key.value.is_empty() {
            Err(KeyProviderError::Empty)
        } else {
            Ok(key)
        }
    }

    fn get_env_value(name: &str) -> KeyProviderResult {
        std::env::var(name)
            .map(KeyValue::new)
            .map_err(|e| KeyProviderError::UnexpectedError(anyhow!(e)))
    }

    fn get_key_value(name: &str) -> KeyProviderResult {
        let path = format!("{}/{}", Self::get_env_value("SECRETS_ROOT")?, name);
        let value = fs::read_to_string(&path).map(KeyValue::new).map_err(|e| {
            KeyProviderError::UnexpectedError(anyhow!(
                "Couldn't read secret {} from disk: {}",
                e,
                path
            ))
        })?;

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::auth::key_provider::KeyProvider;
    use crate::auth::TokenType;
    use std::fs;

    #[test]
    fn can_read_secret_from_env() -> anyhow::Result<()> {
        std::env::set_var("JWT_SECRET", "123123");

        let key = KeyProvider::get_secret(TokenType::UserAuth)?;

        assert_eq!("123123".to_string(), key.to_string());

        Ok(())
    }

    #[test]
    fn can_read_secret_from_file() -> anyhow::Result<()> {
        std::env::set_var("JWT_SECRET", "098080");
        std::env::set_var("SECRETS_ROOT", "/tmp");
        let path = "/tmp/JWT_SECRET";
        fs::write(path, b"123123")?;

        let key = KeyProvider::get_secret(TokenType::UserAuth)?;

        assert_eq!("123123".to_string(), key.to_string());

        fs::remove_file(path)?;

        Ok(())
    }
}
