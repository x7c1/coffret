use serde::Serialize;

use crate::reconnect::Reconnect;

/// Where the last reconnect stands, and the sentence a person reads about it.
///
/// No URL, even while a flow waits: the consent page goes back to the request
/// that pressed and to nobody polling, so a second tab asking what the server
/// is doing is told a page is open and not handed one to open.
#[derive(Serialize)]
pub(super) struct ReconnectDto {
    /// `waiting`, `renewed`, `refused`, `timed_out` or `failed`.
    state: &'static str,
    /// One sentence a person could read.
    message: &'static str,
}

impl ReconnectDto {
    pub(super) fn of(reconnect: &Reconnect) -> Self {
        let (state, message) = match reconnect {
            Reconnect::Waiting { .. } => (
                "waiting",
                "waiting for Google's consent page to be answered in the tab that opened",
            ),
            Reconnect::Renewed => (
                "renewed",
                "Google Drive's permission for this device was renewed",
            ),
            Reconnect::Refused => (
                "refused",
                "the consent page was declined, so Google Drive's permission was not renewed",
            ),
            Reconnect::TimedOut => (
                "timed_out",
                "the consent page was not answered in time, so Google Drive's permission was \
                 not renewed",
            ),
            Reconnect::Failed => (
                "failed",
                "Google Drive's permission could not be renewed — the server's log says why",
            ),
        };
        Self { state, message }
    }
}
