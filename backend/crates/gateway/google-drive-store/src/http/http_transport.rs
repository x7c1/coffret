use async_trait::async_trait;

use crate::http::http_request::HttpRequest;
use crate::http::http_response::HttpResponse;
use crate::http::transport_error::TransportError;

/// Whatever turns an [`HttpRequest`] into an [`HttpResponse`].
///
/// The Drive gateway holds one of these rather than an HTTP client, and it is a
/// constructor argument rather than something chosen at build time. That is
/// what makes the parts worth testing testable: which failures are worth
/// retrying, that a 401 costs exactly one token refresh, that a digest Drive
/// disagrees with fails the upload. All of it is exercised by handing the
/// gateway a transport that answers from a script, with no network involved and
/// no second code path that only exists under `cfg(test)`.
#[async_trait]
pub trait HttpTransport: Send + Sync {
    /// Makes one call.
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError>;
}
