use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{any, get};
use axum::serve::ListenerExt;
use axum::{Json, Router};
use futures_util::{stream, StreamExt};
use serde_json::{json, Map, Value};
use tokio::net::TcpListener;

/// A server on a port of its own that counts the connections made to it, and
/// answers
///
/// - `/api/headers` with the request line and every header it was sent;
/// - `/api/echo` with the body it was sent;
/// - `/api/redirect` with a redirect;
/// - `/api/cookie` with a `Set-Cookie` and a CORS header;
/// - `/api/broken` with part of a body, and then breaks off;
/// - every other path with an empty `200`.
pub struct Upstream {
    pub address: SocketAddr,
    connections: Arc<AtomicUsize>,
}

impl Upstream {
    pub async fn start() -> Self {
        let app = Router::new()
            .route("/api/headers", any(what_was_sent))
            .route(
                "/api/echo",
                any(|body: Body| async move { Body::from_stream(body.into_data_stream()) }),
            )
            .route(
                "/api/redirect",
                get(|| async { Redirect::to("/elsewhere") }),
            )
            .route(
                "/api/cookie",
                get(|| async {
                    (
                        [
                            ("set-cookie", "session=from-the-server"),
                            ("access-control-allow-origin", "*"),
                            ("x-kept", "1"),
                        ],
                        "ok",
                    )
                }),
            )
            .route("/api/broken", get(broken_off))
            .fallback(|| async { StatusCode::OK });
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback port is free");
        let address = listener
            .local_addr()
            .expect("a bound listener has an address");
        let connections = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&connections);
        let listener = listener.tap_io(move |_| {
            counted.fetch_add(1, Ordering::SeqCst);
        });
        tokio::spawn(async move { axum::serve(listener, app).await });
        Self {
            address,
            connections,
        }
    }

    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }
}

async fn broken_off() -> Body {
    let first = stream::once(async { Ok::<_, io::Error>(Bytes::from_static(b"the first half")) });
    // Held back long enough for the status and the first piece to have gone,
    // so that what breaks is the body and not the answer.
    let broken = stream::once(async {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        Err(io::Error::other("the server stopped midway"))
    });
    Body::from_stream(first.chain(broken))
}

async fn what_was_sent(request: Request) -> Response {
    let mut headers = Map::new();
    for name in request.headers().keys() {
        let values: Vec<Value> = request
            .headers()
            .get_all(name)
            .iter()
            .map(|value| Value::from(value.to_str().expect("the cases send text")))
            .collect();
        headers.insert(name.to_string(), Value::Array(values));
    }
    Json(json!({
        "method": request.method().as_str(),
        "uri": request.uri().to_string(),
        "headers": headers,
    }))
    .into_response()
}
