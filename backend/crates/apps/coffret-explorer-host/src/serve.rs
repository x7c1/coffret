use std::io;
use std::time::Duration;

use axum::Router;
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;
use tokio::net::TcpListener;

use crate::READ_DEADLINE;

/// How much of a request's head is read before it is refused as too large:
/// the request line and the headers together, so that the headers' own bound
/// is checked over a head that was itself bounded (spec: LA-19).
///
/// Larger than the 8 KiB the headers may come to, because the request line
/// carries an Entry Path in its query and is not what that bound is about.
const MAX_HEAD_BYTES: usize = 64 * 1024;

/// How long to wait before accepting again after the listener itself failed,
/// such as when this process is out of file descriptors.
const ACCEPT_BACKOFF: Duration = Duration::from_secs(1);

/// Answers `router` on `listener`, the listener it was built around, for as
/// long as the process runs: the explorer's host, and any other loopback
/// listener in the same app that bounds what it reads the same way: the
/// desktop app's Passphrase door (spec: LA-22).
///
/// HTTP/1 alone, which is what a browser speaks to an `http:` origin, with what
/// it reads of each request's head bounded in size and in time
/// (spec: LA-19). A connection whose request was refused before its body was
/// read is closed rather than drained.
pub async fn serve(listener: TcpListener, router: Router) {
    serve_with_head_deadline(listener, router, READ_DEADLINE).await;
}

/// Answers as [`serve`] does, with each request's head given `head_deadline`
/// to arrive in rather than LA-19's five seconds.
///
/// What a test of that deadline serves through, so that a head that never
/// finishes arriving is given up on in a fraction of a second rather than
/// after five of them. Everything else is [`serve`]'s.
pub async fn serve_with_head_deadline(
    listener: TcpListener,
    router: Router,
    head_deadline: Duration,
) {
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(accepted) => accepted,
            // A connection that broke before it was accepted is that
            // connection's, and the listener goes on.
            Err(cause) if is_per_connection(&cause) => continue,
            // The listener's own, and passing: what runs out is given back as
            // connections close.
            Err(cause) => {
                tracing::warn!(
                    io_error_kind = ?cause.kind(),
                    "a loopback host could not accept a connection; it tries again shortly",
                );
                tokio::time::sleep(ACCEPT_BACKOFF).await;
                continue;
            }
        };
        let service = TowerToHyperService::new(router.clone());
        tokio::spawn(async move {
            let served = http1::Builder::new()
                .timer(TokioTimer::new())
                .header_read_timeout(head_deadline)
                .max_buf_size(MAX_HEAD_BYTES)
                .serve_connection(TokioIo::new(stream), service)
                .await;
            if let Err(cause) = served {
                // The kind of failure and nothing else: what hyper says can
                // quote the request (spec: EL-1).
                tracing::debug!(
                    timeout = cause.is_timeout(),
                    parse = cause.is_parse(),
                    "a connection to a loopback host ended with an error",
                );
            }
        });
    }
}

fn is_per_connection(cause: &io::Error) -> bool {
    matches!(
        cause.kind(),
        io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::Interrupted
    )
}
