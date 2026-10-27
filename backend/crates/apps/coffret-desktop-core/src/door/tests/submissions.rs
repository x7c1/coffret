//! What a submission the door admits does to the Library it names (spec:
//! LA-17, LA-24, DK-1).

use super::*;

// LA-22, LA-24: a wrong Passphrase is answered with the one sentence that
// says so, which carries no path, and the window may try again.
#[test]
fn a_wrong_passphrase_is_said_plainly_and_without_a_path() {
    let case = Case::new();
    let token = case.token(DoorMode::Open);
    case.run(async {
        create("door-wrong").await;
        let answered = case
            .send(case.submission(&token, opening("door-wrong", "not the passphrase")))
            .await;
        assert_refused(&answered, StatusCode::UNPROCESSABLE_ENTITY, "refused");
        let body = answered.json();
        assert_eq!(body["message"], "That is not this Library's Passphrase.");
        assert_eq!(body["ended"], false);
        let state = testing::state_dir().to_str().unwrap();
        for path in [state, "master-key", "/libraries/", ".cfmk"] {
            assert!(!answered.text.contains(path), "{path} in {}", answered.text);
        }
        assert!(case.desktop.served().is_none());
    });
    assert_eq!(
        case.desktop.shared().windows.admit(&token),
        Some(DoorMode::Open),
        "the window may try again"
    );
}

// LA-17, LA-24: a Library opened through the door is served with the
// explorer in front of it; the window's token ends with that success, and a
// ticket issued through the core exchanges once at the explorer's host and is
// refused the second time.
#[test]
fn a_ticket_from_the_core_exchanges_once() {
    let case = Case::new();
    let token = case.token(DoorMode::Open);
    let told = case.events();
    case.run(async {
        create("door-ticket").await;
        let answered = case
            .send(case.submission(&token, opening("door-ticket", PASSPHRASE)))
            .await;
        assert_eq!(answered.status, StatusCode::OK, "{}", answered.text);
        assert_eq!(answered.json()["done"], true);

        let used = case
            .send(case.submission(&token, opening("door-ticket", PASSPHRASE)))
            .await;
        assert_refused(&used, StatusCode::UNAUTHORIZED, "authentication_required");

        let served = case.desktop.served().expect("the Library is served");
        assert_eq!(served.name.as_deref(), Some("door-ticket"));
        assert!(served.unlocked);
        let explorer = case
            .desktop
            .explorer_address()
            .expect("the explorer is served");
        let ticket = case.desktop.issue_ticket().unwrap();
        let (status, body) = exchange(explorer, &ticket).await;
        assert_eq!(status, reqwest::StatusCode::OK, "{body}");
        assert!(body["token"].is_string());
        let (again, _) = exchange(explorer, &ticket).await;
        assert_eq!(again, reqwest::StatusCode::UNAUTHORIZED);
    });
    assert_eq!(*told.lock().unwrap(), [Event::LibraryOpened]);
}

// LA-24: two submissions at once are taken one after the other: of two
// windows opening the same Library, one opens it and the other is told it is
// open, and a third naming another Library is told one is already open.
#[test]
fn submissions_at_once_open_one_library() {
    let case = Case::new();
    let first = case.token(DoorMode::Open);
    let second = case.token(DoorMode::Open);
    let third = case.token(DoorMode::Open);
    let told = case.events();
    case.run(async {
        create("door-race").await;
        create("door-race-other").await;
        let (a, b, c) = tokio::join!(
            case.send(case.submission(&first, opening("door-race", PASSPHRASE))),
            case.send(case.submission(&second, opening("door-race", PASSPHRASE))),
            case.send(case.submission(&third, opening("door-race-other", PASSPHRASE))),
        );
        for answered in [&a, &b] {
            assert_eq!(answered.status, StatusCode::OK, "{}", answered.text);
            assert_eq!(answered.json()["done"], true);
        }
        assert_refused(&c, StatusCode::UNPROCESSABLE_ENTITY, "refused");
        assert_eq!(
            c.json()["message"],
            "Another Library is already open in this app."
        );
    });
    assert_eq!(
        case.desktop.served().unwrap().name.as_deref(),
        Some("door-race")
    );
    assert_eq!(*told.lock().unwrap(), [Event::LibraryOpened]);
}

// DK-1, LA-23: once the Library has locked, the explorer's unlock reaches the
// shell as an event, and a window opened in the unlock mode unlocks the
// Library in place with its Passphrase.
#[test]
fn an_unlock_the_explorer_asks_for_is_taken_through_the_door() {
    let case = Case::new();
    let token = case.token(DoorMode::Open);
    let told = case.events();
    case.run(async {
        create("door-unlock").await;
        let opened = case
            .send(case.submission(&token, opening("door-unlock", PASSPHRASE)))
            .await;
        assert_eq!(opened.status, StatusCode::OK, "{}", opened.text);

        let state = Arc::clone(&case.desktop.shared().served().unwrap().state);
        state.lock();
        assert!(!case.desktop.served().unwrap().unlocked);

        let explorer = case.desktop.explorer_address().unwrap();
        let ticket = case.desktop.issue_ticket().unwrap();
        let (_, exchanged) = exchange(explorer, &ticket).await;
        let browser = exchanged["token"].as_str().unwrap().to_owned();
        let asked = reqwest::Client::new()
            .post(format!("http://{explorer}/api/unlock"))
            .header("origin", format!("http://{explorer}"))
            .header("sec-fetch-site", "same-origin")
            .header("authorization", format!("Bearer {browser}"))
            .send()
            .await
            .unwrap();
        assert_eq!(asked.status(), reqwest::StatusCode::ACCEPTED);
        for _ in 0..100 {
            if told.lock().unwrap().contains(&Event::UnlockRequested) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(told.lock().unwrap().contains(&Event::UnlockRequested));

        let unlocking = case.token(DoorMode::Unlock);
        let described = case.send(case.look(&unlocking)).await.json();
        assert_eq!(described["mode"], "unlock");
        assert_eq!(described["library"]["name"], "door-unlock");
        assert_eq!(described["library"]["needs_attention"], false);
        assert_eq!(
            described["library"]["id"].as_str().map(str::len),
            Some(16),
            "{described}"
        );
        let wrong = case
            .send(case.submission(&unlocking, json!({ "passphrase": "wrong" })))
            .await;
        assert_eq!(
            wrong.json()["message"],
            "That is not this Library's Passphrase."
        );
        let unlocked = case
            .send(case.submission(&unlocking, json!({ "passphrase": PASSPHRASE })))
            .await;
        assert_eq!(unlocked.status, StatusCode::OK, "{}", unlocked.text);
    });
    assert!(case.desktop.served().unwrap().unlocked);
    assert_eq!(
        *told.lock().unwrap(),
        [
            Event::LibraryOpened,
            Event::UnlockRequested,
            Event::LibraryUnlocked
        ]
    );
}
