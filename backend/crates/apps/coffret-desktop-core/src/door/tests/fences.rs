//! The fences every request passes before the door reads it (spec: LA-22).

use super::*;

// LA-22: the door answers only its own authority, on every path: a missing,
// wrong or duplicated `Host` is refused, the page's included.
#[test]
fn a_host_that_is_not_the_doors_own_is_refused() {
    let case = Case::new();
    let token = case.token(DoorMode::Open);
    let other_port = format!("127.0.0.1:{}", case.desktop.door().port().wrapping_add(1));
    let localhost = format!("localhost:{}", case.desktop.door().port());
    case.run(async {
        for path in ["/", "/window", "/submit", "/nowhere"] {
            let hosts: [&[&str]; 4] = [
                &[],
                &[&localhost],
                &[&other_port],
                &[&case.authority, &case.authority],
            ];
            for hosts in hosts {
                let mut request = Request::builder()
                    .method(if path == "/submit" {
                        Method::POST
                    } else {
                        Method::GET
                    })
                    .uri(path)
                    .header("origin", &case.origin)
                    .header("sec-fetch-site", "same-origin")
                    .header("authorization", format!("Door {token}"));
                for host in hosts {
                    request = request.header("host", *host);
                }
                let answered = case.send(request.body(Body::empty()).unwrap()).await;
                assert_refused(&answered, StatusCode::FORBIDDEN, "origin_refused");
            }
        }
        let page = case
            .send(case.request(Method::GET, "/").body(Body::empty()).unwrap())
            .await;
        assert_eq!(page.status, StatusCode::OK);
    });
}

// LA-22: a request bringing more than 8 KiB of headers is refused, on every
// path, the page's included.
#[test]
fn headers_past_their_bound_are_refused() {
    let case = Case::new();
    case.run(async {
        for path in ["/", "/window", "/submit"] {
            let request = case
                .request(Method::GET, path)
                .header("x-padding", "a".repeat(8 * 1024))
                .body(Body::empty())
                .unwrap();
            let answered = case.send(request).await;
            assert_refused(
                &answered,
                StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
                "headers_too_large",
            );
        }
    });
}

// LA-22: a submission carries the door's own `Origin`, exactly and once; a
// missing one, another origin's, `null`, and two are refused.
#[test]
fn a_submission_from_another_origin_or_none_is_refused() {
    let case = Case::new();
    let token = case.token(DoorMode::Open);
    case.run(async {
        let origins: [&[&str]; 5] = [
            &[],
            &["http://evil.invalid"],
            &["null"],
            &["https://127.0.0.1"],
            &[&case.origin, &case.origin],
        ];
        for origins in origins {
            let mut request = case
                .request(Method::POST, "/submit")
                .header("sec-fetch-site", "same-origin")
                .header("authorization", format!("Door {token}"))
                .header("content-type", "application/json");
            for origin in origins {
                request = request.header("origin", *origin);
            }
            let answered = case.send(request.body(Body::from("{}")).unwrap()).await;
            assert_refused(&answered, StatusCode::FORBIDDEN, "origin_refused");
        }
        let look = case
            .send(
                case.request(Method::GET, "/window")
                    .header("origin", "http://evil.invalid")
                    .header("sec-fetch-site", "same-origin")
                    .header("authorization", format!("Door {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_refused(&look, StatusCode::FORBIDDEN, "origin_refused");
    });
    // None of those counted against the window.
    assert_eq!(
        case.desktop.shared().windows.admit(&token),
        Some(DoorMode::Open)
    );
}

// LA-22: `Sec-Fetch-Site` is `same-origin`, given once, on a submission and on
// a look alike; missing or anything else is refused.
#[test]
fn a_fetch_site_other_than_same_origin_is_refused() {
    let case = Case::new();
    let token = case.token(DoorMode::Open);
    case.run(async {
        let sites: [&[&str]; 5] = [
            &[],
            &["same-site"],
            &["cross-site"],
            &["none"],
            &["same-origin", "same-origin"],
        ];
        for sites in sites {
            for method in [Method::POST, Method::GET] {
                let path = if method == Method::POST {
                    "/submit"
                } else {
                    "/window"
                };
                let mut request = case
                    .request(method, path)
                    .header("origin", &case.origin)
                    .header("authorization", format!("Door {token}"))
                    .header("content-type", "application/json");
                for site in sites {
                    request = request.header("sec-fetch-site", *site);
                }
                let answered = case.send(request.body(Body::from("{}")).unwrap()).await;
                assert_refused(&answered, StatusCode::FORBIDDEN, "origin_refused");
            }
        }
    });
}

// LA-22: the door sends no `Access-Control-Allow-*` header on any answer, and
// an `OPTIONS` request is refused like any other method a route does not take.
#[test]
fn no_answer_carries_a_cors_header() {
    let case = Case::new();
    let token = case.token(DoorMode::Open);
    case.run(async {
        let preflight = case
            .request(Method::OPTIONS, "/submit")
            .header("origin", "http://evil.invalid")
            .header("access-control-request-method", "POST")
            .body(Body::empty())
            .unwrap();
        let answers = [
            case.send(preflight).await,
            case.send(case.request(Method::GET, "/").body(Body::empty()).unwrap())
                .await,
            case.send(case.look(&token)).await,
            case.send(case.submission(&token, json!({}))).await,
        ];
        assert_eq!(answers[0].status, StatusCode::METHOD_NOT_ALLOWED);
        for answered in &answers {
            for (name, _) in &answered.headers {
                assert!(
                    !name.as_str().starts_with("access-control-"),
                    "{name} on {}",
                    answered.status
                );
            }
        }
    });
}
