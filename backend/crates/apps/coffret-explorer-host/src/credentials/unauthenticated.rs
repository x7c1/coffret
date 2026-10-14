use std::fmt;

/// A browser token that admits nobody: missing, malformed, never issued,
/// expired, revoked, or of a generation the host has since retired. One value
/// for all of them, so that no answer can tell them apart (spec: LA-14).
#[derive(Debug, Clone, Copy)]
pub struct Unauthenticated;

impl fmt::Display for Unauthenticated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the browser token is not one this host holds")
    }
}

impl std::error::Error for Unauthenticated {}
