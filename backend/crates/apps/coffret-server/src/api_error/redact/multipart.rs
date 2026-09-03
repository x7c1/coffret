use axum::extract::multipart::MultipartError;

/// What the multipart extractor refused, as an event may carry it.
///
/// The status it decided on, which is the whole of what this server reads off
/// one: whether the body outran the limit the route is mounted with or was not
/// a multipart body at all. Its message is `axum`'s own account of a request
/// somebody else composed, and a part's filename is exactly the kind of thing
/// such an account may quote.
pub(crate) fn multipart(cause: &MultipartError) -> String {
    format!("Multipart(status={})", cause.status().as_u16())
}
