//! An endpoint that takes every connection and answers nothing, for the cases
//! about the deadline on a small call.

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::Client;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

use crate::call_deadline::{stalled_stream_protection, timeout_config};

/// Starts a loopback listener that reads every request and answers none, and
/// returns a client built with this crate's timeouts pointed at it.
pub(crate) async fn client_at_silent_endpoint() -> Client {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port is free");
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                // Drained so the client is never held up sending, and never
                // answered.
                let mut buf = [0u8; 4096];
                while matches!(socket.read(&mut buf).await, Ok(n) if n > 0) {}
            });
        }
    });

    let config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .endpoint_url(endpoint)
        .credentials_provider(Credentials::new("key", "secret", None, None, "test"))
        .force_path_style(true)
        .timeout_config(timeout_config())
        .stalled_stream_protection(stalled_stream_protection())
        .http_client(aws_smithy_http_client::Builder::new().build_http())
        .build();
    Client::from_conf(config)
}
