use axum::extract::multipart::MultipartError;
use axum::http::StatusCode;
use coffret_device::Error;

use crate::api_error::ApiError;

/// What a part was not taken for, and how far that reaches.
pub(super) enum Refusal {
    /// About this file alone, and the drop goes on: one file the Library will
    /// not have is not the file beside it.
    Part(ApiError),
    /// About the request, which stops here: whatever made it is no truer of the
    /// next part than of this one, so there is nothing to be gained by reading
    /// the rest. A budget this request has outrun, room this device has not for
    /// what is still coming or could not be asked about at all, a stream that
    /// broke before the next part, the state of a mapped root the parts are
    /// going through — one condition gathers them, and it is what the next of
    /// them is judged by.
    ///
    /// That condition is what the variant is chosen by, rather than the wire
    /// kind the refusal goes out under: a refused root reaches a page as
    /// `declined` the way a declined placement does (spec: EP-13), and a root
    /// this device could not read its own marker in reaches it as `server` the
    /// way this machine's other failures do — and both are about the whole
    /// drop.
    Request(ApiError),
    /// Not a refusal at all: the body stopped arriving while this part was
    /// being read.
    ///
    /// Nothing about the part or the request was decided, so neither of the
    /// variants above says it truly. What it is instead is the failure the
    /// route's own loop meets when the stream breaks between two parts, met a
    /// little earlier — so it is handed back to that loop as the failure it is,
    /// and the loop says it the one way it says its own. A refusal of this part
    /// would be recorded as a part somebody refused, which nobody did.
    Interrupted(MultipartError),
}

impl Refusal {
    /// What a failure to read the next chunk of a part comes to.
    ///
    /// Two different things arrive as the same error type, and axum's reading
    /// of the `multer` error under it is what tells them apart. A body that
    /// could not be read as multipart — a boundary that is not one, a part that
    /// ends before its data does — or one that ran past the body limit is the
    /// request being refused, a `400` or a `413` it has earned wherever in it
    /// that was found. A body whose stream failed — the connection went while a
    /// chunk was in flight — is `multer`'s `StreamReadFailed` over something
    /// that is not a limit, and axum reads that alone as `500`: nothing about
    /// what was sent is wrong with it, it stopped being sent.
    pub(super) fn reading(cause: MultipartError) -> Self {
        match cause.status() {
            StatusCode::INTERNAL_SERVER_ERROR => Self::Interrupted(cause),
            _ => Self::Request(ApiError::multipart(cause)),
        }
    }
}

/// Anything that goes wrong about one file is about that one file, so `?` on it
/// says so and the drop carries on.
///
/// Which is what keeps the other variant honest: an [`ApiError`]'s kinds are
/// the wire's, and they do not divide by reach — `declined` is what a refused
/// root and a refused part both go out under — so nothing in one tells this type
/// how far it reaches. This is therefore the conversion that can never make a
/// refusal about the request, and every place that decides the reach from the
/// request itself — a budget it has outrun, the room this device has, a body
/// that could not be read — writes [`Request`](Self::Request) out in full.
///
/// The conversion below is the one exception, and it is one because it has a
/// failure kind rather than a wire kind to read: a device error whose kind
/// settles the reach wherever it is met. So `?` on one of those may stop the
/// request, and its two arms are the whole of where that is decided.
impl From<ApiError> for Refusal {
    fn from(refusal: ApiError) -> Self {
        Self::Part(refusal)
    }
}

impl From<Error> for Refusal {
    fn from(cause: Error) -> Self {
        match cause {
            // The root the parts of this drop go into: in a drop onto a folder
            // it is one root for all of them, and at the Library root, where
            // the parts carry mappings of their own (spec: EP-9), it is the
            // first of those to be refused. Either way nothing is placed into
            // it and the request fails as a whole, the way a declined placement
            // fails a single writer's (spec: EP-11, EP-13).
            //
            // And the same root when nothing could be learned about it: a
            // permission the process has not on its marker is a fact about the
            // folder every part is going through, settled before the first of
            // them was read. Reported as one part's business the drop would
            // read the next part, meet the refusal again, and report it once
            // per file for a condition none of them caused. It is not a verdict
            // on the mapping and nothing here makes it one: where a refused root
            // tells the page which mapping to record again, this one tells it
            // only that the server could not answer, and the folder the disk
            // would not answer about stays out of the page and out of the record
            // alike (spec: EL-1, EP-11, EP-13).
            refused @ (Error::RootRefused(_) | Error::RootUnvouched { .. }) => {
                Self::Request(refused.into())
            }
            other => Self::Part(other.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use axum::body::{Body, Bytes};
    use axum::extract::multipart::MultipartError;
    use axum::extract::{FromRequest, Multipart};
    use axum::http::Request;
    use futures_util::stream;
    use tokio::sync::mpsc;

    use super::Refusal;

    /// The opening of one part, as a browser sends it, and none of its end.
    const PART_HEAD: &str = "--b\r\nContent-Disposition: form-data; name=\"file\"; \
                             filename=\"a.txt\"\r\nContent-Type: text/plain\r\n\r\nsome bytes";

    /// What reading one part's chunks meets, once its headers have arrived and
    /// the body then does `then`: fails, or — given nothing — simply ends.
    ///
    /// Fed through a channel rather than laid out in advance, because the
    /// failure has to arrive after the part has been handed over: a body that
    /// fails before its first part's headers are read is a failure of the
    /// route's own loop, which is not what these cases are about.
    async fn met_reading(then: Option<io::Error>) -> MultipartError {
        let (send, receive) = mpsc::unbounded_channel::<Result<Bytes, io::Error>>();
        send.send(Ok(Bytes::from_static(PART_HEAD.as_bytes())))
            .expect("the body is still being read");
        let body = Body::from_stream(stream::unfold(receive, |mut receive| async move {
            receive.recv().await.map(|item| (item, receive))
        }));
        let request = Request::builder()
            .method("POST")
            .uri("/api/upload")
            .header("content-type", "multipart/form-data; boundary=b")
            .body(body)
            .expect("a multipart request is well formed");
        let mut parts = Multipart::from_request(request, &())
            .await
            .expect("a multipart request is taken as one");
        let mut part = parts
            .next_field()
            .await
            .expect("the part's headers arrived whole")
            .expect("there is a part");
        match then {
            Some(failure) => send
                .send(Err(failure))
                .expect("the body is still being read"),
            None => drop(send),
        }
        loop {
            match part.chunk().await {
                Ok(Some(_)) => {}
                Ok(None) => panic!("the part was meant to end in a failure, and ended"),
                Err(cause) => return cause,
            }
        }
    }

    // A connection that went while a chunk was in flight is not a refusal of
    // anything: nothing about the part or the request was decided. It is handed
    // back as what it is, so that the part is never recorded as refused.
    #[tokio::test]
    async fn a_body_that_stopped_arriving_mid_part_is_no_refusal() {
        let went = io::Error::new(io::ErrorKind::ConnectionReset, "the connection went");

        assert!(matches!(
            Refusal::reading(met_reading(Some(went)).await),
            Refusal::Interrupted(_)
        ));
    }

    // A body that arrived whole and ends before its part does is a request that
    // cannot be read, and that is a refusal of the request — the one it always
    // was.
    #[tokio::test]
    async fn a_body_that_ends_mid_part_is_a_refused_request() {
        match Refusal::reading(met_reading(None).await) {
            Refusal::Request(refusal) => assert_eq!(refusal.kind(), "bad_request"),
            _ => panic!("a truncated body is the request being refused"),
        }
    }
}
