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
