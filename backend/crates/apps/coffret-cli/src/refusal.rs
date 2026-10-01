//! The refusals this binary makes itself, before it calls a flow.

use std::error;
use std::fmt;

use crate::drive_client::CLIENT_SECRET;

/// What was given that no flow can be called with, found by this shell and not
/// by a layer beneath it.
///
/// A type rather than a sentence, so that the `--json` answer can name each of
/// these by its variant instead of calling it `"other"`, the kind of a failure
/// a script cannot tell from any other. Every variant is a
/// state of the command line or the environment that a person can put right,
/// and none of them repeats a secret it was refusing.
#[derive(Debug)]
pub enum Refusal {
    /// A provider's flag was given without the flags that provider needs.
    ///
    /// The parser is told which flags each provider requires and refuses most
    /// of these itself, so this is the shell's own check that it did — a
    /// shape it let through is still refused, and in words, rather than
    /// unwrapped.
    FlagsMissing {
        /// The provider's flag, as typed: `--drive` or `--s3`.
        provider: &'static str,
        /// The flags it needs, as the sentence names them.
        needs: &'static str,
    },
    /// The client secret variable is set to an empty value.
    ///
    /// Why that is refused rather than read as unset is on
    /// [`client_secret`](crate::drive_client::client_secret).
    EmptyClientSecret,
    /// The client secret variable holds something that is not Unicode.
    ///
    /// What it holds is not kept: it is the secret, or something meant as it,
    /// and a refusal of a secret names the check it failed and never the bytes
    /// that failed it.
    ClientSecretNotUnicode,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FlagsMissing { provider, needs } => write!(f, "{provider} needs {needs}"),
            Self::EmptyClientSecret => write!(
                f,
                "{CLIENT_SECRET} is set to an empty value; set it to the secret the client was \
                 registered with, or unset it where the client was registered without one"
            ),
            Self::ClientSecretNotUnicode => write!(f, "{CLIENT_SECRET} is not valid Unicode"),
        }
    }
}

// Nothing underneath any of them: each is this shell's own reading of what it
// was given, and the one value that could have been a cause — the variable's
// contents — is withheld on purpose.
impl error::Error for Refusal {}
