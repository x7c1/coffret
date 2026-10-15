use std::fs;
use std::net::SocketAddr;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::{to_bytes, Body};
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use coffret_explorer_host::credentials::{Clock, Credentials, Gate};
use coffret_explorer_host::launch_file::LaunchFileError;
use coffret_explorer_host::{Config, KeyFile, LibraryLock, PageSource};
use coffret_logging::testing::CapturedLogs;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tower::ServiceExt;

use super::launch_dir_path::choose;
use super::*;

/// A clock a case moves by hand.
struct ManualClock(Mutex<Instant>);

impl ManualClock {
    fn advance(&self, by: Duration) {
        *self.0.lock().unwrap() += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap()
    }
}

/// Whether the Library is locked, as the case says.
#[derive(Default)]
struct LockDouble(AtomicBool);

impl LibraryLock for LockDouble {
    fn is_locked(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// A launcher writing into a directory of the case's own, for a host at
/// `127.0.0.1:5173`, and what the case reads about it.
struct Case {
    launcher: Launcher,
    gate: Gate,
    clock: Arc<ManualClock>,
    dir: PathBuf,
    _parent: TempDir,
}

impl Case {
    fn new() -> Self {
        Self::for_host("127.0.0.1:5173".parse().unwrap())
    }

    fn for_host(host: SocketAddr) -> Self {
        let parent = tempfile::tempdir().unwrap();
        let dir = parent.path().join("io.github.x7c1.coffret.test");
        let clock = Arc::new(ManualClock(Mutex::new(Instant::now())));
        let credentials = Credentials::new(clock.clone()).expect("the CSPRNG answers");
        let gate = credentials.gate();
        let launch_dir = prepare_launch_dir(&dir).expect("the directory is made");
        Self {
            launcher: Launcher::new(credentials, launch_dir, host),
            gate,
            clock,
            dir,
            _parent: parent,
        }
    }

    /// The files in the directory, by name.
    fn files(&self) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(&self.dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }
}

fn mode_of(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().mode() & 0o777
}

/// The ticket the page in `file` refreshes to.
fn ticket_in(file: &Path) -> String {
    let page = fs::read_to_string(file).unwrap();
    let (_, after) = page
        .split_once("#launch=")
        .expect("the page names a ticket");
    after
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

// LA-21: opening writes an owner-only file into an owner-only directory, and
// what it holds is a refresh to the host's fixed origin with the ticket in the
// fragment, a link to the same, and nothing that runs.
#[test]
fn opening_writes_an_owner_only_refresh_to_the_hosts_origin() {
    let case = Case::new();
    assert_eq!(mode_of(&case.dir), 0o700);

    let file = case.launcher.open().expect("the file is written");
    assert_eq!(file.parent(), Some(case.dir.as_path()));
    assert_eq!(mode_of(&file), 0o600);
    assert_eq!(
        case.files(),
        ["explorer-0.html"],
        "no temporary file is left"
    );

    let ticket = ticket_in(&file);
    assert_eq!(ticket.len(), 43);
    let page = fs::read_to_string(&file).unwrap();
    let target = format!("http://127.0.0.1:5173/#launch={ticket}");
    assert!(page.contains(&format!(
        "<meta http-equiv=\"refresh\" content=\"0; url={target}\">"
    )));
    assert!(page.contains(&format!("<a href=\"{target}\"")));
    assert_eq!(
        page.matches("http://").count(),
        2,
        "the refresh and the link are the only addresses: {page}"
    );
    let lower = page.to_ascii_lowercase();
    for forbidden in [
        "<script",
        "javascript:",
        "<iframe",
        "<object",
        "<img",
        "<link",
    ] {
        assert!(!lower.contains(forbidden), "{forbidden} in {page}");
    }
    let handlers = lower
        .split(|c: char| c.is_whitespace() || c == '<')
        .filter(|word| word.starts_with("on") && word.contains('='))
        .count();
    assert_eq!(
        handlers, 0,
        "no event handler, which is script by another name"
    );

    // Each opening is a file of its own, with a ticket of its own, so that a
    // browser that has not read the first yet still finds it whole.
    let second = case.launcher.open().unwrap();
    assert_ne!(second, file);
    assert_ne!(ticket_in(&second), ticket);
    assert!(file.exists());
}

// LA-21: the file is written exclusively, under a temporary name, by the
// host's own writer; one already standing at the next name is replaced whole
// rather than written through.
#[test]
fn opening_replaces_rather_than_writes_through() {
    let case = Case::new();
    let squatter = case.dir.join("explorer-0.html");
    fs::write(&squatter, "somebody else's").unwrap();
    fs::set_permissions(&squatter, fs::Permissions::from_mode(0o644)).unwrap();

    let file = case.launcher.open().unwrap();
    assert_eq!(file, squatter);
    assert_eq!(
        mode_of(&file),
        0o600,
        "the file is a new one, not the old one"
    );
    assert!(fs::read_to_string(&file).unwrap().contains("#launch="));
}

// LA-21: what the system is asked to open carries the file's path, and no part
// of the ticket in it.
#[test]
fn the_browser_is_handed_the_path_and_no_part_of_the_ticket() {
    let case = Case::new();
    let file = case.launcher.open().unwrap();
    let ticket = ticket_in(&file);

    let target = browser_target(&file).expect("the path is absolute");
    assert_eq!(target.scheme(), "file");
    assert_eq!(target.to_file_path().unwrap(), file);
    let handed = target.as_str();
    assert!(handed.contains(file.to_str().unwrap()), "{handed}");
    assert!(target.fragment().is_none() && target.query().is_none());
    for part in ticket.as_bytes().windows(8) {
        let part = std::str::from_utf8(part).unwrap();
        assert!(!handed.contains(part), "{part:?} of the ticket in {handed}");
    }
    assert!(browser_target(Path::new("explorer-0.html")).is_err());
}

// LA-21: a directory that is wider than 0700, or is not this account's, is
// refused rather than used.
#[test]
fn a_directory_that_is_not_the_owners_alone_is_refused() {
    let parent = tempfile::tempdir().unwrap();
    let wide = parent.path().join("wide");
    fs::create_dir(&wide).unwrap();
    fs::set_permissions(&wide, fs::Permissions::from_mode(0o750)).unwrap();
    let refused = prepare_launch_dir(&wide).expect_err("a wide directory is refused");
    assert!(
        matches!(
            refused.downcast_ref::<LaunchFileError>(),
            Some(LaunchFileError::TooOpen { mode: 0o750 })
        ),
        "{refused:?}"
    );
    assert_eq!(mode_of(&wide), 0o750, "it is refused, not narrowed");

    // Another account's directory cannot be made without being that account;
    // the superuser's home is one where this machine has it at 0700.
    let foreign = Path::new("/root");
    let me = fs::metadata(parent.path()).unwrap().uid();
    if let Ok(metadata) = fs::symlink_metadata(foreign) {
        if metadata.is_dir() && metadata.uid() != me && metadata.mode() & 0o077 == 0 {
            let refused = prepare_launch_dir(foreign).expect_err("another's is refused");
            assert!(
                matches!(
                    refused.downcast_ref::<LaunchFileError>(),
                    Some(LaunchFileError::ForeignOwner)
                ),
                "{refused:?}"
            );
        }
    }
}

// LA-17, LA-21: a file is removed once its ticket is exchanged, and once it has
// expired, and not before.
#[test]
fn a_file_is_removed_once_its_ticket_is_exchanged_or_expired() {
    let case = Case::new();
    let used = case.launcher.open().unwrap();
    let waiting = case.launcher.open().unwrap();
    let waiting_ticket = ticket_in(&waiting);

    assert_eq!(case.launcher.tidy(), 2, "nothing is spent yet");
    case.gate
        .exchange(&ticket_in(&used))
        .expect("the ticket in the file exchanges");
    assert_eq!(case.launcher.tidy(), 1);
    assert!(!used.exists(), "the exchanged file is removed");
    assert!(waiting.exists());

    case.clock.advance(Duration::from_secs(59));
    assert_eq!(case.launcher.tidy(), 1, "not yet expired");
    case.clock.advance(Duration::from_secs(1));
    assert_eq!(case.launcher.tidy(), 0);
    assert!(!waiting.exists(), "the expired file is removed");
    assert!(case.gate.exchange(&waiting_ticket).is_err());
    assert!(case.files().is_empty());
}

// LA-21: what an earlier run left in the directory is removed as the app
// starts, and what is not a launch file is left alone.
#[test]
fn stale_files_are_removed_at_start() {
    let parent = tempfile::tempdir().unwrap();
    let path = parent.path().join("io.github.x7c1.coffret.test");
    let earlier = prepare_launch_dir(&path).unwrap();
    let clock = Arc::new(ManualClock(Mutex::new(Instant::now())));
    let launcher = Launcher::new(
        Credentials::new(clock).unwrap(),
        earlier,
        "127.0.0.1:5173".parse().unwrap(),
    );
    launcher.open().unwrap();
    launcher.open().unwrap();
    fs::write(path.join(".explorer-2.html.99.0.tmp"), "half").unwrap();
    fs::write(path.join("README"), "somebody's").unwrap();
    // The earlier run ends without tidying, as a crash would.
    drop(launcher);

    prepare_launch_dir(&path).expect("the directory is taken again");
    let left: Vec<_> = fs::read_dir(&path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(left, ["README"]);
}

// LA-21: a file that could not be removed leaves a ticket that is still spent.
#[test]
fn a_failed_removal_does_not_re_enable_the_ticket() {
    let case = Case::new();
    let file = case.launcher.open().unwrap();
    let ticket = ticket_in(&file);
    case.gate
        .exchange(&ticket)
        .expect("the first exchange lands");

    // A directory nobody may remove anything from, its owner included; still
    // the owner's alone.
    fs::set_permissions(&case.dir, fs::Permissions::from_mode(0o500)).unwrap();
    let logs = CapturedLogs::capture();
    let left = case.launcher.tidy();
    fs::set_permissions(&case.dir, fs::Permissions::from_mode(0o700)).unwrap();

    if file.exists() {
        let event = logs.only(tracing::Level::WARN);
        assert_eq!(event.field("kind"), "launch_file_not_removed");
        assert_eq!(event.field("io_error_kind"), "PermissionDenied");
    }
    assert_eq!(left, 0, "the file is let go of all the same");
    assert!(
        case.gate.exchange(&ticket).is_err(),
        "the ticket stays spent"
    );
    case.clock.advance(Duration::from_secs(3600));
    assert!(case.gate.exchange(&ticket).is_err());
}

// LA-18: the server stopping, and the app quitting, retire the generation:
// every session and every ticket ends, and every file goes. Within one run the
// Library served never changes — the app serves the first one opened until it
// ends, and an unlock in place refuses any other — so this is also the only
// way a Library's sessions end but by expiry or revocation.
#[test]
fn retiring_ends_every_session_and_ticket_and_removes_every_file() {
    let case = Case::new();
    let before = case.gate.instance_id();
    let exchanged = case
        .gate
        .exchange(&ticket_in(&case.launcher.open().unwrap()))
        .unwrap();
    let token = exchanged.token.encoded().to_string();
    let waiting = case.launcher.open().unwrap();
    let waiting_ticket = ticket_in(&waiting);
    assert!(case.gate.authenticate(&token).is_ok());

    case.launcher.retire();

    assert!(case.gate.authenticate(&token).is_err(), "the session ended");
    assert!(
        case.gate.exchange(&waiting_ticket).is_err(),
        "the ticket was spent"
    );
    assert!(case.files().is_empty(), "every file went");
    assert_ne!(case.gate.instance_id(), before, "a new generation began");
}

// LA-18: *Disconnect all browser tabs* revokes every session, and spends every
// ticket not yet exchanged, so that none makes a session afterwards.
#[test]
fn disconnecting_all_revokes_every_session() {
    let case = Case::new();
    let tokens: Vec<String> = (0..3)
        .map(|_| {
            let file = case.launcher.open().unwrap();
            case.gate
                .exchange(&ticket_in(&file))
                .unwrap()
                .token
                .encoded()
                .to_string()
        })
        .collect();
    let waiting = ticket_in(&case.launcher.open().unwrap());
    let instance = case.gate.instance_id();

    case.launcher.disconnect_all();

    for token in &tokens {
        assert!(case.gate.authenticate(token).is_err());
    }
    assert!(case.gate.exchange(&waiting).is_err());
    assert!(case.files().is_empty());
    assert_eq!(case.gate.instance_id(), instance, "the generation stands");

    // The tray's *Open the explorer* still gives a working tab afterwards.
    let file = case.launcher.open().unwrap();
    assert!(case.gate.exchange(&ticket_in(&file)).is_ok());
}

/// A host in front of nothing, with a Library the case locks, issuing from a
/// launcher; and the host's `Host` authority.
async fn host_with_lock() -> (Router, Case, Arc<LockDouble>, String, TcpListener) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let case = Case::for_host(address);
    let lock = Arc::new(LockDouble::default());
    // A key to pin as the first session is made; the server it is for is
    // nowhere, which is past what these cases ask about.
    let key = case.dir.parent().unwrap().join("server-key");
    fs::write(&key, "a server key").unwrap();
    let router = coffret_explorer_host::router(
        &listener,
        Config {
            server: "127.0.0.1:9".parse().unwrap(),
            key: Arc::new(KeyFile(key)),
            lock: lock.clone(),
            page: PageSource::Embedded,
        },
        case.gate.clone(),
    )
    .unwrap();
    (router, case, lock, address.to_string(), listener)
}

fn page_request(authority: &str, method: Method, uri: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("host", authority)
        .header("origin", format!("http://{authority}"))
        .header("sec-fetch-site", "same-origin")
}

// LA-14, LA-18: an idle lock keeps the tabs' sessions, which the host refuses
// data with `423` until the Library is unlocked; nothing the app does on a lock
// ends them.
#[tokio::test]
async fn an_idle_lock_keeps_sessions_and_refuses_data_until_unlocked() {
    let (router, case, lock, authority, _listener) = host_with_lock().await;
    let file = case.launcher.open().unwrap();
    let exchanged = router
        .clone()
        .oneshot(
            page_request(&authority, Method::POST, "/auth/launch")
                .header("authorization", format!("Launch {}", ticket_in(&file)))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exchanged.status(), StatusCode::OK);
    let body = to_bytes(exchanged.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = body["token"].as_str().unwrap().to_owned();
    assert_eq!(case.launcher.tidy(), 0, "the exchanged file goes");

    let send = |method: Method, uri: &str| {
        router.clone().oneshot(
            page_request(&authority, method, uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
    };

    lock.0.store(true, Ordering::SeqCst);
    let refused = send(Method::GET, "/api/list").await.unwrap();
    assert_eq!(refused.status(), StatusCode::LOCKED);
    let session = send(Method::GET, "/auth/session").await.unwrap();
    assert_eq!(session.status(), StatusCode::OK, "the session survives");
    let session = to_bytes(session.into_body(), usize::MAX).await.unwrap();
    let session: serde_json::Value = serde_json::from_slice(&session).unwrap();
    assert_eq!(session["library"], "locked");
    assert!(case.gate.authenticate(&token).is_ok());

    lock.0.store(false, Ordering::SeqCst);
    let let_through = send(Method::GET, "/api/list").await.unwrap();
    assert_ne!(let_through.status(), StatusCode::LOCKED);
    assert_ne!(let_through.status(), StatusCode::UNAUTHORIZED);
}

// EL-1: opening the explorer, the exchange, the files' removal, and the
// revocations leave no ticket, no URL and no path in any event.
#[test]
fn opening_leaves_no_ticket_url_or_path_in_any_event() {
    let case = Case::new();
    let logs = CapturedLogs::capture();

    let file = case.launcher.open().unwrap();
    let ticket = ticket_in(&file);
    let target = browser_target(&file).unwrap();
    case.gate.exchange(&ticket).unwrap();
    case.launcher.tidy();
    case.launcher.open().unwrap();
    case.launcher.disconnect_all();
    case.launcher.open().unwrap();
    case.launcher.retire();

    let events = logs.events();
    let kinds: Vec<String> = events.iter().map(|event| event.field("kind")).collect();
    for kind in [
        "issue",
        "launch_file_written",
        "exchange",
        "launch_file_removed",
        "revocation",
    ] {
        assert!(kinds.iter().any(|seen| seen == kind), "{kind} in {kinds:?}");
    }
    let file_text = file.to_str().unwrap();
    let dir_text = case.dir.to_str().unwrap();
    logs.assert_free_of(&[
        &ticket,
        target.as_str(),
        "#launch=",
        "127.0.0.1:5173",
        file_text,
        dir_text,
        "explorer-0.html",
        "io.github.x7c1.coffret.test",
    ]);
}

// LA-21: the directory is the identifier's own, under the runtime directory
// where there is one and under the state directory where there is not.
#[test]
fn the_directory_is_the_identifiers_own() {
    let state = || Ok(PathBuf::from("/state/coffret"));
    assert_eq!(
        choose(
            Some("/run/user/1000".into()),
            "io.github.x7c1.coffret",
            state
        )
        .unwrap(),
        Path::new("/run/user/1000/io.github.x7c1.coffret")
    );
    assert_eq!(
        choose(None, "io.github.x7c1.coffret.dev", state).unwrap(),
        Path::new("/state/coffret/explorer-launch/io.github.x7c1.coffret.dev")
    );
    assert_eq!(
        choose(Some("relative".into()), "io.github.x7c1.coffret", state).unwrap(),
        Path::new("/state/coffret/explorer-launch/io.github.x7c1.coffret"),
        "a relative runtime directory is not one (XDG)"
    );
    for identifier in ["", ".", "..", "a/b", ".hidden"] {
        assert!(choose(None, identifier, state).is_err(), "{identifier:?}");
    }
}
