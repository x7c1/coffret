use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;

use super::Refusal;

/// The Passphrase page, which reaches the door with `fetch` and nothing else
/// (spec: LA-22). Compiled in, so that the page a window loads is the one this
/// build carries.
const INDEX: &str = include_str!("page/index.html");
const SCRIPT: &str = include_str!("page/unlock.js");
const STYLE: &str = include_str!("page/unlock.css");

pub(super) async fn index() -> impl IntoResponse {
    ([(CONTENT_TYPE, "text/html; charset=utf-8")], INDEX)
}

pub(super) async fn script() -> impl IntoResponse {
    ([(CONTENT_TYPE, "text/javascript; charset=utf-8")], SCRIPT)
}

pub(super) async fn style() -> impl IntoResponse {
    ([(CONTENT_TYPE, "text/css; charset=utf-8")], STYLE)
}

pub(super) async fn not_found() -> impl IntoResponse {
    Refusal::NotFound.answer("path", false)
}
