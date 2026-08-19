use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::error::{Error, Result};

/// What the loopback server answers the browser with once it has the code.
const COMPLETION_PAGE: &str = "<!doctype html><meta charset=\"utf-8\">\
<title>coffret</title><p>coffret is authorized. You can close this tab.</p>";

/// Waits for the browser to arrive with the authorization code.
///
/// A browser sent to a loopback port asks for other things too — a favicon,
/// most often — so anything that is not the redirect is answered and ignored
/// rather than mistaken for one.
pub(super) async fn wait_for_code(listener: &TcpListener, state: &str) -> Result<String> {
    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|error| Error::Authorization {
                detail: format!("could not accept the redirect: {error}"),
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
        .map_err(|error| Error::Authorization {
            detail: format!("could not read the redirect: {error}"),
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
    let url = url::Url::parse(&format!("http://127.0.0.1{target}")).map_err(|error| {
        Error::Authorization {
            detail: format!("the redirect target {target:?} is not a URL: {error}"),
        }
    })?;

    let mut code = None;
    let mut error = None;
    let mut returned_state = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            "state" => returned_state = Some(value.into_owned()),
            _ => {}
        }
    }

    if let Some(error) = error {
        return Err(Error::Authorization {
            detail: format!("the request was refused: {error}"),
        });
    }
    let Some(code) = code else {
        return Ok(None);
    };

    // The `state` is what tells our own callback from any other page on the
    // machine that happened to be aimed at this port.
    if returned_state.as_deref() != Some(state) {
        return Err(Error::Authorization {
            detail: "the redirect did not carry the state this flow sent".to_owned(),
        });
    }
    Ok(Some(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_matching_redirect_yields_its_code() {
        let code = parse_redirect("/?code=4%2Fabc&state=s3cr3t", "s3cr3t")
            .expect("a well-formed redirect must parse");

        assert_eq!(code.as_deref(), Some("4/abc"));
    }

    #[test]
    fn a_redirect_carrying_someone_elses_state_is_refused() {
        let outcome = parse_redirect("/?code=4%2Fabc&state=elsewhere", "s3cr3t");
        assert!(matches!(outcome, Err(Error::Authorization { .. })));
    }

    #[test]
    fn a_refusal_is_reported_rather_than_waited_out() {
        let outcome = parse_redirect("/?error=access_denied&state=s3cr3t", "s3cr3t");
        assert!(matches!(outcome, Err(Error::Authorization { .. })));
    }

    #[test]
    fn anything_that_is_not_the_redirect_keeps_the_flow_waiting() {
        assert_eq!(
            parse_redirect("/favicon.ico", "s3cr3t").expect("an unrelated request is not an error"),
            None
        );
    }
}
