//! Dropping files onto a folder, as a browser sends them.

use axum::body::Body;
use axum::http::Response;

use super::{asking, Served};

impl Served {
    /// Drops files onto one folder, as a browser sends them.
    ///
    /// Each part carries its path relative to the folder as its filename, which
    /// is what a plain file drop and a folder drop both look like on the wire.
    pub async fn upload(&self, folder: &str, parts: &[(&str, &[u8])]) -> Response<Body> {
        self.dropped(folder, parts, false, Declares::Length).await
    }

    /// The same, as a book being brought into a folder made for it.
    ///
    /// One parameter apart from an ordinary drop, and the whole of the
    /// difference on the wire: what it arms is a freeze of that folder rather
    /// than a sync (spec: PK-17).
    pub async fn upload_book(&self, folder: &str, parts: &[(&str, &[u8])]) -> Response<Body> {
        self.dropped(folder, parts, true, Declares::Length).await
    }

    /// The same drop, saying nothing about how much is coming.
    ///
    /// What a caller streaming its body looks like from here: no
    /// `Content-Length`, which every browser sending a `FormData` does send. It
    /// is not refused for that, and what the room fence asks for instead is one
    /// part's ceiling (spec: LA-9, LA-11).
    pub async fn upload_undeclared(&self, folder: &str, parts: &[(&str, &[u8])]) -> Response<Body> {
        self.dropped(folder, parts, false, Declares::Nothing).await
    }

    async fn dropped(
        &self,
        folder: &str,
        parts: &[(&str, &[u8])],
        freeze: bool,
        declares: Declares,
    ) -> Response<Body> {
        let mut body: Vec<u8> = Vec::new();
        for (name, content) in parts {
            body.extend_from_slice(
                format!(
                    "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; \
                     filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(content);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());

        let mut uri = match folder {
            "" => "/api/upload".to_owned(),
            named => format!("/api/upload?path={named}"),
        };
        if freeze {
            uri.push_str(match folder {
                "" => "?freeze=true",
                _ => "&freeze=true",
            });
        }
        let request = asking("POST", &uri).header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        );
        let request = match declares {
            // Said, because a browser sending a `FormData` says it, and the
            // server's room fence reads it: without it every drop here would
            // be asking this device for the room one whole part could take
            // rather than for the room this drop needs.
            Declares::Length => request.header("content-length", body.len()),
            Declares::Nothing => request,
        };
        self.send(
            request
                .body(Body::from(body))
                .expect("a multipart request is well formed"),
        )
        .await
    }
}

/// What every multipart body a case sends is delimited by.
const BOUNDARY: &str = "coffret-case-boundary";

/// Whether a drop says how much it is bringing.
///
/// The one header the room fence reads, and the only difference between a
/// browser's `FormData` and a caller streaming its body (spec: LA-11).
#[derive(Clone, Copy)]
enum Declares {
    /// A `Content-Length`, as every browser sends.
    Length,
    /// Nothing, as a streamed body carries.
    Nothing,
}
