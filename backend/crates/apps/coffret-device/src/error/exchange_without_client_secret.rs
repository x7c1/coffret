//! Whether an [`Error`] is a code exchange refused to a client that sent no
//! secret, looked for through the failures that carry one.

use super::Error;

impl Error {
    /// Whether this is a code exchange the token endpoint refused, made by a
    /// client that sent no secret.
    ///
    /// The one refusal a shell has something of its own to add to: a client
    /// registered with a secret cannot be authorized without it, and where a
    /// secret would have come from — a variable, a flag, a settings file — is
    /// the shell's vocabulary rather than this crate's or the gateway's. So the
    /// fact travels up typed, and the shell that knows the name says it.
    ///
    /// The two creation failures are looked through because that is where this
    /// arrives from: an `init` or a `join` that got as far as the browser and
    /// no further reports the step, and the exchange's refusal is inside it.
    pub fn is_exchange_without_client_secret(&self) -> bool {
        match self {
            Self::Drive { cause } => {
                matches!(
                    cause.as_ref(),
                    google_drive_store::Error::CodeExchangeWithoutSecret { .. }
                )
            }
            Self::LibraryNotCreated { cause, .. } | Self::LibraryNotJoined { cause, .. } => {
                cause.is_exchange_without_client_secret()
            }
            _ => false,
        }
    }
}
