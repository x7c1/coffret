use std::fmt;

use zeroize::Zeroizing;

/// The address a Passphrase window is loaded from:
/// `http://127.0.0.1:<port>/#door=<token>`, the window's token in the fragment,
/// which the page takes out of the address before it does anything else
/// (spec: LA-23).
///
/// It carries the token, so it is held as the token is: in a buffer that is
/// wiped when it is dropped, not `Clone`, and redacted in `Debug` and
/// `Display`, so that it is never put into an event (spec: EL-1).
pub struct DoorUrl(Zeroizing<String>);

impl DoorUrl {
    pub(crate) fn new(url: Zeroizing<String>) -> Self {
        Self(url)
    }

    /// The URL, for the shell to load into the window and nowhere else.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DoorUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DoorUrl(<redacted>)")
    }
}

impl fmt::Display for DoorUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted door URL>")
    }
}
