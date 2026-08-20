use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::http::http_request::HttpRequest;
use crate::http::http_response::HttpResponse;
use crate::http::http_transport::HttpTransport;
use crate::http::recorded_request::RecordedRequest;
use crate::http::request_body::RequestBody;
use crate::http::stub_answer::StubAnswer;
use crate::http::transport_error::TransportError;

/// A transport that answers from a script and remembers what it was asked.
///
/// It is the whole reason the gateway takes its transport as an argument: a 429,
/// a 5xx, a connection that times out, a digest that disagrees with the bytes
/// sent — none of them can be provoked against the real API on demand, and all
/// of them are exactly what the adapter has to get right.
pub struct StubTransport {
    answers: Mutex<VecDeque<StubAnswer>>,
    requests: Mutex<Vec<RecordedRequest>>,
}

impl StubTransport {
    /// Scripts a transport to answer these, in order.
    pub fn new(answers: impl IntoIterator<Item = StubAnswer>) -> Arc<Self> {
        Arc::new(Self {
            answers: Mutex::new(answers.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        })
    }

    /// How many calls have been made.
    pub fn call_count(&self) -> usize {
        self.requests.lock().expect("no test panics here").len()
    }

    /// Looks at one of the calls made.
    ///
    /// The call stays recorded, so a test can look at several of them and an
    /// index keeps meaning the position the call was made in.
    pub fn request(&self, index: usize) -> RecordedRequest {
        let requests = self.requests.lock().expect("no test panics here");
        assert!(
            index < requests.len(),
            "no call {index} was made; there were {}",
            requests.len()
        );
        requests[index].clone()
    }
}

#[async_trait]
impl HttpTransport for StubTransport {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        // Draining the body is what a real transport does, and the upload path
        // depends on it: the digest is computed from the bytes as they are read.
        let body = match request.body {
            RequestBody::Empty => Vec::new(),
            RequestBody::Bytes(bytes) => bytes,
            RequestBody::Stream(stream) => stream
                .into_bytes()
                .await
                .expect("a scripted upload streams what it says it will"),
        };

        self.requests
            .lock()
            .expect("no test panics here")
            .push(RecordedRequest {
                method: request.method,
                url: request.url,
                headers: request.headers,
                body,
            });

        match self
            .answers
            .lock()
            .expect("no test panics here")
            .pop_front()
        {
            Some(StubAnswer::Respond(response)) => Ok(response),
            Some(StubAnswer::Fail(error)) => Err(error),
            None => panic!("the gateway made more calls than the test scripted answers for"),
        }
    }
}
