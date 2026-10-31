//! The development host: the explorer's build on disk behind the same fences
//! as the desktop app's host, and a launch file kept ready for a person to
//! open (spec: LA-14, LA-16, LA-18, LA-21).

use std::fs;
use std::net::{SocketAddr, TcpListener as StdListener};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use axum::body::{to_bytes, Body};
use axum::extract::Request;
use axum::http::{Method, StatusCode};
use axum::response::Response;
use axum::Router;
use coffret_explorer_dev::{host, LaunchKeeper, Settings, Tick, LAUNCH_FILE, REFRESH_AFTER};
use coffret_explorer_host::launch_file::LaunchDir;
use coffret_explorer_host::KeyFile;
use tokio::net::TcpListener;
use tower::ServiceExt;

/// What nothing outside the build may serve.
const OUTSIDE: &str = "a file beside the build, outside it";

/// A build, a key file and a launch directory, in a directory of the case's
/// own.
struct Checkout {
    root: tempfile::TempDir,
}

impl Checkout {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let dist = root.path().join("dist");
        fs::create_dir_all(dist.join("assets")).unwrap();
        fs::write(
            dist.join("index.html"),
            "<!doctype html><title>the explorer</title>",
        )
        .unwrap();
        fs::write(dist.join("assets/app-1a2b.js"), "console.log('app')").unwrap();
        fs::write(dist.join(".env"), "SECRET=1").unwrap();
        fs::write(root.path().join("secret.txt"), OUTSIDE).unwrap();
        std::os::unix::fs::symlink(root.path().join("secret.txt"), dist.join("linked.txt"))
            .unwrap();
        std::os::unix::fs::symlink(root.path(), dist.join("up")).unwrap();
        let checkout = Self { root };
        checkout.publish("the-first-run's-key");
        checkout
    }

    fn dist(&self) -> PathBuf {
        self.root.path().join("dist")
    }

    fn key_file(&self) -> PathBuf {
        self.root.path().join("server-key")
    }

    /// A server of another run publishing its key.
    fn publish(&self, key: &str) {
        fs::write(self.key_file(), key).unwrap();
    }

    fn launch_dir(&self) -> LaunchDir {
        LaunchDir::prepare(self.root.path().join("launch")).unwrap()
    }
}

async fn started(checkout: &Checkout, server: SocketAddr) -> (Router, String, TcpListener) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let settings = Settings {
        server,
        key_file: checkout.key_file(),
        dist: checkout.dist(),
        appearance: coffret_device::AppearanceFile::at(checkout.root.path().join("explorer.json")),
    };
    let (router, _) = host(&listener, &settings).expect("the host starts");
    let authority = listener.local_addr().unwrap().to_string();
    (router, authority, listener)
}

async fn get(router: &Router, authority: &str, path: &str) -> Response {
    let request = Request::builder()
        .method(Method::GET)
        .uri(path)
        .header("host", authority)
        .header("origin", format!("http://{authority}"))
        .header("sec-fetch-site", "same-origin")
        .body(Body::empty())
        .unwrap();
    router.clone().oneshot(request).await.unwrap()
}

async fn text(response: Response) -> String {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8_lossy(&body).into_owned()
}

/// A server that answers connections and nothing else, which is all the
/// keeper asks of one.
fn answering() -> StdListener {
    StdListener::bind("127.0.0.1:0").unwrap()
}

