//! A book drop whose request stays open while a case acts on the server.

use axum::body::{Body, Bytes};
use axum::http::Response;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::drops::BOUNDARY;

/// A book drop whose request is still open (see
/// [`Served::upload_book_streamed`](super::Served::upload_book_streamed)).
pub struct StreamedDrop {
    folder: String,
    parts: mpsc::UnboundedSender<Result<Bytes, std::io::Error>>,
    answer: JoinHandle<Response<Body>>,
}

impl StreamedDrop {
    /// The drop
    /// [`Served::upload_book_streamed`](super::Served::upload_book_streamed)
    /// started, whose remaining parts go through `parts` and whose answer
    /// `answer` waits for.
    pub(super) fn new(
        folder: String,
        parts: mpsc::UnboundedSender<Result<Bytes, std::io::Error>>,
        answer: JoinHandle<Response<Body>>,
    ) -> Self {
        Self {
            folder,
            parts,
            answer,
        }
    }

    /// Puts one more file of the book on the wire, `name` relative to the
    /// folder, and says where it lands.
    pub fn send(&self, name: &str, content: &[u8]) -> String {
        let mut part = format!(
            "\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .into_bytes();
        part.extend_from_slice(content);
        part.extend_from_slice(format!("\r\n--{BOUNDARY}").as_bytes());
        self.parts
            .send(Ok(Bytes::from(part)))
            .expect("the request is still reading");
        format!("{}/{name}", self.folder)
    }

    /// Ends the body, and answers with what the route said.
    pub async fn finish(self) -> Response<Body> {
        self.parts
            .send(Ok(Bytes::from_static(b"--\r\n")))
            .expect("the request is still reading");
        drop(self.parts);
        self.answer.await.expect("the request does not panic")
    }

    /// Stops the request where it stands, as a process ending mid-drop does:
    /// nothing after it runs, the release of the reservation included.
    /// Stops sending without ending the body, and waits for the answer: what
    /// a request a page stopped looks like from the server, whose parts stop
    /// arriving.
    pub async fn answered(self) -> Response<Body> {
        drop(self.parts);
        self.answer.await.expect("the request does not panic")
    }

    pub fn abandon(self) {
        self.answer.abort();
    }
}
