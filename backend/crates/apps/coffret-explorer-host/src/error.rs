use std::fmt;

/// The host could not be built: the HTTP client it forwards through refused to
/// start, which is the TLS backend failing to initialise and nothing a caller
/// can put right by trying again.
#[derive(Debug)]
pub struct Error(pub(crate) reqwest::Error);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the explorer's host could not start its HTTP client")
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
