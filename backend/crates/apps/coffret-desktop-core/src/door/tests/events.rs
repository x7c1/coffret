//! What the door's events leave out (spec: EL-1, LA-25).

use super::*;

// EL-1, LA-25: opening a window, a refused and a landed submission, a ticket,
// its exchange and the retirement leave no door token, ticket, URL or address
// in any event.
#[test]
fn no_event_carries_a_token_a_ticket_or_a_url() {
    let case = Case::new();
    // Coffret's own events: the Storage SDK's and the HTTP client's traces are
    // theirs, and the installed log keeps only these (spec: EL-1).
    let logs = CapturedLogs::capture_target("coffret");
    let url = case
        .desktop
        .open_door(DoorMode::Open)
        .unwrap()
        .expose()
        .to_owned();
    let (_, token) = url.split_once("#door=").unwrap();
    let (explorer, ticket) = case.run(async {
        create("door-events").await;
        case.send(case.submission(token, opening("door-events", "wrong")))
            .await;
        let answered = case
            .send(case.submission(token, opening("door-events", PASSPHRASE)))
            .await;
        assert_eq!(answered.status, StatusCode::OK, "{}", answered.text);
        let explorer = case.desktop.explorer_address().unwrap();
        let ticket = case.desktop.issue_ticket().unwrap().to_string();
        exchange(explorer, &ticket).await;
        (explorer, ticket)
    });
    case.desktop.retire();

    let text = logs.text();
    for kind in [
        "door_window_issued",
        "door_attempt_refused",
        "door_window_ended",
        "issue",
        "exchange",
        "revocation",
    ] {
        assert!(
            text.contains(&format!("\"kind\":\"{kind}\"")),
            "{kind} in {text}"
        );
    }
    let door = case.desktop.door().to_string();
    let explorer = explorer.to_string();
    logs.assert_free_of(&[token, &ticket, &url, "#door=", "#launch=", &door, &explorer]);
}

// EL-1, EL-3, EL-4: a body that is not a submission is logged as the one
// `door_refusal` event, with the way the JSON reader classified it — and,
// where the body broke off, the kind of the I/O failure underneath — and with
// nothing of what the body said.
#[test]
fn a_body_that_is_not_json_is_logged_by_its_classification_alone() {
    let case = Case::new();
    let token = case.token(DoorMode::Open);
    let logs = CapturedLogs::capture_target("coffret");
    let secret = "horse-battery-staple";
    case.run(async {
        for (body, failure) in [
            (format!(r#"{{"passphrase":"{secret}"#), "eof"),
            (format!(r#"{{"passphrase":{secret}}}"#), "syntax"),
            (format!(r#"{{"passphrase":"{secret}","extra":1}}"#), "data"),
        ] {
            let answered = case
                .send(case.raw_submission(&token, Body::from(body)))
                .await;
            assert_refused(&answered, StatusCode::BAD_REQUEST, "bad_request");
            let refusal = last_refusal(&logs);
            assert_eq!(refusal.field("reason"), "body", "{}", logs.text());
            assert_eq!(refusal.field("failure"), failure, "{}", logs.text());
        }

        let answered = case
            .send(case.raw_submission(&token, Body::new(Broken)))
            .await;
        assert_refused(&answered, StatusCode::BAD_REQUEST, "bad_request");
        let refusal = last_refusal(&logs);
        assert_eq!(refusal.field("reason"), "body_broken", "{}", logs.text());
        assert_eq!(refusal.field("failure"), "broken", "{}", logs.text());
        assert_eq!(
            refusal.field("io_kind"),
            "ConnectionReset",
            "{}",
            logs.text()
        );
    });

    // Neither the body nor where in it the reader stopped, which the reader's
    // own rendering names as a line and a column.
    logs.assert_free_of(&[secret, "column"]);
}

/// The last `door_refusal` event the capture holds.
fn last_refusal(logs: &CapturedLogs) -> coffret_logging::testing::LoggedEvent {
    logs.events()
        .into_iter()
        .rev()
        .find(|event| event.message() == "a request to the door was refused")
        .unwrap_or_else(|| panic!("a door_refusal event in {}", logs.text()))
}

/// A body whose connection breaks before its first frame arrives.
struct Broken;

impl http_body::Body for Broken {
    type Data = axum::body::Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        std::task::Poll::Ready(Some(Err(std::io::Error::from(
            std::io::ErrorKind::ConnectionReset,
        ))))
    }
}
