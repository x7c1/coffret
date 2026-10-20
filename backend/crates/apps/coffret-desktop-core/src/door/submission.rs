use std::future::poll_fn;
use std::pin::Pin;
use std::time::Duration;

use axum::body::Body;
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::HeaderMap;
use http_body::Body as _;
use serde::Deserialize;
use zeroize::Zeroizing;

use super::Refusal;
use crate::EnteredPassphrase;

/// How many bytes of body a submission may bring: room for a long Passphrase
/// and a Library's name, and far short of anything a person types
/// (spec: LA-22).
pub(crate) const MAX_BODY_BYTES: usize = 4 * 1024;

/// How long reading a submission's body may take, as reading a request to the
/// explorer host's `/auth` may (spec: LA-19).
const READ_DEADLINE: Duration = Duration::from_secs(5);

/// What a Passphrase window submits: the Library to open, in the window's open
/// mode, and the Passphrase.
///
/// The Passphrase is read from the body straight into an
/// [`EnteredPassphrase`], through its own visitor, with no `String` in
/// between (spec: DK-7).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Submission {
    #[serde(default)]
    pub(crate) library: Option<String>,
    pub(crate) passphrase: EnteredPassphrase,
}

impl Submission {
    /// The submission `body` brings, refused where `headers` do not say it is
    /// JSON, where it says or turns out to be more than [`MAX_BODY_BYTES`],
    /// where it takes too long to arrive, and where it is not a submission.
    ///
    /// The body is gathered into a buffer that is wiped when it is dropped,
    /// reserved at its bound so that it never grows and leaves an unwiped copy
    /// behind, and read from there. The frames it arrives in are the network
    /// stack's, outside the claim (spec: DK-7).
    pub(crate) async fn read(
        headers: &HeaderMap,
        body: Body,
    ) -> Result<Self, (Refusal, &'static str)> {
        if !is_json(headers) {
            return Err((Refusal::NotJsonContent, "content_type"));
        }
        let declared = headers
            .get(CONTENT_LENGTH)
            .and_then(|length| length.to_str().ok())
            .map(|length| length.trim().parse::<u64>());
        match declared {
            Some(Ok(length)) if length <= MAX_BODY_BYTES as u64 => {}
            None => {}
            Some(_) => return Err((Refusal::BodyTooLarge, "body_bytes")),
        }
        let gathered = tokio::time::timeout(READ_DEADLINE, gather(body))
            .await
            .map_err(|_| (Refusal::TooSlow, "body_deadline"))??;
        serde_json::from_slice(&gathered).map_err(|_| (Refusal::NotJson, "body"))
    }
}

/// Every byte of `body`, up to [`MAX_BODY_BYTES`], into a buffer that is
/// wiped when it is dropped.
async fn gather(mut body: Body) -> Result<Zeroizing<Vec<u8>>, (Refusal, &'static str)> {
    let mut gathered = Zeroizing::new(Vec::with_capacity(MAX_BODY_BYTES));
    while let Some(frame) = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
        let frame = frame.map_err(|_| (Refusal::NotJson, "body_broken"))?;
        let Ok(data) = frame.into_data() else {
            continue;
        };
        if gathered.len() + data.len() > MAX_BODY_BYTES {
            return Err((Refusal::BodyTooLarge, "body_bytes"));
        }
        gathered.extend_from_slice(&data);
    }
    Ok(gathered)
}

/// Whether `headers` say the body is JSON: `application/json`, with or
/// without parameters.
fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|essence| essence.trim().eq_ignore_ascii_case("application/json"))
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn json_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers
    }

    async fn read(body: &'static str) -> Result<Submission, (Refusal, &'static str)> {
        Submission::read(&json_headers(), Body::from(body)).await
    }

    // DK-7: the body is read straight into the inventory type. The field's
    // type is `EnteredPassphrase`, so there is no `String` to have been made on
    // the way; the bytes are the ones the window sent, an escaped one included.
    #[tokio::test]
    async fn the_passphrase_is_read_straight_into_the_inventory_type() {
        let submission = read(r#"{"library":"home","passphrase":"correct \"horse\""}"#)
            .await
            .unwrap();
        assert_eq!(submission.library.as_deref(), Some("home"));
        let entered: EnteredPassphrase = submission.passphrase;
        assert_eq!(entered.into_passphrase().as_bytes(), b"correct \"horse\"");
    }

    // LA-22: a body that says it is too large is refused before it is read,
    // and one that turns out to be is refused where it passes the bound.
    #[tokio::test]
    async fn an_over_limit_body_is_refused() {
        let mut headers = json_headers();
        headers.insert(CONTENT_LENGTH, HeaderValue::from(MAX_BODY_BYTES + 1));
        let declared = Submission::read(&headers, Body::from("{}")).await;
        assert!(matches!(declared, Err((Refusal::BodyTooLarge, _))));

        let long = format!(r#"{{"passphrase":"{}"}}"#, "a".repeat(MAX_BODY_BYTES));
        let undeclared = Submission::read(&json_headers(), Body::from(long)).await;
        assert!(matches!(undeclared, Err((Refusal::BodyTooLarge, _))));
    }

    #[tokio::test]
    async fn anything_but_a_json_submission_is_refused() {
        let plain = Submission::read(&HeaderMap::new(), Body::from("{}")).await;
        assert!(matches!(plain, Err((Refusal::NotJsonContent, _))));
        for body in [
            "",
            "{}",
            r#"{"passphrase":7}"#,
            r#"{"passphrase":"a","extra":1}"#,
        ] {
            assert!(
                matches!(read(body).await, Err((Refusal::NotJson, _))),
                "{body}"
            );
        }
    }
}
