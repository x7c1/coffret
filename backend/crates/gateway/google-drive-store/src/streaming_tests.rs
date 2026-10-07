//! Whether an upload sends its object as it reads it.
//!
//! A Pack is tens of megabytes, and the use case counts how far its upload has
//! got by what the store has pulled off the stream it was handed. Both depend on
//! the same property of the request this gateway builds: that the body goes out
//! as it is read, rather than being read whole and then sent. Read whole, memory
//! would grow with the size of a book, and the count would jump to the end at
//! once and then say nothing for as long as the sending took.
//!
//! The scripted transport cannot answer that — it drains a body before it
//! answers, as any stand-in has to — so the call that carries the bytes is made
//! by the real client, against a listener on the loopback interface that plays
//! the session URI Drive would have handed back.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use coffret_usecase::{ByteStream, ObjectStore, ProviderHash};
use md5::{Digest, Md5};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::http::{
    HttpRequest, HttpResponse, HttpTransport, Method, ReqwestTransport, StubAnswer, StubTransport,
    TransportError,
};
use crate::test_support::CountingTokens;
use crate::{DriveSettings, GoogleDrive};

/// What the body yields before it waits to hear that some of it has arrived.
///
/// Several times any frame a client sends a streamed body in, so that what the
/// listener has seen of it by then cannot be a first frame that went out ahead
/// of a body otherwise held back.
const FIRST: usize = 256 * 1024;

/// What it yields after that.
const REST: usize = 256 * 1024;

/// A transport that opens the session from a script and sends the bytes for
/// real.
///
/// The session's opening is not what is under test, and its answer is what
/// points the upload at the listener: the URI the bytes go to is whatever
/// `Location` Drive answered with.
struct OpenedByScript {
    opening: Arc<StubTransport>,
    wire: ReqwestTransport,
}

#[async_trait]
impl HttpTransport for OpenedByScript {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        match request.method {
            Method::Post => self.opening.execute(request).await,
            _ => self.wire.execute(request).await,
        }
    }
}

/// Plays the session URI: takes one upload, says once the first [`FIRST`]
/// bytes of its body have arrived, and answers with the digest of what came.
async fn session(arrived: oneshot::Sender<()>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port is free");
    let url = format!(
        "http://{}/upload?upload_id=session-1",
        listener.local_addr().unwrap()
    );
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("the upload connects");
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
        let declared: usize = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .map(|value| value.trim().parse().expect("a numeric length"))
            .expect("an upload declares its length");
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

        let md5 = Md5::digest(&body)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let json = format!(
            r#"{{"id":"file-1","name":"pack-1.cfrt","size":"{}","md5Checksum":"{md5}"}}"#,
            body.len()
        );
        let answer = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{json}",
            json.len()
        );
        socket
            .write_all(answer.as_bytes())
            .await
            .expect("the answer goes back");
    });
    url
}

// The body holds back everything past its first stretch until the listener has
// seen that stretch arrive. An upload that read its body whole before sending
// would never send that stretch, the listener would never say so, and the put
// would wait for ever — which is what the timeout turns into a failure.
#[tokio::test]
async fn an_upload_sends_its_body_while_it_is_still_reading_it() {
    let (arrived, heard) = oneshot::channel();
    let url = session(arrived).await;
    let opening = StubTransport::new([StubAnswer::json_with_headers(
        200,
        vec![("location".to_owned(), url)],
        "",
    )]);
    let transport = Arc::new(OpenedByScript {
        opening,
        wire: ReqwestTransport::with_default_client().expect("an HTTP client must be buildable"),
    });
    let store = GoogleDrive::new(
        transport,
        CountingTokens::new(),
        DriveSettings::new("folder-1"),
    );

    let (reader, mut writer) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        writer
            .write_all(&vec![1u8; FIRST])
            .await
            .expect("the upload takes the first stretch");
        heard
            .await
            .expect("the listener says the first stretch arrived");
        writer
            .write_all(&vec![2u8; REST])
            .await
            .expect("the upload takes the rest");
    });

    let len = (FIRST + REST) as u64;
    let object = tokio::time::timeout(
        Duration::from_secs(20),
        store.put("pack-1.cfrt", ByteStream::new(len, reader)),
    )
    .await
    .expect("the body was sent as it was read, not read whole and then sent")
    .expect("the upload is answered");

    let mut sent = vec![1u8; FIRST];
    sent.extend(vec![2u8; REST]);
    let md5 = Md5::digest(&sent)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        object.hash.as_ref().map(ProviderHash::as_str),
        Some(md5.as_str()),
        "every byte arrived, in order",
    );
}
