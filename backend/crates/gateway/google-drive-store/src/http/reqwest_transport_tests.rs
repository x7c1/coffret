//! The transport's deadlines, against a server on the loopback interface.
//!
//! The scripted transport cannot say anything about these: what is under test
//! is what the real client does when the other end of a socket is silent, slow,
//! or slow and still moving. So each case stands up a listener that behaves one
//! of those ways and hands the real transport a deadline short enough to wait
//! out, which is the only thing about the transport a case changes.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use coffret_usecase::{ByteStream, RetryPolicy};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Instant;

use crate::http::{HttpRequest, HttpTransport, Method, ReqwestTransport, TransportError};

/// The whole-call deadline the cases hold small calls to.
const DEADLINE: Duration = Duration::from_millis(300);

/// How long a server that trickles waits between one byte and the next.
///
/// Far inside the client's between-bytes timeout, so only the whole-call
/// deadline can end a call that is trickling at this pace.
const TRICKLE: Duration = Duration::from_millis(30);

/// A transport with the production client and the cases' short deadline.
fn transport() -> ReqwestTransport {
    ReqwestTransport::with_default_client()
        .expect("an HTTP client must be buildable")
        .with_whole_call_deadline(DEADLINE)
}

/// Reads one request's head and as much of its body as it declares.
async fn read_request(socket: &mut TcpStream) -> Vec<u8> {
    let mut received = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        let n = socket.read(&mut buf).await.expect("the request arrives");
        assert!(n > 0, "the client closed before sending a whole head");
        received.extend_from_slice(&buf[..n]);
        if let Some(at) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break at + 4;
        }
    };
    let head = String::from_utf8_lossy(&received[..head_end]).to_ascii_lowercase();
    let declared = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .map(|value| value.trim().parse::<usize>().expect("a numeric length"))
        .unwrap_or(0);
    let mut body = received.split_off(head_end);
    while body.len() < declared {
        let n = socket.read(&mut buf).await.expect("the body arrives");
        assert!(n > 0, "the client closed before sending the whole body");
        body.extend_from_slice(&buf[..n]);
    }
    body
}

/// Starts a server that answers each connection with `answer`, and returns the
/// URL it listens at and how many connections it has taken.
async fn serve<F, Fut>(answer: F) -> (String, Arc<AtomicUsize>)
where
    F: Fn(TcpStream, usize) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port is free");
    let url = format!("http://{}/files", listener.local_addr().unwrap());
    let taken = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&taken);
    tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            let index = counter.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(answer(socket, index));
        }
    });
    (url, taken)
}

/// Takes the request and says nothing, holding the connection open.
async fn stay_silent(mut socket: TcpStream) {
    read_request(&mut socket).await;
    tokio::time::sleep(Duration::from_secs(3600)).await;
}

/// Answers with a head declaring `len` bytes, then sends them one at a time.
async fn trickle(mut socket: TcpStream, len: usize) {
    read_request(&mut socket).await;
    let head = format!("HTTP/1.1 200 OK\r\ncontent-length: {len}\r\n\r\n");
    if socket.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    for _ in 0..len {
        tokio::time::sleep(TRICKLE).await;
        // The client hanging up is how a case that gave up ends this.
        if socket.write_all(b"x").await.is_err() {
            return;
        }
    }
}

/// Answers at once with a small JSON document.
async fn answer_at_once(mut socket: TcpStream, body: &str) {
    read_request(&mut socket).await;
    let answer = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(answer.as_bytes()).await;
}

/// A listing, which is one of the small calls.
fn listing(url: &str) -> HttpRequest {
    HttpRequest::new(Method::Get, url)
}

/// Unwraps a failed call, naming what came back if it did not fail.
fn failure(outcome: Result<crate::http::HttpResponse, TransportError>) -> TransportError {
    match outcome {
        Ok(response) => panic!("expected a failure, got an answer: {}", response.status()),
        Err(error) => error,
    }
}

#[tokio::test]
async fn a_small_call_nothing_answers_times_out_at_the_deadline() {
    let (url, _) = serve(|socket, _| stay_silent(socket)).await;

    let started = Instant::now();
    let error = failure(transport().execute(listing(&url)).await);
    let took = started.elapsed();

    assert!(
        matches!(error, TransportError::Timeout { .. }),
        "silence is a timeout: {error:?}"
    );
    // Bounded from below by the deadline, and from above by far less than the
    // client's own between-bytes timeout, which is the only other bound that
    // could have ended it.
    assert!(took >= DEADLINE, "it waited the deadline out: {took:?}");
    assert!(
        took < Duration::from_secs(5),
        "it ended at the deadline: {took:?}"
    );
}

#[tokio::test]
async fn a_small_answer_that_trickles_times_out_at_the_deadline() {
    // Forty bytes at the trickle's pace take more than a second, so each byte
    // arrives well inside the between-bytes timeout and the whole of the answer
    // well outside the deadline.
    let (url, _) = serve(|socket, _| trickle(socket, 40)).await;

    let started = Instant::now();
    let error = failure(transport().execute(listing(&url)).await);
    let took = started.elapsed();

    assert!(
        matches!(error, TransportError::Timeout { .. }),
        "a trickle past the deadline is a timeout: {error:?}"
    );
    assert!(
        took < TRICKLE * 40,
        "it ended at the deadline rather than when the answer did: {took:?}"
    );
}

