/// What kind of failure a forwarded request met, as one fixed word.
///
/// Read off the error's predicates and nothing else. A `reqwest::Error`'s
/// `Display` carries the request's URL, and the query of a request for
/// `/api/file` holds an Entry Path, so neither it nor anything under it in the
/// source chain is what an event says (spec: EL-1, spec: EL-4): an event is
/// built from this word, the operation, and the server's address alone.
pub(super) fn failure_kind(error: &reqwest::Error) -> &'static str {
    // A connection that timed out is both; the timeout is the more telling.
    if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect"
    } else if error.is_body() {
        "body"
    } else if error.is_decode() {
        "decode"
    } else if error.is_status() {
        "status"
    } else if error.is_redirect() {
        "redirect"
    } else if error.is_upgrade() {
        "upgrade"
    } else if error.is_builder() {
        "builder"
    } else if error.is_request() {
        "request"
    } else {
        "other"
    }
}
