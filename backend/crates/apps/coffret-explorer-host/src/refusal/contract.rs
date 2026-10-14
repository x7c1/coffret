//! Every refusal the host can send in place of the server's answer, written out
//! as the wire carries it, for the explorer's own cases to read back through
//! its types — as the server's own refusals are (`coffret-server`'s
//! `api_error/contract.rs`, whose file sits beside this one's).
//!
//! A word this side grows or renames fails here until the file is brought
//! along, and fails in the explorer's `contract.test.ts` until it has a word
//! for it or reads it as one it does not know.

use std::path::Path;
use std::time::Duration;

use axum::http::HeaderMap;
use axum::response::Response;

use super::Refusal;
use crate::forward::not_answering_answer;

/// Where the explorer reads the host's refusals from, relative to this crate.
const REFUSALS: &str = "../../../../frontend/packages/gateway/api/src/contract/host-refusals.json";

/// What rewrites the committed file instead of comparing against it, as on the
/// server's side.
const WRITE: &str = "COFFRET_WRITE_CONTRACT";

/// Every refusal, in the order [`Refusal`] declares them, then the two answers
/// a request that could not be forwarded gets.
fn every_answer() -> Vec<Response> {
    let none = HeaderMap::new();
    let mut answers: Vec<Response> = [
        Refusal::OriginRefused,
        Refusal::AuthenticationRequired,
        Refusal::LibraryLocked,
        Refusal::RateLimited(Duration::from_millis(1500)),
        Refusal::SessionLimit,
        Refusal::NotFound,
        Refusal::MethodNotAllowed,
        Refusal::NotJson,
        Refusal::NotJsonContent,
        Refusal::BodyTooLarge,
        Refusal::HeadersTooLarge,
        Refusal::TooSlow,
        Refusal::ServerUnavailable,
        Refusal::NotDrawn,
    ]
    .into_iter()
    .map(|refusal| refusal.answer("contract", &none))
    .collect();
    let server = "127.0.0.1:8787".parse().expect("an address");
    answers.push(not_answering_answer(server, true, "connect"));
    answers.push(not_answering_answer(server, false, "timeout"));
    answers
}

async fn on_the_wire(response: Response) -> serde_json::Value {
    let status = response.status().as_u16();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a refusal's body is in memory");
    let body: serde_json::Value = serde_json::from_slice(&body).expect("a refusal's body is JSON");
    serde_json::json!({ "body": body, "status": status })
}

// The explorer's half of the contract reads this file; this is what holds the
// file to the host.
#[tokio::test]
async fn the_refusals_the_explorer_reads_are_the_ones_this_host_sends() {
    let mut written = Vec::new();
    for answer in every_answer() {
        written.push(on_the_wire(answer).await);
    }
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::Value::Array(written))
            .expect("a JSON value renders")
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(REFUSALS);
    if std::env::var_os(WRITE).is_some() {
        std::fs::write(&path, rendered)
            .unwrap_or_else(|cause| panic!("{} must be writable: {cause}", path.display()));
        return;
    }
    let held = std::fs::read_to_string(&path)
        .unwrap_or_else(|cause| panic!("{} must be readable: {cause}", path.display()));
    assert!(
        held == rendered,
        "{} is not what this host sends. If the change means to alter the wire, run the case \
         again with {WRITE}=1 and read the file's diff as what the explorer will now receive — \
         then make its `contract.test.ts` pass over it. What this host sends now:\n{rendered}",
        path.display(),
    );
}
