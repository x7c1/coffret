//! A loopback bucket for the cases that are not about S3.
//!
//! This crate's own cases reach it directly, and `coffret-cli`'s reach it
//! through the `stub-bucket` feature, which only that crate's
//! `[dev-dependencies]` turns on: nothing a shipping build compiles holds it.
//! Only the standard library is used, so the feature adds no dependency.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::OnceLock;

/// An endpoint that answers the two questions putting an S3 Library on a
/// device asks.
///
/// Creating a Library asks its bucket whether it is there, which is what turns a
/// mistyped bucket into a refusal at `init` rather than a surprise at the first
/// sync. Joining one asks that and then whether the prefix it was given holds
/// any head or Index Snapshot of a Library, which is what turns a mistyped
/// Library ID into a word at `join` rather than into a `fetch` that reports
/// nothing forever. Both have to be answered for the cases that are not about
/// S3 to be about anything else, and a container is far more than answering
/// them takes.
///
/// So this is a socket that says `200` to a request asking whether the bucket
/// is there, an empty page to a listing of it, and `404` to a request addressed
/// at a key under it — which is exactly what a bucket that exists and has never
/// been written into answers, and what every Library these cases create is:
/// creating one writes nothing to Storage.
///
/// It checks nothing a request is signed with, and sets no credentials to sign
/// with either: whatever the SDK resolves has to be *something* for a request
/// to be signed at all, and which something is each caller's to put where the
/// SDK looks before its first request.
///
/// It says nothing else about S3 and is not meant to. What a real
/// implementation answers is the conformance suites' and the round trip's
/// business, and those run against MinIO.
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

/// An empty page of a `ListObjectsV2` listing: what a bucket answers a
/// listing of a prefix nothing has been written under with.
const EMPTY_LISTING: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    r#"<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#,
    "<KeyCount>0</KeyCount><IsTruncated>false</IsTruncated>",
    "</ListBucketResult>",
);

/// Answers every request one connection carries, until it closes.
fn answer(stream: TcpStream) {
    let Ok(mut writer) = stream.try_clone() else {
        // Nothing to report it to and nothing that depends on it: a case whose
        // bucket did not answer fails on its own account.
        return;
    };
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    let mut answer = Vec::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        // The first line of a request carries what it is about, and it is the
        // only part of the head worth reading here.
        if let Some(target) = request_target(&line) {
            answer = answer_to(target);
            continue;
        }
        // The request's head ends at the blank line; nothing here reads a body,
        // because every call made against this is a `HEAD` or a listing, and
        // neither carries one.
        if line.trim().is_empty()
            && writer
                .write_all(&answer)
                .and_then(|()| writer.flush())
                .is_err()
        {
            return;
        }
    }
}

/// What a bucket that exists and holds nothing answers a request addressed at
/// `target` with.
fn answer_to(target: &str) -> Vec<u8> {
    let (status, body) = if names_a_key(target) {
        ("404 Not Found", "")
    } else if target.contains("list-type=2") {
        ("200 OK", EMPTY_LISTING)
    } else {
        ("200 OK", "")
    };
    format!(
        "HTTP/1.1 {status}\r\ncontent-length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
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
