//! What a device records about one account: the OAuth client its grant was
//! issued to.

use std::fs;
use std::io::ErrorKind;

use coffret_usecase::{LocalIoError, LocalOperation};
use serde::{Deserialize, Serialize};

use crate::account_dir::AccountDir;
use crate::error::{Error, Result};
use crate::owner_only;

/// The OAuth client one account's grant was issued to on this device.
///
/// One account name binds to one client: the account's grant is renewed, and
/// consented to, only through the client the account first consented to, since
/// a token minted through another client is a different grant with a different
/// reach (spec: SA-8). So the client is recorded here, beside the cache, and a
/// Library naming another one is refused rather than served by this grant.
///
/// The secret is here for the reason it is in a Library's settings: a desktop
/// client's secret ships inside every copy of the client, and renewing the
/// account's grant by its name needs it without any one Library being named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AccountSettings {
    /// The shape this file is in.
    pub(crate) version: u32,
    /// The OAuth client the account's grant was issued to.
    pub(crate) client_id: String,
    /// The client secret, for a client registered with one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) client_secret: Option<String>,
}

/// Just enough of the file to say whether this build can read the rest.
#[derive(Deserialize)]
struct Shape {
    version: u32,
}

impl AccountSettings {
    /// The version this build writes and reads.
    pub(crate) const VERSION: u32 = 1;

    /// Settings for an account whose grant goes to `client_id`.
    pub(crate) fn new(client_id: &str, client_secret: Option<&str>) -> Self {
        Self {
            version: Self::VERSION,
            client_id: client_id.to_owned(),
            client_secret: client_secret.map(str::to_owned),
        }
    }

    /// Reads the settings of the account in `dir`, or `None` where it has none.
    pub(crate) fn read(dir: &AccountDir) -> Result<Option<Self>> {
        let path = dir.settings_file();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(cause) if cause.kind() == ErrorKind::NotFound => return Ok(None),
            Err(cause) => {
                return Err(LocalIoError::new(LocalOperation::Reading, path, cause).into())
            }
        };
        let malformed = |cause| Error::MalformedSettings {
            path: path.clone(),
            cause,
        };
        let shape: Shape = serde_json::from_slice(&bytes).map_err(malformed)?;
        if shape.version != Self::VERSION {
            return Err(Error::UnsupportedSettingsVersion {
                path,
                version: shape.version,
                expected: Self::VERSION,
            });
        }
        serde_json::from_slice(&bytes).map(Some).map_err(malformed)
    }

    /// Writes these settings as the settings of the account in `dir`,
    /// creating the directory where it is not there yet.
    pub(crate) fn write(&self, dir: &AccountDir) -> Result<()> {
        owner_only::create_dir(dir.path())?;
        let path = dir.settings_file();
        let mut document =
            serde_json::to_vec_pretty(self).map_err(|cause| Error::UnencodableSettings {
                path: path.clone(),
                cause,
            })?;
        document.push(b'\n');
        owner_only::write_file(&path, &document)
    }
}
