//! Asking the routes, as the service they are.

use axum::body::Body;
use axum::http::{Request, Response};
use tower::ServiceExt;

use super::{asking, Served};

impl Served {
    /// Asks one route, as the service it is.
    pub async fn get(&self, uri: &str) -> Response<Body> {
        self.send(
            asking("GET", uri)
                .body(Body::empty())
                .expect("a request with no body is well formed"),
        )
        .await
    }

    /// Asks one route twice at once.
    pub async fn get_twice(&self, uri: &str) -> (Response<Body>, Response<Body>) {
        tokio::join!(self.get(uri), self.get(uri))
    }

    /// Posts to one route, as the service it is.
    pub async fn post(&self, uri: &str) -> Response<Body> {
        self.send(
            asking("POST", uri)
                .body(Body::empty())
                .expect("a request with no body is well formed"),
        )
        .await
    }

    /// Posts a JSON body to one route, as the explorer sends one.
    pub async fn post_json(&self, uri: &str, body: &serde_json::Value) -> Response<Body> {
        self.send(
            asking("POST", uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("a request with a JSON body is well formed"),
        )
        .await
    }

    /// Drives the router with a request a case built for itself.
    ///
    /// For the cases about who is answered at all, which are the only ones that
    /// have anything to say about the headers: everything else asks through
    /// [`asking`], which sends what the explorer sends.
    pub async fn send(&self, request: Request<Body>) -> Response<Body> {
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("the router answers every request")
    }
}
