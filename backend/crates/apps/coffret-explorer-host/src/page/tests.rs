use std::collections::HashMap;

use axum::body::to_bytes;
use axum::http::StatusCode;
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use sha2::{Digest as _, Sha256};

use super::policy::policy_for;
use super::*;

/// A build of the case's own, holding what a real one should never answer
/// beside what it should.
struct Build(HashMap<&'static str, &'static [u8]>);

impl Assets for Build {
    fn file(&self, name: &str) -> Lookup {
        match self.0.get(name) {
            Some(contents) => Lookup::Found((*contents).into()),
            None => Lookup::Missing,
        }
    }
}

fn build() -> Build {
    Build(HashMap::from([
        (INDEX, &b"<!doctype html><title>index</title>"[..]),
        ("assets/app-3f2a.js", &b"console.log(1)"[..]),
        (".env", &b"SECRET=1"[..]),
        ("assets/.hidden", &b"hidden"[..]),
    ]))
}

async fn body(response: Response) -> String {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8_lossy(&body).into_owned()
}

#[test]
fn a_file_is_served_as_its_extension_says() {
    assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
    assert_eq!(
        content_type("assets/index-3f2a.js"),
        "text/javascript; charset=utf-8"
    );
    assert_eq!(
        content_type("assets/index-3f2a.CSS"),
        "text/css; charset=utf-8"
    );
    assert_eq!(content_type("LICENSE"), "application/octet-stream");
}

#[tokio::test]
async fn a_file_in_the_build_is_served_and_a_path_naming_none_is_the_index() {
    let build = build();
    let asset = respond_from(&build, "/assets/app-3f2a.js", HostKind::Production);
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(body(asset).await, "console.log(1)");

    for path in ["/", "/folders/somewhere/deep"] {
        let page = respond_from(&build, path, HostKind::Production);
        assert_eq!(page.status(), StatusCode::OK, "{path}");
        assert!(body(page).await.contains("<title>index"), "{path}");
    }
}

// LA-16: only the build is served: no dotfile, and nothing a traversal reaches.
#[tokio::test]
async fn a_dotfile_or_a_traversal_is_not_served() {
    let build = build();
    for path in [
        "/.env",
        "/assets/.hidden",
        "/%2eenv",
        "/assets/%2Ehidden",
        "/../etc/passwd",
        "/assets/../.env",
        "/assets/%2e%2e/.env",
        "/%2e%2e/%2e%2e/etc/passwd",
        "/assets%2f..%2f.env",
        "/assets\\..\\.env",
        "//etc/passwd",
        "/assets//app-3f2a.js",
        "/a/./b",
        "/%00",
        "/%zz",
    ] {
        let answer = respond_from(&build, path, HostKind::Production);
        assert_eq!(answer.status(), StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body(answer).await, NOT_HERE, "{path}");
    }
}

// LA-16: the policy admits the build's own inline style by its hash and
// nothing inline besides.
#[test]
fn the_policy_admits_each_inline_style_by_its_hash() {
    let index = b"<html><head><style>\n  body { margin: 0; }\n</style></head></html>";
    let policy = policy_for(Some(index));
    // What a browser hashes is the text between the tags, exactly.
    let expected = STANDARD.encode(Sha256::digest(b"\n  body { margin: 0; }\n"));
    assert!(
        policy.contains(&format!("style-src 'self' 'sha256-{expected}'")),
        "{policy}"
    );
    for directive in [
        "default-src 'self'",
        "connect-src 'self'",
        "img-src 'self' blob:",
        "object-src 'none'",
        "base-uri 'none'",
        "frame-ancestors 'none'",
        "form-action 'none'",
    ] {
        assert!(policy.contains(directive), "{directive} in {policy}");
    }
    assert!(!policy.contains("unsafe"), "{policy}");

    assert!(policy_for(None).contains("style-src 'self';"));
}

// LA-16: a build output holding a link is refused before it is embedded, since
// embedding would follow the link out of the build.
#[cfg(unix)]
#[test]
fn a_link_in_the_build_output_is_found() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), "outside the build").unwrap();
    let dist = tempfile::tempdir().unwrap();
    std::fs::create_dir(dist.path().join("assets")).unwrap();
    std::fs::write(dist.path().join("index.html"), "<!doctype html>").unwrap();
    std::fs::write(dist.path().join("assets/app.js"), "").unwrap();
    assert!(super::dist_links::links_in(dist.path()).unwrap().is_empty());

    let link = dist.path().join("assets/linked.txt");
    std::os::unix::fs::symlink(outside.path().join("secret"), &link).unwrap();
    let folder = dist.path().join("elsewhere");
    std::os::unix::fs::symlink(outside.path(), &folder).unwrap();
    assert_eq!(
        super::dist_links::links_in(dist.path()).unwrap(),
        vec![link, folder]
    );
}

/// A build on disk, beside a file outside it that nothing may serve.
#[cfg(unix)]
struct OnDisk {
    /// Holds both directories for as long as the case runs.
    _parent: tempfile::TempDir,
    dist: DistDir,
}

#[cfg(unix)]
const OUTSIDE: &str = "outside the build";

