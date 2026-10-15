use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{Method, StatusCode};
use axum::response::Response;
use axum::Router;
use coffret_explorer_host::credentials::{Credentials, SystemClock};
use coffret_explorer_host::{router, Config, KeySource, PageSource};
use tokio::net::TcpListener;
use tower::ServiceExt;

use super::{json_of, KeyDouble, LockDouble, Upstream};

/// One host, and everything a case reads about it.
pub struct Harness {
    pub router: Router,
    pub credentials: Credentials,
    pub key: Arc<KeyDouble>,
    pub lock: Arc<LockDouble>,
    pub upstream: Upstream,
    /// `127.0.0.1:<port>`, as a `Host` spells it.
    pub authority: String,
    /// `http://127.0.0.1:<port>`, as an `Origin` spells it.
    pub origin: String,
    /// Held so that the port stays the host's for as long as the case runs.
    pub listener: TcpListener,
}

impl Harness {
    pub async fn start() -> Self {
        Self::in_front_of(Upstream::start().await).await
    }

    /// A host in front of an upstream of the case's choosing.
    pub async fn in_front_of(upstream: Upstream) -> Self {
        let key = KeyDouble::new();
        Self::reading(upstream, key.clone(), key).await
    }

    /// A host that reads the server key from `source`; `key` is what the case
    /// reads the count of reads from, and is not read by the host unless it is
    /// `source` too.
    pub async fn reading(
        upstream: Upstream,
        source: Arc<dyn KeySource>,
        key: Arc<KeyDouble>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback port is free");
        let authority = listener.local_addr().unwrap().to_string();
        let lock = Arc::new(LockDouble::default());
        let credentials = Credentials::new(Arc::new(SystemClock)).expect("the CSPRNG answers");
        let router = router(
            &listener,
            Config {
                server: upstream.address,
                key: source,
                lock: lock.clone(),
                page: PageSource::Embedded,
            },
            credentials.gate(),
        )
        .expect("the host starts");
        Self {
            router,
            credentials,
            key,
            lock,
            upstream,
            origin: format!("http://{authority}"),
            authority,
            listener,
        }
    }

    /// A request to `uri` naming the host's own authority, and nothing else.
    pub fn request(&self, method: Method, uri: &str) -> axum::http::request::Builder {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("host", &self.authority)
    }

    /// The request the explorer's own page sends: its authority, its origin,
    /// and `same-origin`.
    pub fn page_request(&self, method: Method, uri: &str) -> axum::http::request::Builder {
        self.request(method, uri)
            .header("origin", &self.origin)
            .header("sec-fetch-site", "same-origin")
    }

    pub async fn send(&self, request: Request) -> Response {
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("a router never fails")
    }

    /// A launch ticket, issued as the app issues one.
    pub fn ticket(&self) -> String {
        self.credentials
            .issue_ticket()
            .expect("the CSPRNG answers")
            .encoded()
            .to_string()
    }

    /// `POST /auth/launch` with `ticket`, as the page sends it.
    pub fn launch(&self, ticket: &str) -> Request {
        self.page_request(Method::POST, "/auth/launch")
            .header("authorization", format!("Launch {ticket}"))
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap()
    }

    /// A browser token, exchanged for a fresh ticket.
    pub async fn token(&self) -> String {
        let response = self.send(self.launch(&self.ticket())).await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "a fresh ticket exchanges"
        );
        json_of(response).await["token"]
            .as_str()
            .expect("an exchange answers a token")
            .to_owned()
    }

    /// A request the page sends with `token`.
    pub fn with_token(
        &self,
        method: Method,
        uri: &str,
        token: &str,
    ) -> axum::http::request::Builder {
        self.page_request(method, uri)
            .header("authorization", format!("Bearer {token}"))
    }
}
