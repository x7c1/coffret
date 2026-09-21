//! A loopback bucket for the cases that are not about S3.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::OnceLock;

/// An endpoint that answers the questions putting an S3 Library on a device
/// asks.
///
/// Creating a Library asks its bucket whether it is there, which is what turns a
/// mistyped bucket into a refusal at `init` rather than a surprise at the first
/// sync. Joining one asks that and then whether the prefix it was given holds
/// the first link of a Library's head chain, which is what turns a mistyped
/// Library ID into a word at `join` rather than into a `fetch` that reports
/// nothing forever. The cases about `init`, `join`, `map` and `recovery-code`
/// are not about S3 and should not need a container running, so this answers
/// both.
///
/// It says `200` to a request addressed at the bucket and `404` to one addressed
/// at a key under it — which is exactly what a bucket that exists and has never
/// been written into answers, and what every Library these cases create is:
/// creating one writes nothing to Storage.
///
/// It says nothing else about S3 and is not meant to. What a real implementation
/// answers is the round trip's business, and that one runs against MinIO.
pub fn stub_endpoint() -> &'static str {
    static ENDPOINT: OnceLock<String> = OnceLock::new();
    ENDPOINT
        .get_or_init(|| {
            let listener = TcpListener::bind("127.0.0.1:0")
                .expect("a loopback port must be available for the stub bucket");
            let endpoint = format!(
                "http://{}",
                listener
                    .local_addr()
                    .expect("a bound listener has an address")
            );

            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    std::thread::spawn(move || answer(stream));
                }
            });
            endpoint
        })
        .as_str()
}

/// Answers every request one connection carries, until it closes.
fn answer(stream: TcpStream) {
    let Ok(mut writer) = stream.try_clone() else {
        // Nothing to report it to and nothing that depends on it: a case whose
        // bucket did not answer fails on its own account.
        return;
    };
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    let mut about_a_key = false;
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        // The first line of a request carries what it is about, and it is the
        // only part of the head worth reading here.
        if let Some(target) = request_target(&line) {
            about_a_key = names_a_key(target);
            continue;
        }
        // The request's head ends at the blank line; nothing here reads a body,
        // because every call made against this is a `HEAD`.
        if line.trim().is_empty() {
            let answer: &[u8] = match about_a_key {
                true => b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\n\r\n",
                false => b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n",
            };
            if writer
                .write_all(answer)
                .and_then(|()| writer.flush())
                .is_err()
            {
                return;
            }
        }
    }
}

/// What a request line is addressed at, where the line is one.
fn request_target(line: &str) -> Option<&str> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    (version.starts_with("HTTP/") && method.chars().all(|c| c.is_ascii_uppercase()))
        .then_some(target)
}

/// Whether a request target names a key inside the bucket rather than the
/// bucket itself.
///
/// Every case here addresses the bucket as a path segment, so the bucket alone
/// is one segment and anything under it is more than one.
fn names_a_key(target: &str) -> bool {
    target
        .split('?')
        .next()
        .unwrap_or(target)
        .trim_matches('/')
        .contains('/')
}
