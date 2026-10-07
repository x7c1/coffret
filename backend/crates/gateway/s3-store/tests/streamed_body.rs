//! Whether a write sends its object as it reads it.
//!
//! A Pack is tens of megabytes, and the use case counts how far its upload has
//! got by what the store has pulled off the stream it was handed. Both depend on
//! the request this gateway builds sending the body as it is read, rather than
//! reading it whole and then sending it: read whole, memory would grow with the
//! size of a book, and the count would jump to the end at once and then say
//! nothing for as long as the sending took.
//!
//! The capturing client cannot answer that, so the request goes to a listener
//! on the loopback interface that plays S3.

use std::time::Duration;

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::Client;
use coffret_usecase::{ByteStream, ObjectStore};
use s3_store::{S3Settings, S3};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// What the body yields before it waits to hear that some of it has arrived.
///
/// Several frames' worth of what this gateway hands the client, so that what
/// the listener has seen of it by then cannot be a first frame that went out
/// ahead of a body otherwise held back.
const FIRST: usize = 256 * 1024;

/// What it yields after that.
const REST: usize = 256 * 1024;

/// Plays S3 for one `PutObject`: says once the first [`FIRST`] bytes of the
/// body have arrived, reads the rest, and answers as S3 answers a write.
async fn bucket(arrived: oneshot::Sender<()>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port is free");
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("the write connects");
        let mut received = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        let head_end = loop {
            let n = socket.read(&mut buf).await.expect("the request arrives");
            assert!(n > 0, "the client closed before sending a whole head");
            received.extend_from_slice(&buf[..n]);
            if let Some(at) = received.windows(4).position(|window| window == b"\r\n\r\n") {
                break at + 4;
            }
        };
        let head = String::from_utf8_lossy(&received[..head_end]).to_ascii_lowercase();
        // The length on the wire, which is what the body is read to. Where the
        // client frames the object with a trailing checksum, that is a little
        // more than the object; either way it is at least the first stretch.
        let declared: usize = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .map(|value| value.trim().parse().expect("a numeric length"))
            .expect("a write declares its length");
        let mut body = received.split_off(head_end);
        let mut arrived = Some(arrived);
        loop {
            if body.len() >= FIRST {
                if let Some(arrived) = arrived.take() {
                    let _ = arrived.send(());
                }
            }
            if body.len() >= declared {
                break;
            }
            let n = socket.read(&mut buf).await.expect("the body arrives");
            assert!(n > 0, "the client closed before sending the whole body");
            body.extend_from_slice(&buf[..n]);
        }
        let answer = "HTTP/1.1 200 OK\r\netag: \"0123456789abcdef0123456789abcdef\"\r\ncontent-length: 0\r\n\r\n";
        socket
            .write_all(answer.as_bytes())
            .await
            .expect("the answer goes back");
    });
    endpoint
}

// The body holds back everything past its first stretch until the listener has
// seen that stretch arrive. A write that read its body whole before sending
// would never send that stretch, the listener would never say so, and the put
// would wait for ever — which is what the timeout turns into a failure.
#[tokio::test]
async fn a_write_sends_its_body_while_it_is_still_reading_it() {
    let (arrived, heard) = oneshot::channel();
    let endpoint = bucket(arrived).await;
    let config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .endpoint_url(endpoint)
        .credentials_provider(Credentials::new("key", "secret", None, None, "test"))
        .force_path_style(true)
        .http_client(aws_smithy_http_client::Builder::new().build_http())
        .build();
    let store = S3::new(Client::from_conf(config), S3Settings::new("bucket"));

    let (reader, mut writer) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        writer
            .write_all(&vec![1u8; FIRST])
            .await
            .expect("the write takes the first stretch");
        heard
            .await
            .expect("the listener says the first stretch arrived");
        writer
            .write_all(&vec![2u8; REST])
            .await
            .expect("the write takes the rest");
    });

    let len = (FIRST + REST) as u64;
    tokio::time::timeout(
        Duration::from_secs(20),
        store.put("pack-1.cfrt", ByteStream::new(len, reader)),
    )
    .await
    .expect("the body was sent as it was read, not read whole and then sent")
    .expect("the write is answered");
}
