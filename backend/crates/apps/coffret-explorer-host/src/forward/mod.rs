//! `/api`, forwarded to the server with the key it admits callers by.

mod headers;

mod not_answering;
use not_answering::not_answering;

mod remove_hop_by_hop;
use remove_hop_by_hop::remove_hop_by_hop;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::HeaderValue;
use axum::response::Response;
use futures_util::TryStreamExt;

use crate::Config;

pub(crate) struct Forwarder {
    client: reqwest::Client,
    server: SocketAddr,
    /// `<server>`, the only `Host` the server accepts.
    server_host: HeaderValue,
    /// `http://<server>`, which is what the server's own origin is.
    server_origin: HeaderValue,
    key_file: PathBuf,
}

impl Forwarder {
    pub(crate) fn new(config: Config) -> reqwest::Result<Self> {
        let client = reqwest::Client::builder()
            // A redirect is the server's answer, for the page to act on; one
            // followed here would be a second request the page never made.
            .redirect(reqwest::redirect::Policy::none())
            // The server is on this device's loopback interface. A proxy named
            // in the environment would carry the key off it.
            .no_proxy()
            .build()?;
        let server_host = HeaderValue::from_str(&config.server.to_string())
            .expect("a socket address is a valid header value");
        let server_origin = HeaderValue::from_str(&format!("http://{}", config.server))
            .expect("a socket address is a valid header value");
        Ok(Self {
            client,
            server: config.server,
            server_host,
            server_origin,
            key_file: config.key_file,
        })
    }
}

/// Forwards one request under `/api` to the server and hands its answer back.
pub(crate) async fn forward(State(host): State<Arc<Forwarder>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map_or("/", |path_and_query| path_and_query.as_str());
    let url = format!("http://{}{}", host.server, path_and_query);
    let headers = host.headers(parts.headers);

    let sent = host
        .client
        .request(parts.method, url)
        .headers(headers)
        .body(reqwest::Body::wrap_stream(body.into_data_stream()))
        .send()
        .await;
    let answer = match sent {
        Ok(answer) => answer,
        Err(cause) => return not_answering(host.server, &cause),
    };

    let status = answer.status();
    let mut headers = answer.headers().clone();
    remove_hop_by_hop(&mut headers);
    let server = host.server;
    // A failure midway is met after the status has gone, so the browser sees
    // only a body cut short; this is the one place it can still be said.
    let body = answer.bytes_stream().inspect_err(move |cause| {
        tracing::warn!(%server, %cause, "the server's answer broke off midway");
    });
    let mut response = Response::new(Body::from_stream(body));
    *response.status_mut() = status;
    *response.headers_mut() = headers;
    response
}
