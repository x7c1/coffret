use serde::Serialize;

use crate::reconnect::Reconnect;

/// Where the last reconnect stands, and the sentence a person reads about it.
///
/// No URL, even while a flow waits: the consent page goes back to the request
/// that pressed and to nobody polling, so a second tab asking what the server
/// is doing is told a page is open and not handed one to open.
///
/// What a waiting flow carries instead is `flow`, which names it without
/// giving its page away. One flow can end and another begin between two
/// polls, and the state alone reads `waiting` both times: a window holding the
/// page its own press was answered with compares that page's [`flow_of`] with
/// this one, and drops its link once they differ.
#[derive(Serialize)]
pub(super) struct ReconnectDto {
    /// `waiting`, `renewed`, `refused`, `timed_out` or `failed`.
    state: &'static str,
    /// One sentence a person could read.
    message: &'static str,
    /// The waiting flow's name, which is [`flow_of`] its consent page: present
    /// while it waits, and absent once it has ended.
    #[serde(skip_serializing_if = "Option::is_none")]
    flow: Option<String>,
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
        let flow = match reconnect {
            Reconnect::Waiting { url } => Some(flow_of(url)),
            _ => None,
        };
        Self {
            state,
            message,
            flow,
        }
    }
}

/// The name a waiting flow goes by in the work answer: the 32-bit FNV-1a digest
/// of its consent page's UTF-8 bytes, as eight lowercase hex digits.
///
/// Drawn from the page rather than counted, because the page is what a window
/// holds and what its link leads to: two flows are told apart exactly where
/// their pages differ, and every real page does — each carries a `state` and a
/// challenge of its own (spec: SA-1, SA-2). The digest gives the page back to
/// no one; the explorer computes the same one over the page it was answered
/// with (`flowOf` in the web app), and the two are held to each other by the
/// waiting sample in the frontend gateway's `contract/work.json`.
fn flow_of(url: &str) -> String {
    let digest = url.bytes().fold(0x811c_9dc5_u32, |digest, byte| {
        (digest ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    format!("{digest:08x}")
}

#[cfg(test)]
mod tests {
    use super::flow_of;

    // The digest the explorer's `flowOf` is pinned to as well: the two sides
    // agree on the page's name, or a window's link never matches its flow.
    #[test]
    fn names_a_page_by_its_fnv_1a_digest() {
        assert_eq!(flow_of(""), "811c9dc5");
        assert_eq!(flow_of("https://consent.example/"), "c9ebbe11");
        assert_ne!(
            flow_of("https://consent.example/a"),
            flow_of("https://consent.example/b"),
        );
    }
}
