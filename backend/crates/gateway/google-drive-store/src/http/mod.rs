//! The HTTP seam the Drive gateway is built on.
//!
//! The gateway makes requests and reads responses; it does not own a client.
//! What performs the call is an [`HttpTransport`] handed to its constructor.

mod http_request;
pub use http_request::HttpRequest;

mod http_response;
pub use http_response::HttpResponse;

mod http_transport;
pub use http_transport::HttpTransport;

mod method;
pub use method::Method;

#[cfg(test)]
mod recorded_request;
#[cfg(test)]
pub use recorded_request::RecordedRequest;

mod request_body;
pub use request_body::RequestBody;

mod reqwest_transport;
pub use reqwest_transport::ReqwestTransport;

#[cfg(test)]
mod stub_answer;
#[cfg(test)]
pub use stub_answer::StubAnswer;

#[cfg(test)]
mod stub_transport;
#[cfg(test)]
pub use stub_transport::StubTransport;

mod transport_error;
pub use transport_error::TransportError;
