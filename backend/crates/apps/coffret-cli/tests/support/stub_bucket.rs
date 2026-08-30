//! A loopback bucket for the cases that are not about S3.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::OnceLock;

/// An endpoint that answers the one question creating an S3 Library asks.
///
/// Creating a Library asks its bucket whether it is there, which is what turns a
/// mistyped bucket into a refusal at `init` rather than a surprise at the first
/// sync. The cases about `init`, `map` and `recovery-code` are not about S3 and
/// should not need one running, so this answers `200` to whatever arrives — the
/// whole of what `HeadBucket` on a bucket that exists comes back as. What a real
/// implementation answers is the round trip's business, and that one runs
/// against MinIO.
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
                    std::thread::spawn(move || answer_ok(stream));
                }
            });
            endpoint
        })
        .as_str()
}

/// Says `200` to every request one connection carries, until it closes.
fn answer_ok(stream: TcpStream) {
    let Ok(mut writer) = stream.try_clone() else {
        // Nothing to report it to and nothing that depends on it: a case whose
        // bucket did not answer fails on its own account.
        return;
    };
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        // The request's head ends at the blank line; nothing here reads a body,
        // because the only call made against this is a `HEAD`.
        if line.trim().is_empty()
            && writer
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n")
                .and_then(|()| writer.flush())
                .is_err()
        {
            return;
        }
    }
}
