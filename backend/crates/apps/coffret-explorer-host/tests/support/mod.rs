//! What the host's cases drive it with: the real router, built around a real
//! loopback listener, in front of doubles that count what reaches them.
//!
//! Each case's binary uses part of this, so what one leaves unused is not dead.
#![allow(dead_code)]

mod harness;
pub use harness::Harness;

mod key_double;
pub use key_double::KeyDouble;

mod lock_double;
pub use lock_double::LockDouble;

mod upstream;
pub use upstream::Upstream;

use axum::body::to_bytes;
use axum::response::Response;
use serde_json::Value;

/// The key the upstream double is published under, unless a case changes it.
pub const SERVER_KEY: &str = "the-server-key-in-the-file";

pub async fn text_of(response: Response) -> String {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the answer arrives in full");
    String::from_utf8_lossy(&body).into_owned()
}

pub async fn json_of(response: Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the answer arrives in full");
    serde_json::from_slice(&body).expect("the answer is JSON")
}
