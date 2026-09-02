use axum::http::{header, HeaderMap};

/// How many bytes the request said it was bringing, where it said.
///
/// The whole body and not the files alone — the multipart framing is in it — so
/// what it answers is a little more than what will be written. That is the right
/// way round for a fence: it asks this device to have room for slightly more
/// than the drop needs.
pub(super) fn declared_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}
