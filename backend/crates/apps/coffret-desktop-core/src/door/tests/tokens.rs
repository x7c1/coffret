//! A window's token: what it admits, and when it ends (spec: LA-23, LA-24).

use super::*;

// LA-23: a submission or a look with no token, another scheme's, or one the
// door never issued is refused, and says the window has to be opened again.
#[test]
fn a_missing_or_unknown_token_is_refused() {
    let case = Case::new();
    let issued = case.token(DoorMode::Open);
    let never_issued = Secret::draw().unwrap().encoded().to_string();
    case.run(async {
        let presented = [
            None,
            Some(format!("Bearer {issued}")),
            Some(format!("Door {never_issued}")),
            Some(format!("Door {}", &issued[1..])),
            Some("Door ".to_owned()),
        ];
        for authorization in &presented {
            let mut submission = case
                .request(Method::POST, "/submit")
                .header("origin", &case.origin)
                .header("sec-fetch-site", "same-origin")
                .header("content-type", "application/json");
            let mut look = case
                .request(Method::GET, "/window")
                .header("sec-fetch-site", "same-origin");
            if let Some(authorization) = authorization {
                submission = submission.header("authorization", authorization);
                look = look.header("authorization", authorization);
            }
            let body = opening("home", "anything").to_string();
            for request in [
                submission.body(Body::from(body)).unwrap(),
                look.body(Body::empty()).unwrap(),
            ] {
                let answered = case.send(request).await;
                assert_refused(
                    &answered,
                    StatusCode::UNAUTHORIZED,
                    "authentication_required",
                );
                assert_eq!(answered.json()["ended"], true);
            }
        }
        // Given twice, even the right one.
        let twice = case
            .request(Method::POST, "/submit")
            .header("origin", &case.origin)
            .header("sec-fetch-site", "same-origin")
            .header("authorization", format!("Door {issued}"))
            .header("authorization", format!("Door {issued}"))
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        let answered = case.send(twice).await;
        assert_refused(
            &answered,
            StatusCode::UNAUTHORIZED,
            "authentication_required",
        );
    });
}

// LA-22, LA-24: a body over the bound is refused before it is read, and counts
// against the window; so does one that is not a submission. The fifth refusal
// ends the token, and the token is refused from then on.
#[test]
fn refused_submissions_count_and_the_fifth_ends_the_token() {
    let case = Case::new();
    let token = case.token(DoorMode::Open);
    case.run(async {
        let mut over = case.submission(&token, json!({}));
        over.headers_mut()
            .insert("content-length", (MAX_BODY_BYTES + 1).into());
        let answered = case.send(over).await;
        assert_refused(&answered, StatusCode::PAYLOAD_TOO_LARGE, "body_too_large");
        assert_eq!(answered.json()["ended"], false);

        let long = "a".repeat(MAX_BODY_BYTES);
        let answered = case
            .send(case.submission(&token, opening("home", &long)))
            .await;
        assert_refused(&answered, StatusCode::PAYLOAD_TOO_LARGE, "body_too_large");

        for _ in 3..MAX_REFUSED {
            let answered = case.send(case.submission(&token, json!({}))).await;
            assert_refused(&answered, StatusCode::BAD_REQUEST, "bad_request");
            assert_eq!(answered.json()["ended"], false);
        }
        let answered = case.send(case.submission(&token, json!(7))).await;
        assert_refused(&answered, StatusCode::BAD_REQUEST, "bad_request");
        assert_eq!(answered.json()["ended"], true, "the fifth ends it");

        let after = case
            .send(case.submission(&token, opening("home", "anything")))
            .await;
        assert_refused(&after, StatusCode::UNAUTHORIZED, "authentication_required");
        let look = case.send(case.look(&token)).await;
        assert_refused(&look, StatusCode::UNAUTHORIZED, "authentication_required");
    });
}

// LA-23: a window learns what it is for behind its token, and looking counts
// as no attempt.
#[test]
fn a_window_learns_its_mode_behind_its_token() {
    let case = Case::new();
    let token = case.token(DoorMode::Open);
    case.run(async {
        create("door-look").await;
        let answered = case.send(case.look(&token)).await;
        assert_eq!(answered.status, StatusCode::OK, "{}", answered.text);
        let described = answered.json();
        assert_eq!(described["mode"], "open");
        assert!(described["libraries"]
            .as_array()
            .unwrap()
            .contains(&json!("door-look")));
    });
    assert!(
        case.desktop.open_door(DoorMode::Unlock).is_err(),
        "nothing is open to unlock"
    );
    assert_eq!(
        case.desktop.shared().windows.admit(&token),
        Some(DoorMode::Open)
    );
}

// LA-18, LA-23: retiring ends every window's token as well as every session.
#[test]
fn retiring_ends_every_windows_token() {
    let case = Case::new();
    let token = case.token(DoorMode::Open);
    case.desktop.retire();
    case.run(async {
        let answered = case.send(case.look(&token)).await;
        assert_refused(
            &answered,
            StatusCode::UNAUTHORIZED,
            "authentication_required",
        );
    });
}
