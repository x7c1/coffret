use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout_at, Instant};
use tracing::warn;

use crate::error::{Error, RedirectStep, Result};

/// What the loopback server answers the browser with once it has the code.
const COMPLETION_PAGE: &str = "<!doctype html><meta charset=\"utf-8\">\
<title>coffret</title><p>coffret is authorized. You can close this tab.</p>";

/// How many bytes of a request head — request line included — the loopback
/// server reads before it answers.
///
/// A browser's redirect carries a dozen or so headers: `Host`, `User-Agent`,
/// the `Accept` family, the `Sec-Fetch-*` and client-hint headers, and a
/// `Referer` naming the provider's consent page. That is a few kilobytes. The
/// one header that can grow is `Cookie`: cookies are scoped by host and not by
/// port, so whatever else on this machine serves `127.0.0.1` can leave its
/// cookies on this request too. 64 KiB is room for a generous jar of those —
/// four times what common HTTP servers accept by default — while keeping what
/// a client that never ends its head can make the server read small. Only one
/// line at a time is held, so this bounds the work, not a buffer.
const MAX_HEAD_BYTES: u64 = 64 * 1024;

/// How long the loopback server waits for a whole request head, counted from
/// when it takes the connection.
///
/// A browser that connects to send a request writes the head at once, and on
/// loopback it arrives within milliseconds; a second is far beyond that, and
/// short enough that a client which sends nothing, or stops before the blank
/// line, holds up the wait for the real redirect only briefly.
const HEAD_WAIT: Duration = Duration::from_secs(1);

/// Waits for the browser to arrive with the authorization code (spec: SA-2).
///
/// A browser sent to a loopback port asks for other things too — a favicon,
/// most often — so any other request that arrives is answered and ignored
/// rather than mistaken for the redirect, and a connection that sends nothing
/// before the server stops waiting for it is closed without an answer. The
/// wait ends with the redirect — its code, the provider's refusal, or the
/// refusal of a redirect without this run's state — or with a failure to
/// accept on the listener; nothing else one connection sends, or fails to
/// send, ends it.
pub(super) async fn wait_for_code(listener: &TcpListener, state: &str) -> Result<String> {
    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|cause| Error::LoopbackRedirect {
                step: RedirectStep::Accept,
                cause,
            })?;

        if let Some(outcome) = read_redirect(stream, state).await? {
            return Ok(outcome);
        }
    }
}

