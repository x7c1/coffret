//! A loopback bucket for the cases that are not about S3.
//!
//! This crate's own cases reach it directly, and `coffret-cli`'s reach it
//! through the `stub-bucket` feature, which only that crate's
//! `[dev-dependencies]` turn on: nothing a shipping build compiles holds it.
//! Only the standard library is used, so the feature adds no dependency.

mod answer_to;
use answer_to::answer_to;

mod request;
use request::read_request;

use std::collections::BTreeMap;
use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Mutex, OnceLock};

/// An endpoint that answers what putting an S3 Library on a device asks of
/// its bucket.
///
/// Creating a Library asks its bucket whether it is there, which is what turns
/// a mistyped bucket into a refusal before a Passphrase is chosen rather than
/// at `init`'s last step, and then writes under its prefix the Library's first
/// Keyring generation, read back, and the first head that selects it, `head-0`
/// (spec: CP-2, FM-13).
/// Joining one asks whether the bucket is there and whether the prefix it was
/// given holds any head or Index Snapshot of a Library, which is what turns a
/// mistyped Library ID into a word at `join` rather than into a `fetch` that
/// reports nothing forever. All of that has to be answered for the cases that
/// are not about S3 to be about anything else, and a container is far more than
/// answering it takes.
///
/// So this is a socket over a map of keys to bytes, shared by every case in the
/// process — each Library is under a prefix of its own Library ID, so none of
/// them sees another's. It answers a request asking whether the bucket is
/// there with `200`; a listing with every key under the prefix asked for, one
/// level deep, in one page; a write by storing it, or with `412` where the
/// write asks for a key that is free and finds it taken; a read with the bytes
/// or the range asked for, or `404`; a copy and a delete as S3 does.
///
/// It checks nothing a request is signed with, and sets no credentials to sign
/// with either: whatever the SDK resolves has to be *something* for a request
/// to be signed at all, and which something is each caller's to put where the
/// SDK looks before its first request. The entity tag it answers a write with
/// is not a digest of anything — nothing these cases write compares one.
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
                    std::thread::spawn(move || serve(stream));
                }
            });
            endpoint
        })
        .as_str()
}

/// Every object every case wrote, by bucket and key.
fn objects() -> &'static Mutex<BTreeMap<String, Vec<u8>>> {
    static OBJECTS: OnceLock<Mutex<BTreeMap<String, Vec<u8>>>> = OnceLock::new();
    OBJECTS.get_or_init(Mutex::default)
}

/// Answers every request one connection carries, until it closes.
fn serve(stream: TcpStream) {
    let Ok(mut writer) = stream.try_clone() else {
        // Nothing to report it to and nothing that depends on it: a case whose
        // bucket did not answer fails on its own account.
        return;
    };
    let mut reader = BufReader::new(stream);
    loop {
        let request = match read_request(&mut reader, &mut writer) {
            Ok(Some(request)) => request,
            Ok(None) | Err(_) => return,
        };
        let answer = answer_to(&request);
        if writer
            .write_all(&answer)
            .and_then(|()| writer.flush())
            .is_err()
        {
            return;
        }
    }
}

/// A percent-encoded path segment or query value, decoded.
fn decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                at += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
