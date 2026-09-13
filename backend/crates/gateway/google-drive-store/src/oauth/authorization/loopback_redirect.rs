use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::error::{Error, RedirectStep, Result};

/// What the loopback server answers the browser with once it has the code.
const COMPLETION_PAGE: &str = "<!doctype html><meta charset=\"utf-8\">\
<title>coffret</title><p>coffret is authorized. You can close this tab.</p>";

/// Waits for the browser to arrive with the authorization code (spec: SA-2).
///
/// A browser sent to a loopback port asks for other things too — a favicon,
/// most often — so anything that is not the redirect is answered and ignored
/// rather than mistaken for one.
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

/// Reads one loopback request, answers it, and reports the code it carried.
async fn read_redirect(stream: TcpStream, state: &str) -> Result<Option<String>> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .map_err(|cause| Error::LoopbackRedirect {
            step: RedirectStep::Read,
            cause,
        })?;

    let target = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_owned();

    let mut stream = reader.into_inner();
    let answer = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{COMPLETION_PAGE}",
        COMPLETION_PAGE.len()
    );
    let _ = stream.write_all(answer.as_bytes()).await;
    let _ = stream.shutdown().await;

    parse_redirect(&target, state)
}

/// Reads the code out of a redirect target, checking it is ours.
///
/// `None` means the request was not the redirect at all, so waiting continues.
fn parse_redirect(target: &str, state: &str) -> Result<Option<String>> {
    let url = url::Url::parse(&format!("http://127.0.0.1{target}")).map_err(|cause| {
        Error::MalformedRedirect {
            target: target.to_owned(),
            cause,
        }
    })?;

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
    //! SA-2, one arrival at a time: what the flow takes for its redirect, and
    //! what it does with everything else that reaches the port.

    use super::*;

    #[test]
    fn a_matching_redirect_yields_its_code() {
        let code = parse_redirect("/?code=4%2Fabc&state=s3cr3t", "s3cr3t")
            .expect("a well-formed redirect must parse");

        assert_eq!(code.as_deref(), Some("4/abc"));
    }

    // The CSRF check failing: a redirect aimed at this port by something that
    // never saw the state is the case the state exists for.
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

    // The provider's own word for why it said no travels whole, so a person is
    // told what they declined rather than that something went wrong.
    #[test]
    fn a_refusal_is_reported_rather_than_waited_out() {
        let outcome = parse_redirect("/?error=access_denied&state=s3cr3t", "s3cr3t");
        let Err(Error::ProviderRefusedAuthorization { refusal }) = &outcome else {
            panic!("a provider that refused must be reported as such: {outcome:?}");
        };
        assert_eq!(refusal, "access_denied");
    }

    #[test]
    fn anything_that_is_not_the_redirect_keeps_the_flow_waiting() {
        assert_eq!(
            parse_redirect("/favicon.ico", "s3cr3t").expect("an unrelated request is not an error"),
            None
        );
    }
}
