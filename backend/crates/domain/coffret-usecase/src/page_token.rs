use std::fmt;

/// The provider's marker for where one page of a listing left off.
///
/// It is opaque: a caller passes back what the previous page returned and never
/// builds one. S3 spells it as a continuation token and Drive as a page token;
/// both are ordinary strings that mean nothing outside the listing that issued
/// them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PageToken(String);

impl PageToken {
    /// Takes the marker the provider returned with a page.
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// The marker as the provider spells it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PageToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
