use std::sync::{Mutex, MutexGuard};

use coffret_explorer_host::credentials::{Digest, NotDrawn, Secret};
use zeroize::Zeroizing;

use super::DoorMode;

/// How many refused submissions a window's token survives: the fifth ends it,
/// and the window has to be opened again (spec: LA-24).
pub(crate) const MAX_REFUSED: u8 = 5;

/// How many windows' tokens are held at once. Opening one more ends the
/// oldest, so that a shell that opens windows and never submits them holds a
/// handful of tokens rather than one per opening (spec: LA-23).
pub(crate) const MAX_WINDOWS: usize = 4;

/// The door's tokens: one per opening of a Passphrase window, each held as its
/// digest beside the mode it opened the window in (spec: LA-23).
///
/// Every transition — an issue, an admission, an outcome, the end of all of
/// them — is one step under one lock. No event recorded here names a token or
/// its digest: only what happened and the counts it left (spec: EL-1, LA-25).
#[derive(Default)]
pub(crate) struct Windows(Mutex<Vec<Window>>);

/// One window's token, as the door holds it.
struct Window {
    digest: Digest,
    mode: DoorMode,
    refused: u8,
}

/// What became of a window's token after a refused submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AfterRefusal {
    /// It stands, for another try.
    Stands,
    /// That was the last try it had; it has ended.
    Ended,
    /// It was no longer held: it ended while the submission was under way.
    Gone,
}

impl Windows {
    /// A fresh token for a window opened in `mode`, as the window is handed
    /// it: 43 characters of unpadded base64url, in a buffer that is wiped when
    /// it is dropped (spec: LA-20, LA-23).
    pub(crate) fn issue(&self, mode: DoorMode) -> Result<Zeroizing<String>, NotDrawn> {
        let secret = Secret::draw()?;
        let mut windows = self.windows();
        if windows.len() >= MAX_WINDOWS {
            windows.remove(0);
            tracing::info!(
                kind = "door_window_ended",
                reason = "evicted",
                "the oldest Passphrase window's token was ended to make room",
            );
        }
        windows.push(Window {
            digest: secret.digest(),
            mode,
            refused: 0,
        });
        tracing::info!(
            kind = "door_window_issued",
            live_windows = windows.len(),
            "a Passphrase window's token was issued",
        );
        Ok(secret.encoded())
    }

    /// The mode of the window whose token `presented` is, where it is one the
    /// door holds; `None` for one it does not, however it fails to be one.
    pub(crate) fn admit(&self, presented: &str) -> Option<DoorMode> {
        let windows = self.windows();
        find(&windows, presented).map(|at| windows[at].mode)
    }

    /// Ends the token `presented`, its submission having opened or unlocked
    /// the Library (spec: LA-24).
    pub(crate) fn succeeded(&self, presented: &str) {
        let mut windows = self.windows();
        if let Some(at) = find(&windows, presented) {
            windows.remove(at);
            tracing::info!(
                kind = "door_window_ended",
                reason = "succeeded",
                "a Passphrase window's token was spent",
            );
        }
    }

    /// Counts a refused submission against the token `presented`, and ends it
    /// where that was its last try (spec: LA-24).
    pub(crate) fn refused(&self, presented: &str) -> AfterRefusal {
        let mut windows = self.windows();
        let Some(at) = find(&windows, presented) else {
            return AfterRefusal::Gone;
        };
        windows[at].refused += 1;
        let refused = windows[at].refused;
        if refused < MAX_REFUSED {
            tracing::info!(
                kind = "door_attempt_refused",
                refused_attempts = refused,
                "a Passphrase window's submission was refused",
            );
            return AfterRefusal::Stands;
        }
        windows.remove(at);
        tracing::info!(
            kind = "door_window_ended",
            reason = "attempts",
            refused_attempts = refused,
            "a Passphrase window's token was ended after its last refused submission",
        );
        AfterRefusal::Ended
    }

    /// Ends every window's token: what the core does as it retires.
    pub(crate) fn end_all(&self) {
        let mut windows = self.windows();
        let ended = windows.len();
        windows.clear();
        tracing::info!(
            kind = "door_window_ended",
            reason = "retired",
            ended_windows = ended,
            "every Passphrase window's token was ended",
        );
    }

