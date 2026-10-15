//! The host on a real socket, through [`serve`]: what it reads of a request it
//! refuses, and the connection it came on (spec: LA-13, LA-19).

mod support;

use std::time::Duration;

use coffret_explorer_host::serve;
use support::Harness;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Starts serving `host` on its own listener, and where it is.
async fn serving(host: Harness) -> (String, support::Upstream) {
    let Harness {
        router,
        listener,
        authority,
        upstream,
        ..
    } = host;
    tokio::spawn(serve(listener, router));
    (authority, upstream)
}

/// Everything the host writes back on `stream` until it closes it, or `None`
/// where it is still open after a while.
async fn until_closed(stream: &mut TcpStream) -> Option<String> {
    let mut answer = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut answer)).await;
    match read {
        Ok(Ok(_)) => Some(String::from_utf8_lossy(&answer).into_owned()),
        // A reset after the answer is a close as well.
        Ok(Err(_)) => Some(String::from_utf8_lossy(&answer).into_owned()),
        Err(_) => None,
    }
}

// LA-19: an upload without a token is refused on its head, and the connection
// is closed rather than read to the end of the gigabyte it declared.
#[tokio::test]
async fn a_refused_upload_is_answered_and_its_connection_closed_unread() {
    let (authority, upstream) = serving(Harness::start().await).await;
    let mut stream = TcpStream::connect(&authority).await.unwrap();
    stream
        .write_all(
            format!(
                "POST /api/upload?folder=a HTTP/1.1\r\nHost: {authority}\r\n\
                 Content-Type: multipart/form-data; boundary=x\r\n\
                 Content-Length: 1073741824\r\n\r\n--x\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();

    let answer = until_closed(&mut stream)
        .await
        .expect("the host closes the connection");
    assert!(answer.starts_with("HTTP/1.1 401"), "{answer}");
    assert!(
        answer.to_ascii_lowercase().contains("connection: close"),
        "{answer}"
    );
    assert_eq!(upstream.connections(), 0);
}

// LA-13: a target in absolute form naming another authority is refused on a
// real connection too.
#[tokio::test]
async fn an_absolute_target_naming_elsewhere_is_refused_on_the_wire() {
    let (authority, _upstream) = serving(Harness::start().await).await;
    let mut stream = TcpStream::connect(&authority).await.unwrap();
    stream
        .write_all(
            format!(
                "GET http://evil.invalid/api/list HTTP/1.1\r\nHost: {authority}\r\n\
                 Connection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();

    let answer = until_closed(&mut stream).await.expect("the host answers");
    assert!(answer.starts_with("HTTP/1.1 403"), "{answer}");
}

// LA-19: a head larger than the host reads is refused rather than read on.
#[tokio::test]
async fn a_head_past_its_bound_is_refused() {
    let (authority, _upstream) = serving(Harness::start().await).await;
    let mut stream = TcpStream::connect(&authority).await.unwrap();
    let padding = "a".repeat(128 * 1024);
    // The host may close before it has read all of this.
    let _ = stream
        .write_all(
            format!("GET / HTTP/1.1\r\nHost: {authority}\r\nX-Padding: {padding}\r\n\r\n")
                .as_bytes(),
        )
        .await;

    let answer = until_closed(&mut stream).await.expect("the host closes");
    assert!(answer.starts_with("HTTP/1.1 431"), "{answer}");
}
