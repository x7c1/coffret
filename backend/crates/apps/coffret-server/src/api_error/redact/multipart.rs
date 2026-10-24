use axum::extract::multipart::MultipartError;
use coffret_device::Redacted;

/// What the multipart extractor refused, kept as the value it refused with.
///
/// An event is told the status it decided on, which is the whole of what this
/// server reads off one: whether the body outran the limit the route is mounted
/// with or was not a multipart body at all. Its message is `axum`'s own account
/// of a request somebody else composed, and a part's filename is exactly the
/// kind of thing such an account may quote.
pub(crate) struct Multipart(pub(crate) MultipartError);

impl Redacted for Multipart {
    fn redacted(&self) -> String {
        format!("Multipart(status={})", self.0.status().as_u16())
    }
}
