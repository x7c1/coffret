//! Authorizing against Google, once by hand and thereafter by itself.
//!
//! [`Authorization`] is the one-time flow that needs a person at a browser:
//! authorization code with PKCE over a loopback redirect, asking for
//! [`DRIVE_FILE_SCOPE`] and nothing else. Asking is only half of it: what the
//! endpoint answers is read as a [`GrantedScopes`] set and cached only where
//! that set is exactly [`DRIVE_FILE_SCOPE`], so a grant reaching further into
//! the account is refused instead of kept. What the flow leaves behind is a
//! refresh token in a [`TokenCache`] — sealed there under a key derived from
//! the Master Key for that one purpose, since a refresh token reaches every
//! object in the Library — and [`OAuthTokens`] mints access tokens from it for
//! every run afterwards.

mod access_tokens;
pub use access_tokens::AccessTokens;

mod authorization;
pub use authorization::{Authorization, GOOGLE_AUTHORIZATION_ENDPOINT};

mod client_credentials;
pub use client_credentials::ClientCredentials;

mod granted_scopes;
pub use granted_scopes::GrantedScopes;

mod oauth_tokens;
pub use oauth_tokens::OAuthTokens;

mod pkce;

mod stored_tokens;
pub use stored_tokens::StoredTokens;

mod token_cache;
pub use token_cache::TokenCache;

mod token_endpoint;
pub use token_endpoint::{DRIVE_FILE_SCOPE, GOOGLE_TOKEN_ENDPOINT};

mod token_response;