/// Reads one loopback request, answers it, and reports the code it carried:
/// `None` for any request that is not the redirect, which is answered and
/// ignored, and for a connection that sends nothing before the server stops
/// waiting for it, which is closed without an answer.
///
/// The whole head — request line first — is read within [`MAX_HEAD_BYTES`]
/// and by [`HEAD_WAIT`] after the connection was taken, so that no client can
/// hold up the wait for the real redirect or make the server read without end.
async fn read_redirect(stream: TcpStream, state: &str) -> Result<Option<String>> {
    let deadline = Instant::now() + HEAD_WAIT;
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    // `read_until` keeps what it has read in `line` when the deadline cancels
    // it, which is what tells a client that sent nothing from one that sent
    // part of a line.
    let read = timeout_at(
        deadline,
        (&mut reader)
            .take(MAX_HEAD_BYTES)
            .read_until(b'\n', &mut line),
    )
    .await;
    match read {
        // Nothing was asked, so nothing is answered: the connection is closed
        // without a word. A browser opens connections ahead of need, and an
        // answer written into one could be paired with the request it sends
        // there later. With nothing received, the close is a clean one.
        Err(_) | Ok(Ok(0)) if line.is_empty() => {
            warn!(
                operation = "authorize",
                step = "read_request_line",
                reason = "nothing_sent",
                "a connection on the redirect port sent no request"
            );
            return Ok(None);
        }
        // This arm and the next: a request was begun but its line never
        // ended — past the deadline here, past the bound in the next. What
        // was read may be only part of its target, so it is not taken for the
        // redirect, but it is a request, so it is answered like any other that
        // is not the redirect.
        Err(_) => {
            warn!(
                operation = "authorize",
                step = "read_request_line",
                reason = "timed_out",
                "a request on the redirect port did not finish its request line in time"
            );
            answer_and_close(reader.into_inner()).await;
            return Ok(None);
        }
        Ok(Ok(read)) if read as u64 == MAX_HEAD_BYTES && !line.ends_with(b"\n") => {
            warn!(
                operation = "authorize",
                step = "read_request_line",
                reason = "past_bound",
                "a request on the redirect port sent a request line longer than the loopback server reads"
            );
            answer_and_close(reader.into_inner()).await;
            return Ok(None);
        }
        Ok(Ok(_)) => {}
        // The connection failed before a request line was read whole, so there
        // is no request to answer and likely no one left to read an answer:
        // it is closed, and the wait goes on.
        Ok(Err(cause)) => {
            warn!(
                operation = "authorize",
                step = "read_request_line",
                io_error_kind = ?cause.kind(),
                "the loopback server could not read the request line of a connection on the redirect port"
            );
            return Ok(None);
        }
    }
    // A request line that is not UTF-8 is no redirect this flow sent the
    // browser to, but it is a request, so it is answered and passed over.
    let Ok(request_line) = String::from_utf8(line) else {
        warn!(
            operation = "authorize",
            step = "read_request_line",
            reason = "not_utf8",
            "a request on the redirect port sent a request line that is not UTF-8"
        );
        answer_and_close(reader.into_inner()).await;
        return Ok(None);
    };

    let target = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_owned();

    // The rest of the head is read and discarded before the answer goes out.
    // Closing a socket with received bytes still unread makes the operating
    // system reset the connection rather than close it, and a reset can reach
    // the browser before the answer is shown. A head that runs past the bound,
    // stops short or does not finish in time is logged and answered all the
    // same, and its request line still decides whether it was the redirect.
    let budget = MAX_HEAD_BYTES.saturating_sub(request_line.len() as u64);
    match timeout_at(deadline, skip_rest_of_head(&mut reader, budget)).await {
        Ok(Ok(HeadEnd::BlankLine)) => {}
        Ok(Ok(HeadEnd::PastBound)) => warn!(
            operation = "authorize",
            step = "read_head",
            reason = "past_bound",
            "a request on the redirect port sent a head longer than the loopback server reads"
        ),
        Ok(Ok(HeadEnd::CutOff)) => warn!(
            operation = "authorize",
            step = "read_head",
            reason = "cut_off",
            "a request on the redirect port ended before its head did"
        ),
        Ok(Err(cause)) => warn!(
            operation = "authorize",
            step = "read_head",
            io_error_kind = ?cause.kind(),
            "the loopback server could not read the head of a request on the redirect port"
        ),
        Err(_) => warn!(
            operation = "authorize",
            step = "read_head",
            reason = "timed_out",
            "a request on the redirect port did not finish its head in time"
        ),
    }

    answer_and_close(reader.into_inner()).await;
    parse_redirect(&target, state)
}

/// Sends the completion page and closes the connection.
async fn answer_and_close(mut stream: TcpStream) {
    let answer = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{COMPLETION_PAGE}",
        COMPLETION_PAGE.len()
    );
    // A failure to answer or to close is logged and passed over rather than
    // returned. The browser on the other end may have gone away already — a
    // tab closed, a favicon request abandoned — and that must not end the wait
    // for the real redirect, nor lose the code of a redirect that did arrive.
    // Only the kind is recorded: the target, and the code and state
    // it may carry, stay out of the event (spec: EL-1, spec: EL-3).
    if let Err(cause) = stream.write_all(answer.as_bytes()).await {
        warn!(
            operation = "authorize",
            step = "answer",
            io_error_kind = ?cause.kind(),
            "the loopback server could not answer a request on the redirect port"
        );
    }
    if let Err(cause) = stream.shutdown().await {
        warn!(
            operation = "authorize",
            step = "close",
            io_error_kind = ?cause.kind(),
            "the loopback server could not close a connection on the redirect port"
        );
    }
}

/// How the rest of a request head ended for [`skip_rest_of_head`].
#[derive(Debug, PartialEq)]
enum HeadEnd {
    /// The blank line that closes a head arrived.
    BlankLine,
    /// The budget ran out before the blank line did.
    PastBound,
    /// The client stopped sending before the blank line.
    CutOff,
}

/// Reads the header lines that follow a request line, up to and including
/// the blank line that ends them, reading at most `budget` bytes.
async fn skip_rest_of_head(
    reader: &mut BufReader<TcpStream>,
    budget: u64,
) -> std::io::Result<HeadEnd> {
    let mut remaining = budget;
    let mut line = Vec::new();
    loop {
        if remaining == 0 {
            return Ok(HeadEnd::PastBound);
        }
        line.clear();
        let read = (&mut *reader)
            .take(remaining)
            .read_until(b'\n', &mut line)
            .await?;
        if read == 0 {
            return Ok(HeadEnd::CutOff);
        }
        remaining -= read as u64;
        if !line.ends_with(b"\n") {
            // Either the budget ran out mid-line or the client stopped
            // mid-line; the next turn tells which.
            continue;
        }
        if line == b"\r\n" || line == b"\n" {
            return Ok(HeadEnd::BlankLine);
        }
    }
}

