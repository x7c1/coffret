use std::fs;

use coffret_format::decode_token_cache;

use super::TokenCache;
use crate::error::{Error, Result, TokenCacheDefect};
use crate::oauth::stored_tokens::StoredTokens;

impl TokenCache {
    /// Reads the cached tokens, or `None` if nothing has been cached yet.
    pub fn load(&self) -> Result<Option<StoredTokens>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(cause) => {
                return Err(Error::TokenCache {
                    path: self.path.clone(),
                    cause,
                })
            }
        };

        let document = decode_token_cache(&bytes, &self.master_key)
            .map_err(|cause| self.malformed(TokenCacheDefect::Sealed(cause)))?;

        serde_json::from_slice(&document)
            .map(Some)
            .map_err(|cause| self.malformed(TokenCacheDefect::Document(cause)))
    }

    /// Reports a cache this build cannot read.
    fn malformed(&self, cause: TokenCacheDefect) -> Error {
        Error::MalformedTokenCache {
            path: self.path.clone(),
            cause,
        }
    }
}