#[cfg(unix)]
fn on_disk() -> OnDisk {
    let parent = tempfile::tempdir().unwrap();
    let outside = parent.path().join("secret.txt");
    std::fs::write(&outside, OUTSIDE).unwrap();
    let root = parent.path().join("dist");
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::write(root.join(INDEX), "<!doctype html><title>index</title>").unwrap();
    std::fs::write(root.join("assets/app-3f2a.js"), "console.log(1)").unwrap();
    std::fs::write(root.join(".env"), "SECRET=1").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("linked.txt")).unwrap();
    std::os::unix::fs::symlink(parent.path(), root.join("up")).unwrap();
    OnDisk {
        _parent: parent,
        dist: DistDir::new(root),
    }
}

// A build on disk serves what is in it, as it stands at the request.
#[cfg(unix)]
#[tokio::test]
async fn a_build_on_disk_is_read_at_each_request() {
    let build = on_disk();
    assert_eq!(
        body(respond_from(
            &build.dist,
            "/assets/app-3f2a.js",
            HostKind::Production
        ))
        .await,
        "console.log(1)"
    );
    assert!(body(respond_from(&build.dist, "/", HostKind::Production))
        .await
        .contains("<title>index"));
    assert!(
        body(respond_from(&build.dist, "/assets", HostKind::Production))
            .await
            .contains("<title>index")
    );

    std::fs::write(build.dist.root().join(INDEX), "<title>rebuilt</title>").unwrap();
    assert!(body(respond_from(&build.dist, "/", HostKind::Production))
        .await
        .contains("rebuilt"));
}

// LA-16: a build on disk serves nothing outside it: no dotfile, no traversal,
// and nothing a symbolic link inside it points at.
#[cfg(unix)]
#[tokio::test]
async fn a_build_on_disk_serves_nothing_outside_it() {
    let build = on_disk();
    for path in [
        "/.env",
        "/../secret.txt",
        "/%2e%2e/secret.txt",
        "/assets/%2e%2e/%2e%2e/secret.txt",
        "/linked.txt",
        "/up/secret.txt",
        "/up/dist/index.html",
    ] {
        let answer = respond_from(&build.dist, path, HostKind::Production);
        assert_eq!(answer.status(), StatusCode::NOT_FOUND, "{path}");
        let said = body(answer).await;
        assert_eq!(said, NOT_HERE, "{path}");
        assert!(!said.contains(OUTSIDE), "{path}");
    }
}

// A build on disk that has not been written yet says so rather than serving
// nothing in silence.
#[cfg(unix)]
#[tokio::test]
async fn a_build_on_disk_with_no_index_says_so() {
    let empty = tempfile::tempdir().unwrap();
    let answer = respond_from(
        &DistDir::new(empty.path().to_path_buf()),
        "/",
        HostKind::Production,
    );
    assert_eq!(answer.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body(answer).await.contains("no index.html"));
}

/// The favicon's plain files, the badged ones a development host answers
/// their names with, and the content type of each name.
#[cfg(unix)]
const FAVICONS: [(&str, &str, &str); 3] = [
    ("favicon.svg", "favicon-dev.svg", "image/svg+xml"),
    ("favicon-32.png", "favicon-32-dev.png", "image/png"),
    (
        "favicon-32-dark.png",
        "favicon-32-dark-dev.png",
        "image/png",
    ),
];

/// A build on disk holding the favicon's plain and badged files, each holding
/// its own name.
#[cfg(unix)]
fn with_favicons() -> OnDisk {
    let build = on_disk();
    for (plain, badged, _) in FAVICONS {
        for name in [plain, badged] {
            std::fs::write(build.dist.root().join(name), name).unwrap();
        }
    }
    build
}

// A development host answers the favicon's paths with the badged files, as
// the names asked for say they are and cached as any file outside `assets/`.
#[cfg(unix)]
#[tokio::test]
async fn a_development_host_answers_the_favicon_with_the_badged_one() {
    let build = with_favicons();
    for (plain, badged, media) in FAVICONS {
        let answer = respond_from(&build.dist, &format!("/{plain}"), HostKind::Development);
        assert_eq!(answer.status(), StatusCode::OK, "{plain}");
        assert_eq!(answer.headers()[CONTENT_TYPE], media, "{plain}");
        assert_eq!(answer.headers()[CACHE_CONTROL], NO_CACHE, "{plain}");
        assert_eq!(body(answer).await, badged, "{plain}");

        // The badged file asked for by its own name is served as it is.
        let own = respond_from(&build.dist, &format!("/{badged}"), HostKind::Development);
        assert_eq!(body(own).await, badged, "{badged}");
    }
    // Everything else is served as a production host serves it.
    assert_eq!(
        body(respond_from(
            &build.dist,
            "/assets/app-3f2a.js",
            HostKind::Development
        ))
        .await,
        "console.log(1)"
    );
}

// A production host answers the favicon's paths with the plain files.
#[cfg(unix)]
#[tokio::test]
async fn a_production_host_answers_the_favicon_with_the_plain_one() {
    let build = with_favicons();
    for (plain, _, media) in FAVICONS {
        let answer = respond_from(&build.dist, &format!("/{plain}"), HostKind::Production);
        assert_eq!(answer.status(), StatusCode::OK, "{plain}");
        assert_eq!(answer.headers()[CONTENT_TYPE], media, "{plain}");
        assert_eq!(answer.headers()[CACHE_CONTROL], NO_CACHE, "{plain}");
        assert_eq!(body(answer).await, plain, "{plain}");
    }
}
