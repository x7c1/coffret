use std::fs;

use coffret_format::decode_token_cache;

use super::TokenCache;
use crate::error::{Error, Result};
use crate::oauth::stored_tokens::StoredTokens;

impl TokenCache {
    /// Reads the cached tokens, or `None` if nothing has been cached yet.
    pub fn load(&self) -> Result<Option<StoredTokens>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(Error::TokenCache {
                    path: self.path.clone(),
                    detail: error.to_string(),
                })
            }
        };

        let document =
            decode_token_cache(&bytes, &self.master_key).map_err(|error| self.malformed(error))?;

        serde_json::from_slice(&document)
            .map(Some)
            .map_err(|error| self.malformed(error))
    }

    /// Reports a cache this build cannot read.
    fn malformed(&self, detail: impl std::fmt::Display) -> Error {
        Error::MalformedTokenCache {
            path: self.path.clone(),
            detail: detail.to_string(),
        }
    }
}