/// The ticket the launch file at `path` refreshes to.
fn ticket_in(path: &Path) -> String {
    let page = fs::read_to_string(path).expect("a launch file stands there");
    let (_, after) = page
        .split_once("#launch=")
        .expect("the page names a ticket");
    after
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

// LA-14: the development host admits `/api` by a browser token, as every host
// does, and nothing reaches the server without one.
#[tokio::test]
async fn the_development_host_refuses_api_without_a_token() {
    let checkout = Checkout::new();
    let server = answering();
    let (router, authority, _listener) = started(&checkout, server.local_addr().unwrap()).await;

    for path in ["/api/library", "/api/list?path=books", "/api/"] {
        let response = get(&router, &authority, path).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        assert!(
            text(response).await.contains("authentication_required"),
            "{path}"
        );
    }
}

// LA-16: what the development host serves is the build on disk, and nothing
// outside it.
#[tokio::test]
async fn the_development_host_serves_nothing_outside_the_build() {
    let checkout = Checkout::new();
    let server = answering();
    let (router, authority, _listener) = started(&checkout, server.local_addr().unwrap()).await;

    let index = get(&router, &authority, "/").await;
    assert_eq!(index.status(), StatusCode::OK);
    assert!(index.headers().contains_key("content-security-policy"));
    assert!(text(index).await.contains("the explorer"));
    assert_eq!(
        text(get(&router, &authority, "/assets/app-1a2b.js").await).await,
        "console.log('app')"
    );

    for path in [
        "/.env",
        "/../secret.txt",
        "/%2e%2e/secret.txt",
        "/assets/%2e%2e/%2e%2e/secret.txt",
        "/linked.txt",
        "/up/secret.txt",
    ] {
        let response = get(&router, &authority, path).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert!(!text(response).await.contains(OUTSIDE), "{path}");
    }

    // And what is served is the build as it stands now, rebuilt or not.
    fs::write(checkout.dist().join("index.html"), "<title>rebuilt</title>").unwrap();
    assert!(text(get(&router, &authority, "/").await)
        .await
        .contains("rebuilt"));
}

fn keeper(checkout: &Checkout, server: SocketAddr) -> LaunchKeeper {
    let credentials = coffret_explorer_host::credentials::Credentials::new(Arc::new(
        coffret_explorer_host::credentials::SystemClock,
    ))
    .unwrap();
    LaunchKeeper::new(
        credentials,
        checkout.launch_dir(),
        "127.0.0.1:5173".parse().unwrap(),
        server,
        Arc::new(KeyFile(checkout.key_file())),
    )
}

// LA-18, LA-21: a launch file is written as the server is first seen, again
// once its ticket is spent, and afresh — the host's generation retired — once
// the server's key changes.
#[test]
fn a_fresh_launch_file_is_written_when_the_servers_key_changes() {
    let checkout = Checkout::new();
    let server = answering();
    let mut keeper = keeper(&checkout, server.local_addr().unwrap());
    let path = keeper.path();
    assert!(path.ends_with(LAUNCH_FILE));
    let gate = keeper.credentials().gate();
    let now = Instant::now();

    assert_eq!(keeper.tick(now).unwrap(), Tick::Opened(path.clone()));
    let first = ticket_in(&path);
    let token = gate
        .exchange(&first)
        .expect("the ticket in the file exchanges")
        .token
        .encoded()
        .to_string();
    assert!(gate.authenticate(&token).is_ok());

    // The ticket was spent, so another stands in its place.
    assert_eq!(keeper.tick(now).unwrap(), Tick::Rearmed);
    let second = ticket_in(&path);
    assert_ne!(second, first);
    assert_eq!(keeper.tick(now).unwrap(), Tick::Quiet);

    // A server of another run: every session and ticket of the last one goes.
    let before = keeper.credentials().instance_id();
    checkout.publish("the-second-run's-key");
    assert_eq!(keeper.tick(now).unwrap(), Tick::ServerChanged(path.clone()));
    assert_ne!(keeper.credentials().instance_id(), before);
    assert!(
        gate.authenticate(&token).is_err(),
        "the last run's session is gone"
    );
    assert!(
        gate.exchange(&second).is_err(),
        "the last run's ticket is spent"
    );
    let third = ticket_in(&path);
    assert_ne!(third, second);
    gate.exchange(&third)
        .expect("the fresh file's ticket exchanges");
}

// LA-17, LA-21: a ticket old enough to expire while it is opened is replaced
// before it does.
#[test]
fn a_launch_file_is_replaced_before_its_ticket_expires() {
    let checkout = Checkout::new();
    let server = answering();
    let mut keeper = keeper(&checkout, server.local_addr().unwrap());
    let path = keeper.path();
    let now = Instant::now();

    keeper.tick(now).unwrap();
    let first = ticket_in(&path);
    assert_eq!(keeper.tick(now + REFRESH_AFTER).unwrap(), Tick::Rearmed);
    assert_ne!(ticket_in(&path), first);
}

// LA-15, LA-21: a server that stopped is given no launch file, since a session
// it handed out could not be kept; the one for its next run is written as it
// starts.
#[test]
fn no_launch_file_stands_while_the_server_is_gone() {
    let checkout = Checkout::new();
    let server = answering();
    let address = server.local_addr().unwrap();
    let mut keeper = keeper(&checkout, address);
    let path = keeper.path();
    let now = Instant::now();
    keeper.tick(now).unwrap();
    assert!(path.exists());

    // The server goes, and the host fails closed when it finds it gone.
    drop(server);
    keeper.credentials().retire();
    assert_eq!(keeper.tick(now).unwrap(), Tick::Withdrawn);
    assert!(!path.exists());
    assert_eq!(keeper.tick(now).unwrap(), Tick::Quiet);
    assert!(!path.exists());

    // Its next run publishes another key, and binds the same port.
    checkout.publish("the-next-run's-key");
    let _server = StdListener::bind(address).unwrap();
    assert_eq!(keeper.tick(now).unwrap(), Tick::ServerChanged(path.clone()));
    assert!(path.exists());

    keeper.close().unwrap();
    assert!(!path.exists());
}

// LA-15, LA-21: the key a stopped server left in its Library's directory is no
// server, so a host started before the server writes nothing, and says the
// server is up only once one answers.
#[test]
fn a_key_left_by_a_stopped_server_opens_nothing() {
    let checkout = Checkout::new();
    // A port nothing holds: the listener is dropped as soon as it is bound.
    let address = answering().local_addr().unwrap();
    let mut keeper = keeper(&checkout, address);
    let path = keeper.path();
    let now = Instant::now();

    assert_eq!(keeper.tick(now).unwrap(), Tick::Quiet);
    assert!(!path.exists());

    // The server starts, publishing another key and binding the port.
    checkout.publish("the-next-run's-key");
    let _server = StdListener::bind(address).unwrap();
    assert_eq!(keeper.tick(now).unwrap(), Tick::Opened(path.clone()));
    assert!(path.exists());
}
