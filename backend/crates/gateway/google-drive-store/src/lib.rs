//! [`ObjectStore`](coffret_usecase::ObjectStore) over a Google Drive folder.
//!
//! Drive is the provider coffret was designed against, and it is unlike an
//! object store in the places that matter to the port:
//!
//! - **Names are not identity.** Drive mints a file id, and a create may name
//!   the id it is going to use. That is what makes a commit slot a real
//!   reservation here — `files.generateIds` mints one, and the create that
//!   spends it either lands or finds it taken — where an S3 bucket has only its
//!   key space.
//! - **Uploads are sessions.** Every object goes up through a resumable upload,
//!   and the digest Drive reports is checked against one computed as the bytes
//!   were sent, so an object that arrived corrupted fails its upload rather than
//!   sitting in Storage waiting to be found unopenable.
//! - **A trash already exists**, and so does permanent deletion, so both of the
//!   port's removals map onto Drive's own.
//!
//! The grant asked for is [`DRIVE_FILE_SCOPE`] alone: coffret reaches the files
//! it created and nothing else in the account. [`Authorization`] runs the
//! one-time authorization code flow with PKCE over a loopback redirect, and
//! [`OAuthTokens`] mints access tokens from what it cached for every run after
//! that.
//!
//! [`create_app_folder`] is the one operation that runs before any of that: a
//! Library's objects live flat in a folder named after the Library itself
//! (spec: FM-18), and until that folder exists there is no store to build. It
//! is not part of the `ObjectStore` port, which is scoped to a Library that
//! already has somewhere to live.
//!
//! Nothing here reaches for a network of its own accord: the
//! [`HttpTransport`] and the [`AccessTokens`] are constructor arguments. That is
//! what lets the behaviour worth testing — how failures are classified, that a
//! 401 costs exactly one refresh, that a digest disagreement fails an upload —
//! be tested against a scripted transport, with the same code that ships.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod api;
pub use api::DRIVE_API;

mod app_folder;
pub use app_folder::create_app_folder;

#[cfg(test)]
mod classification_tests;

mod digesting_reader;

mod error;
pub use error::{
    AppFolderDefect, Error, RedirectStep, Result, TokenCacheDefect, TokenResponseDefect,
};

mod google_drive;
pub use google_drive::GoogleDrive;

pub mod http;
pub use http::{HttpTransport, ReqwestTransport};

#[cfg(test)]
mod integrity_tests;

#[cfg(test)]
mod logging_tests;

mod oauth;
pub use oauth::{
    AccessTokens, Authorization, ClientCredentials, OAuthTokens, StoredTokens, TokenCache,
    DRIVE_FILE_SCOPE, GOOGLE_AUTHORIZATION_ENDPOINT, GOOGLE_TOKEN_ENDPOINT,
};

#[cfg(test)]
mod refresh_tests;

#[cfg(test)]
mod retry_tests;

mod settings;
pub use settings::DriveSettings;

#[cfg(test)]
mod test_support;

mod upload;

mod upload_digest;
