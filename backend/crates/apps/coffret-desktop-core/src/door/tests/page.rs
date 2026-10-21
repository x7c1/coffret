//! The page the door serves a window (spec: LA-22).

use super::*;

// LA-22: the page is the door's own, with a policy that lets it load nothing
// but its own script and style and reach nothing but its own origin.
#[test]
fn the_page_is_served_with_its_policy() {
    let case = Case::new();
    case.run(async {
        for (path, kind) in [
            ("/", "text/html"),
            ("/unlock.js", "text/javascript"),
            ("/unlock.css", "text/css"),
        ] {
            let answered = case
                .send(case.request(Method::GET, path).body(Body::empty()).unwrap())
                .await;
            assert_eq!(answered.status, StatusCode::OK, "{path}");
            let content_type = answered.headers["content-type"].to_str().unwrap();
            assert!(content_type.starts_with(kind), "{path}: {content_type}");
            let policy = answered.headers["content-security-policy"]
                .to_str()
                .unwrap();
            assert!(policy.contains("default-src 'none'"), "{policy}");
            assert!(policy.contains("connect-src 'self'"), "{policy}");
            assert_eq!(answered.headers["x-frame-options"], "DENY");
            assert_eq!(answered.headers["cache-control"], "no-store");
        }
        let script = case
            .send(
                case.request(Method::GET, "/unlock.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert!(script.text.contains("history.replaceState"));
    });
}
