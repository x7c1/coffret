/// Keeps the part of a URL that says which endpoint was called.
///
/// The query string goes, whole. Drive answers an upload by handing back a
/// resumable session URI whose `upload_id` is a capability — whoever holds it
/// can write to that upload — and a query string is where a provider puts that
/// kind of value in general. It is not a token by name, but it grants access,
/// and the never-list is about what grants access.
///
/// What is left is what the record was for: which endpoint the call went to.
/// The identifiers that matter for reading a log afterwards — the object name,
/// the operation — are on the event as fields of their own, so nothing is lost
/// by dropping the query.
pub fn url(url: &str) -> &str {
    match url.split_once('?') {
        Some((endpoint, _)) => endpoint,
        None => url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_upload_session_uri_keeps_its_endpoint_and_loses_its_capability() {
        let session = "https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&upload_id=AEnB2Uo-secret";

        assert_eq!(
            url(session),
            "https://www.googleapis.com/upload/drive/v3/files",
        );
    }

    #[test]
    fn a_url_with_nothing_in_its_query_is_left_as_it_is() {
        let file = "https://www.googleapis.com/drive/v3/files/1a2B3c";

        assert_eq!(url(file), file);
    }
}