    /// The tokens, for one transition.
    ///
    /// A lock poisoned by a panic in another transition may hold a list that
    /// transition left half done. It is not trusted: every token is ended, so
    /// that what fails, fails closed and the window is opened again.
    fn windows(&self) -> MutexGuard<'_, Vec<Window>> {
        match self.0.lock() {
            Ok(windows) => windows,
            Err(poisoned) => {
                let mut windows = poisoned.into_inner();
                windows.clear();
                self.0.clear_poison();
                tracing::warn!(
                    "a door transition panicked; every Passphrase window's token was ended",
                );
                windows
            }
        }
    }
}

/// Where the token `presented` is among `windows`, comparing each digest
/// without stopping at the first byte that differs (spec: LA-20).
///
/// Every digest held is compared, a match or not, and a presented token that is
/// not 43 characters of unpadded base64url is not looked up at all.
fn find(windows: &[Window], presented: &str) -> Option<usize> {
    let presented = Digest::of_presented(presented)?;
    windows
        .iter()
        .enumerate()
        .fold(None, |found, (at, window)| {
            if window.digest.same_as(&presented) {
                Some(at)
            } else {
                found
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // LA-24: a token ends on success, and is then refused.
    #[test]
    fn a_token_ends_on_success() {
        let windows = Windows::default();
        let token = windows.issue(DoorMode::Open).unwrap();
        assert_eq!(windows.admit(&token), Some(DoorMode::Open));
        windows.succeeded(&token);
        assert_eq!(windows.admit(&token), None);
        assert_eq!(windows.refused(&token), AfterRefusal::Gone);
    }

    // LA-24: a token survives four refused submissions and ends with the
    // fifth, and is then refused.
    #[test]
    fn a_token_ends_after_its_fifth_refusal() {
        let windows = Windows::default();
        let token = windows.issue(DoorMode::Unlock).unwrap();
        for _ in 1..MAX_REFUSED {
            assert_eq!(windows.refused(&token), AfterRefusal::Stands);
            assert_eq!(windows.admit(&token), Some(DoorMode::Unlock));
        }
        assert_eq!(windows.refused(&token), AfterRefusal::Ended);
        assert_eq!(windows.admit(&token), None);
    }

    // LA-23: two windows' tokens are independent: each has its own mode and its
    // own count, and ending one leaves the other.
    #[test]
    fn two_windows_tokens_are_independent() {
        let windows = Windows::default();
        let first = windows.issue(DoorMode::Open).unwrap();
        let second = windows.issue(DoorMode::Unlock).unwrap();
        assert_ne!(*first, *second);

        for _ in 1..MAX_REFUSED {
            windows.refused(&first);
        }
        assert_eq!(windows.admit(&second), Some(DoorMode::Unlock));
        assert_eq!(
            windows.refused(&second),
            AfterRefusal::Stands,
            "the second has its own count"
        );
        windows.succeeded(&second);
        assert_eq!(windows.admit(&second), None);
        assert_eq!(windows.admit(&first), Some(DoorMode::Open));
        assert_eq!(windows.refused(&first), AfterRefusal::Ended);
    }

    // LA-23: opening a window past the cap ends the oldest token and no other.
    #[test]
    fn a_window_past_the_cap_ends_the_oldest() {
        let windows = Windows::default();
        let tokens: Vec<_> = (0..=MAX_WINDOWS)
            .map(|_| windows.issue(DoorMode::Open).unwrap())
            .collect();
        assert_eq!(windows.admit(&tokens[0]), None);
        for token in &tokens[1..] {
            assert_eq!(windows.admit(token), Some(DoorMode::Open));
        }
    }

    // LA-23: retiring ends every token.
    #[test]
    fn ending_all_ends_every_token() {
        let windows = Windows::default();
        let first = windows.issue(DoorMode::Open).unwrap();
        let second = windows.issue(DoorMode::Unlock).unwrap();
        windows.end_all();
        assert_eq!(windows.admit(&first), None);
        assert_eq!(windows.admit(&second), None);
    }

    // LA-20, LA-23: a presented token of another shape, or one never issued, is
    // not admitted.
    #[test]
    fn an_unknown_or_malformed_token_is_refused() {
        let windows = Windows::default();
        let token = windows.issue(DoorMode::Open).unwrap();
        let never_issued = Secret::draw().unwrap().encoded();
        for presented in ["", "door", &token[1..], &never_issued] {
            assert_eq!(windows.admit(presented), None, "{presented:?}");
        }
    }
}
