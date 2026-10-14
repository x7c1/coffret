use std::fmt;

/// The operating system's CSPRNG refused to give a secret its bytes.
///
/// Nothing is issued in its place: a ticket or a token anything could guess
/// is not a weaker credential but none at all (spec: LA-20).
#[derive(Debug)]
pub struct NotDrawn(pub(super) getrandom::Error);

impl fmt::Display for NotDrawn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the operating system's random source refused to draw a credential")
    }
}

impl std::error::Error for NotDrawn {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