#[tokio::test]
async fn a_small_answer_that_stops_between_bytes_is_a_timeout() {
    // The answer begins and then stops, and the client's own between-bytes
    // timeout is what ends it: the whole-call deadline is set far past it, so
    // the case is about how a stall seen while the answer is drained is named.
    let between_bytes = Duration::from_millis(200);
    let (url, _) = serve(|mut socket, _| async move {
        read_request(&mut socket).await;
        let head = "HTTP/1.1 200 OK\r\ncontent-length: 10\r\n\r\nx";
        if socket.write_all(head.as_bytes()).await.is_ok() {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    })
    .await;
    let client = reqwest::Client::builder()
        .read_timeout(between_bytes)
        .build()
        .expect("an HTTP client must be buildable");
    let transport = ReqwestTransport::new(client).with_whole_call_deadline(Duration::from_secs(30));

    let started = Instant::now();
    let error = failure(transport.execute(listing(&url)).await);
    let took = started.elapsed();

    assert!(
        matches!(error, TransportError::Timeout { .. }),
        "a stall between bytes is a timeout: {error:?}"
    );
    assert!(
        took < Duration::from_secs(5),
        "it ended at the between-bytes timeout: {took:?}"
    );
}

#[tokio::test]
async fn a_small_answer_without_a_length_that_stops_between_bytes_is_a_timeout() {
    // The same stall, in an answer that declares no length: chunked, one chunk
    // sent and then nothing. Such an answer is collected on its way in rather
    // than drained afterwards, and a stall is still a timeout there — what a
    // stall is called must not depend on whether the answer said how long it
    // was.
    let between_bytes = Duration::from_millis(200);
    let (url, _) = serve(|mut socket, _| async move {
        read_request(&mut socket).await;
        let head = "HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n1\r\nx\r\n";
        if socket.write_all(head.as_bytes()).await.is_ok() {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    })
    .await;
    let client = reqwest::Client::builder()
        .read_timeout(between_bytes)
        .build()
        .expect("an HTTP client must be buildable");
    let transport = ReqwestTransport::new(client).with_whole_call_deadline(Duration::from_secs(30));

    let started = Instant::now();
    let error = failure(transport.execute(listing(&url)).await);
    let took = started.elapsed();

    assert!(
        matches!(error, TransportError::Timeout { .. }),
        "a stall between bytes is a timeout whether or not a length was declared: {error:?}"
    );
    assert!(
        took < Duration::from_secs(5),
        "it ended at the between-bytes timeout: {took:?}"
    );
}

#[tokio::test]
async fn an_object_s_bytes_that_keep_arriving_are_not_cut_off_by_the_deadline() {
    let len = 20;
    let (url, _) = serve(move |socket, _| trickle(socket, len)).await;
    assert!(
        TRICKLE * len as u32 > DEADLINE,
        "the transfer outlasts the deadline"
    );

    let response = transport()
        .execute(HttpRequest::new(Method::Get, &url).answering_object_bytes())
        .await
        .expect("a streamed get is answered");
    let mut bytes = Vec::new();
    response
        .into_body()
        .into_reader()
        .read_to_end(&mut bytes)
        .await
        .expect("bytes that keep arriving are read to the end");

    assert_eq!(bytes, vec![b'x'; len]);
}

#[tokio::test]
async fn an_upload_whose_bytes_keep_flowing_is_not_cut_off_by_the_deadline() {
    let len = 20u64;
    let (url, _) = serve(|socket, _| answer_at_once(socket, "{}")).await;

    // A body that yields a byte at the trickle's pace, so the upload outlasts
    // the deadline while never pausing long enough to stall.
    let (reader, mut writer) = tokio::io::duplex(1);
    tokio::spawn(async move {
        for _ in 0..len {
            tokio::time::sleep(TRICKLE).await;
            writer
                .write_all(b"x")
                .await
                .expect("the upload takes the byte");
        }
    });
    assert!(
        TRICKLE * len as u32 > DEADLINE,
        "the transfer outlasts the deadline"
    );

    let started = Instant::now();
    let response = transport()
        .execute(HttpRequest::new(Method::Put, &url).with_stream(ByteStream::new(len, reader)))
        .await
        .expect("a streamed upload is answered");

    assert_eq!(response.status(), 200);
    assert!(
        started.elapsed() > DEADLINE,
        "the upload did take longer than the deadline"
    );
}

#[tokio::test]
async fn a_deadline_that_fires_reaches_the_port_as_a_failure_worth_retrying() {
    let (url, _) = serve(|socket, _| stay_silent(socket)).await;

    // Converted exactly as the gateway converts every transport failure it
    // hands to the port.
    let fired = coffret_usecase::Error::from(failure(transport().execute(listing(&url)).await));

    assert!(
        matches!(fired, coffret_usecase::Error::Timeout { .. }),
        "a fired deadline reaches the port as a timeout: {fired:?}"
    );
    assert!(fired.is_retryable(), "{fired}");
}

#[tokio::test]
async fn a_call_that_stalled_once_is_ridden_out_by_the_retry_policy() {
    // Silent on the first connection, answering on every later one: the shape
    // of a Storage that stalled once.
    let (url, taken) = serve(|socket, index| async move {
        if index == 0 {
            stay_silent(socket).await;
        } else {
            answer_at_once(socket, r#"{"files":[]}"#).await;
        }
    })
    .await;
    let transport = transport();
    let policy = RetryPolicy::default()
        .with_attempts(3)
        .with_base_backoff(Duration::from_millis(10))
        .with_wait_ceiling(Duration::from_millis(100))
        .with_total_wait(Duration::from_secs(1));

    let response = policy
        .run("list", || async {
            transport
                .execute(listing(&url))
                .await
                .map_err(coffret_usecase::Error::from)
        })
        .await
        .expect("the attempt after the stall is answered");

    assert_eq!(response.status(), 200);
    assert_eq!(
        taken.load(Ordering::SeqCst),
        2,
        "the stall ended at the deadline and the policy asked again, once"
    );
}
