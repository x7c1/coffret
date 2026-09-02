use axum::http::HeaderMap;

use super::{Admission, CAPABILITY_HEADER};

impl Admission {
    /// Whether the request carries the key this run drew.
    ///
    /// The comparison does not stop at the first byte that differs. A caller
    /// that may ask as often as it likes could otherwise read the key out of how
    /// long each guess took, one byte at a time.
    pub(super) fn shows_the_key(&self, headers: &HeaderMap) -> bool {
        let Some(shown) = headers.get(CAPABILITY_HEADER) else {
            return false;
        };
        let shown = shown.as_bytes();
        let expected = self.secret.as_bytes();
        if shown.len() != expected.len() {
            return false;
        }
        shown
            .iter()
            .zip(expected)
            .fold(0_u8, |sofar, (shown, expected)| sofar | (shown ^ expected))
            == 0
    }
}
