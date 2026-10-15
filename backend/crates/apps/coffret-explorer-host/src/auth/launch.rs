use std::sync::Arc;

use axum::body::to_bytes;
use axum::extract::{Request, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::de::IgnoredAny;
use serde::Serialize;

use super::MAX_BODY_BYTES;
use crate::credentials::ExchangeRefused;
use crate::fences::launch_ticket;
use crate::forward::unpinned_answer;
use crate::host::Host;
use crate::refusal::Refusal;
use crate::READ_DEADLINE;

/// What an exchange answers the page with (spec: LA-16).
#[derive(Serialize)]
struct Launched<'a> {
    token: &'a str,
    expires_in: u64,
    instance_id: String,
}

/// `POST /auth/launch`: exchanges the launch ticket in `Authorization: Launch`
/// for a browser token (spec: LA-16, LA-17).
///
/// The ticket is looked at only once the request is known to be one the page
/// sends: JSON, within its bound, arrived in time. The key the new session's
/// generation is forwarded with is pinned as the session is made, so that a
/// server that restarts after this is not one its token is forwarded to
/// (spec: LA-15).
pub(super) async fn launch(State(host): State<Arc<Host>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let headers = &parts.headers;
    if !is_json(headers) {
        return Refusal::NotJsonContent.answer("content_type", headers);
    }
    let Some(ticket) = launch_ticket(headers) else {
        return Refusal::AuthenticationRequired.answer("ticket", headers);
    };
    let body = match tokio::time::timeout(READ_DEADLINE, to_bytes(body, MAX_BODY_BYTES)).await {
        Ok(Ok(body)) => body,
        // Past the bound, or broken off; either way not a body this takes.
        Ok(Err(_)) => return Refusal::BodyTooLarge.answer("body_bytes", headers),
        Err(_) => return Refusal::TooSlow.answer("read_deadline", headers),
    };
    if serde_json::from_slice::<IgnoredAny>(&body).is_err() {
        return Refusal::NotJson.answer("body", headers);
    }

    let exchanged = match host.gate.exchange(ticket) {
        Ok(exchanged) => exchanged,
        Err(ExchangeRefused::Unauthenticated) => {
            return Refusal::AuthenticationRequired.answer("ticket", headers)
        }
        Err(ExchangeRefused::RateLimited { retry_after }) => {
            return Refusal::RateLimited(retry_after).answer("exchange_rate", headers)
        }
        Err(ExchangeRefused::NotDrawn(_)) => return Refusal::NotDrawn.answer("csprng", headers),
    };
    if let Some(refused) = unpinned_answer(&host, &exchanged.instance_id, headers) {
        return refused;
    }
    let token = exchanged.token.encoded();
    Json(Launched {
        token: token.as_str(),
        expires_in: exchanged.expires_in.as_secs(),
        instance_id: exchanged.instance_id.to_string(),
    })
    .into_response()
}

/// Whether the request says its body is JSON.
fn is_json(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all(CONTENT_TYPE).iter();
    let (Some(value), None) = (values.next(), values.next()) else {
        return false;
    };
    value
        .to_str()
        .ok()
        .and_then(|value| value.split(';').next())
        .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
}