/// Reads the code out of a redirect target, checking it is ours.
///
/// `None` means the request was not the redirect at all, so waiting continues.
/// A target that is not a URL is one of those: the provider sends the browser
/// back to a URL this flow built, so whatever else arrives is not the redirect.
fn parse_redirect(target: &str, state: &str) -> Result<Option<String>> {
    let url = match url::Url::parse(&format!("http://127.0.0.1{target}")) {
        Ok(url) => url,
        Err(cause) => {
            // The target stays out of the event: it may be a redirect cut
            // short, carrying a code or the state (spec: EL-1, spec: EL-3).
            // What the parser reported is kept, by its kind alone: a
            // `url::ParseError` names a rule the target broke and quotes none
            // of it.
            warn!(
                operation = "authorize",
                step = "parse_target",
                reason = "not_a_url",
                parse_error = ?cause,
                "a request on the redirect port asked for a target that is not a URL"
            );
            return Ok(None);
        }
    };

    let mut code = None;
    let mut refusal = None;
    let mut returned_state = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "error" => refusal = Some(value.into_owned()),
            "state" => returned_state = Some(value.into_owned()),
            _ => {}
        }
    }

    if code.is_none() && refusal.is_none() {
        return Ok(None);
    }

    // Asked ahead of the refusal, not only ahead of the code. RFC 6749
    // §4.1.2.1 sends the `state` back on an error redirect too, and without
    // checking it first there is nothing to say the `error` came from the
    // provider rather than from whatever else on this machine can reach the
    // port.
    if returned_state.as_deref() != Some(state) {
        return Err(Error::RedirectWithoutState);
    }
    if let Some(refusal) = refusal {
        return Err(Error::ProviderRefusedAuthorization { refusal });
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    //! One arrival at a time: what the flow takes for its redirect, and what
    //! it does with everything else that reaches the port (spec: SA-2).

    use std::time::Duration;

    use tokio::io::AsyncReadExt;
    use tokio::time::timeout;

    use super::*;

    /// How long any one step of the socket test may take before it counts as
    /// hung rather than slow.
    const STEP: Duration = Duration::from_secs(5);

    // SA-2: a redirect carrying the state this run drew is the redirect, and
    // its code is what the flow takes.
    #[test]
    fn a_matching_redirect_yields_its_code() {
        let code = parse_redirect("/?code=4%2Fabc&state=s3cr3t", "s3cr3t")
            .expect("a well-formed redirect must parse");

        assert_eq!(code.as_deref(), Some("4/abc"));
    }

    // SA-2: the CSRF check failing — a redirect aimed at this port by something
    // that never saw the state is the case the state exists for.
    #[test]
    fn a_redirect_carrying_someone_elses_state_is_refused() {
        for target in [
            // Another page on the machine, carrying a state of its own.
            "/?code=4%2Fabc&state=elsewhere",
            // And one carrying none at all, which is no likelier to be ours.
            "/?code=4%2Fabc",
            // And one naming a refusal: `?error=` is the provider's word only
            // where the state says the redirect is this flow's own, so an
            // unattributable one is this failure and not the provider's.
            "/?error=access_denied",
        ] {
            let outcome = parse_redirect(target, "s3cr3t");
            assert!(
                matches!(outcome, Err(Error::RedirectWithoutState)),
                "{target:?}: {outcome:?}"
            );
        }
    }

    // SA-2: the provider's own word for why it said no travels whole, so a
    // person is told what they declined rather than that something went wrong.
    #[test]
    fn a_refusal_is_reported_rather_than_waited_out() {
        let outcome = parse_redirect("/?error=access_denied&state=s3cr3t", "s3cr3t");
        let Err(Error::ProviderRefusedAuthorization { refusal }) = &outcome else {
            panic!("a provider that refused must be reported as such: {outcome:?}");
        };
        assert_eq!(refusal, "access_denied");
    }

    // SA-2: any other request that arrives on the port is ignored rather than
    // mistaken for the redirect, and the flow goes on waiting.
    #[test]
    fn anything_that_is_not_the_redirect_keeps_the_flow_waiting() {
        for target in [
            "/favicon.ico",
            // Not a URL at all — no port is that large — and so not the
            // redirect either.
            ":99999999",
        ] {
            assert_eq!(
                parse_redirect(target, "s3cr3t").expect("an unrelated request is not an error"),
                None,
                "{target:?}"
            );
        }
    }

    // SA-2: a request that is not the redirect is answered and its connection
    // closed — what keeps a browser asking for `/favicon.ico` from hanging —
    // and the redirect that comes after it still yields its code.
    #[tokio::test]
    async fn a_request_that_is_not_the_redirect_is_answered_and_closed() {
        let (address, waiting) = start_waiting().await;

        let favicon = exchange(address, &browser_request("/favicon.ico")).await;
        assert!(
            favicon.starts_with("HTTP/1.1 200 "),
            "an unrelated request must be answered with 200: {favicon:?}"
        );
        assert!(
            !waiting.is_finished(),
            "an unrelated request must not end the wait"
        );

        expect_code_after_redirect(address, waiting).await;
    }

    // SA-2: a request whose head runs past what the server reads is answered
    // and closed all the same, and does not end the wait for the redirect.
    #[tokio::test]
    async fn a_request_with_a_head_past_the_bound_is_answered_and_closed() {
        let (address, waiting) = start_waiting().await;

        // Exactly the bound and no blank line: the server reads all of it and
        // can tell only that the head has not ended, and nothing is left
        // unread to turn its close into a reset.
        let mut request = String::from("GET /favicon.ico HTTP/1.1\r\nHost: 127.0.0.1\r\nCookie: ");
        let filler = MAX_HEAD_BYTES as usize - request.len();
        request.push_str(&"a".repeat(filler));
        assert_eq!(request.len() as u64, MAX_HEAD_BYTES);

        let answer = exchange(address, &request).await;
        assert!(
            answer.starts_with("HTTP/1.1 200 "),
            "a head past the bound must still be answered with 200: {answer:?}"
        );
        assert!(
            !waiting.is_finished(),
            "a head past the bound must not end the wait"
        );

        expect_code_after_redirect(address, waiting).await;
    }

    // SA-2: a request whose head never finishes is answered once the server
    // stops waiting for it, and does not end the wait for the redirect.
    #[tokio::test]
    async fn a_request_whose_head_never_ends_is_answered_and_closed() {
        let (address, waiting) = start_waiting().await;

        let answer = exchange(address, "GET /favicon.ico HTTP/1.1\r\nHost: 127.0.0.1\r\n").await;
        assert!(
            answer.starts_with("HTTP/1.1 200 "),
            "a head that never ends must still be answered with 200: {answer:?}"
        );
        assert!(
            !waiting.is_finished(),
            "a head that never ends must not end the wait"
        );

        expect_code_after_redirect(address, waiting).await;
    }

    // SA-2: a connection that sends nothing by the deadline is closed without
    // an answer, since nothing was asked, and does not end the wait for the
    // redirect.
    #[tokio::test]
    async fn a_connection_that_sends_nothing_is_closed_without_an_answer() {
        let (address, waiting) = start_waiting().await;

        let answer = exchange(address, "").await;
        assert_eq!(answer, "", "a connection that asked nothing gets no answer");
        assert!(
            !waiting.is_finished(),
            "a connection that sends nothing must not end the wait"
        );

        expect_code_after_redirect(address, waiting).await;
    }

    // SA-2: a request line that runs on past the bound is answered and closed
    // like any request that is not the redirect, and does not end the wait.
    #[tokio::test]
    async fn a_request_line_without_end_is_answered_and_closed() {
        let (address, waiting) = start_waiting().await;

        // Exactly the bound and no line end, so that the server reads all of
        // it and nothing left unread turns its close into a reset.
        let mut request = String::from("GET /");
        request.push_str(&"a".repeat(MAX_HEAD_BYTES as usize - request.len()));

        let answer = exchange(address, &request).await;
        assert!(
            answer.starts_with("HTTP/1.1 200 "),
            "an endless request line must still be answered with 200: {answer:?}"
        );
        assert!(
            !waiting.is_finished(),
            "an endless request line must not end the wait"
        );

        expect_code_after_redirect(address, waiting).await;
    }

    // SA-2: a request line that is not UTF-8 is a request, but not the
    // redirect: it is answered and closed, and does not end the wait.
    #[tokio::test]
    async fn a_request_line_that_is_not_utf8_is_answered_and_closed() {
        let (address, waiting) = start_waiting().await;

        let mut request = b"GET /\xff\xfe HTTP/1.1\r\n".to_vec();
        request.extend_from_slice(b"Host: 127.0.0.1\r\n\r\n");
        let answer = exchange(address, request).await;
        assert!(
            answer.starts_with("HTTP/1.1 200 "),
            "a request line that is not UTF-8 must be answered with 200: {answer:?}"
        );
        assert!(
            !waiting.is_finished(),
            "a request line that is not UTF-8 must not end the wait"
        );

        expect_code_after_redirect(address, waiting).await;
    }

    // SA-2: a target that is not a URL is a request, but not the redirect: it
    // is answered and closed, and does not end the wait.
    #[tokio::test]
    async fn a_target_that_does_not_parse_is_answered_and_closed() {
        let (address, waiting) = start_waiting().await;

        let answer = exchange(address, browser_request(":99999999")).await;
        assert!(
            answer.starts_with("HTTP/1.1 200 "),
            "a target that does not parse must be answered with 200: {answer:?}"
        );
        assert!(
            !waiting.is_finished(),
            "a target that does not parse must not end the wait"
        );

        expect_code_after_redirect(address, waiting).await;
    }

    // The server reads a head up to its blank line, and no further than its
    // bound, so that it is the bound and not the wait that ends a head with no
    // end.
    #[tokio::test]
    async fn the_rest_of_a_head_is_read_to_its_blank_line_or_its_bound() {
        for (sent, budget, expected) in [
            (
                "Host: 127.0.0.1\r\nAccept: */*\r\n\r\n",
                64,
                HeadEnd::BlankLine,
            ),
            ("Host: 127.0.0.1\n\n", 64, HeadEnd::BlankLine),
            (
                "Host: 127.0.0.1\r\nAccept: */*\r\n\r\n",
                20,
                HeadEnd::PastBound,
            ),
            ("Host: 127.0.0.1\r\n", 17, HeadEnd::PastBound),
            ("Host: 127.0.0.1\r\nAccept: */*", 64, HeadEnd::CutOff),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("a loopback port must be free");
            let address = listener.local_addr().expect("a bound port has an address");
            let mut client = TcpStream::connect(address)
                .await
                .expect("the loopback port must accept");
            let (server, _) = listener.accept().await.expect("the client is connecting");
            client
                .write_all(sent.as_bytes())
                .await
                .expect("the head must be sent");
            if expected == HeadEnd::CutOff {
                client
                    .shutdown()
                    .await
                    .expect("the client must close its side");
            }

            let mut reader = BufReader::new(server);
            let ended = timeout(STEP, skip_rest_of_head(&mut reader, budget))
                .await
                .expect("reading the head must not wait once it has ended or run out")
                .expect("the head must be readable");
            assert_eq!(ended, expected, "{sent:?} with a budget of {budget}");
        }
    }

    type Waiting = tokio::task::JoinHandle<Result<String>>;

    /// Binds a loopback port and starts waiting on it for the code of a run
    /// whose state is `s3cr3t`.
    async fn start_waiting() -> (std::net::SocketAddr, Waiting) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback port must be free");
        let address = listener.local_addr().expect("a bound port has an address");
        let waiting = tokio::spawn(async move { wait_for_code(&listener, "s3cr3t").await });
        (address, waiting)
    }

    /// Sends the redirect the way a browser would and checks that it ends the
    /// wait with its code.
    async fn expect_code_after_redirect(address: std::net::SocketAddr, waiting: Waiting) {
        let answer = exchange(address, &browser_request("/?code=4%2Fabc&state=s3cr3t")).await;
        assert!(
            answer.starts_with("HTTP/1.1 200 "),
            "the redirect must be answered with 200: {answer:?}"
        );
        let code = timeout(STEP, waiting)
            .await
            .expect("the redirect must end the wait")
            .expect("the waiting task must not panic")
            .expect("a matching redirect must yield its code");
        assert_eq!(code, "4/abc");
    }

    /// A request for `target` shaped the way a browser sends one: a request
    /// line, a few headers and the blank line that ends them.
    fn browser_request(target: &str) -> String {
        format!(
            "GET {target} HTTP/1.1\r\n\
             Host: 127.0.0.1\r\n\
             User-Agent: Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0\r\n\
             Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n\
             Accept-Language: en-US,en;q=0.5\r\n\
             Connection: keep-alive\r\n\
             \r\n"
        )
    }

    /// Sends one request and reads the answer until the server closes the
    /// connection; a read that times out instead fails the test.
    async fn exchange(address: std::net::SocketAddr, request: impl AsRef<[u8]>) -> String {
        let mut client = timeout(STEP, TcpStream::connect(address))
            .await
            .expect("connecting must not hang")
            .expect("the loopback server must accept");
        timeout(STEP, client.write_all(request.as_ref()))
            .await
            .expect("sending must not hang")
            .expect("the request must be sent");
        let mut answer = Vec::new();
        timeout(STEP, client.read_to_end(&mut answer))
            .await
            .expect("the server must close the connection rather than leave it open")
            .expect("the answer must be readable to its end");
        String::from_utf8(answer).expect("the answer is text")
    }
}
