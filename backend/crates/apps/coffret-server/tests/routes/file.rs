//! The file route: an Entry this device does not have is fetched and placed,
//! one it has is handed over as it is read, and a path that is not an Entry's
//! is refused before anything is looked up.

use coffret_logging::testing::CapturedLogs;
use tracing::Level;

use crate::support::{bytes, header, json as body_of, Served};
// `size_hint`, and under `_` because the name is `axum::body::Body`'s here. It
// is what the one case about the file route's mechanism reads.
use http_body::Body as _;

// EP-10: an Entry this device never materialized is remote, and asking for its
// bytes is what makes it present — the file is placed in the mapped folder and
// the row says so from then on.
#[tokio::test]
async fn asking_for_a_remote_entry_fetches_it_and_makes_it_present() {
    let served = Served::library().await;
    assert!(!served.holds("albums/2026/spring.jpg"));

    let answer = served.get("/api/file?path=albums/2026/spring.jpg").await;
    assert_eq!(answer.status(), 200);
    assert_eq!(header(&answer, "content-type"), "image/jpeg");
    // The user's own plaintext: no shared cache keeps it and no browser writes
    // it to disk.
    assert_eq!(header(&answer, "cache-control"), "private, no-store");
    assert_eq!(bytes(answer).await, b"spring");
    assert!(served.holds("albums/2026/spring.jpg"));
}

// EP-10, EP-11: the row this device wrote when it placed a file outlives the
// file itself when somebody deletes it out of the mapped folder. Asking for the
// Entry again meets a row and a disk that disagree, which is neither of the two
// states a fetch may place into — so it is declined, in the sentence that says
// what this device wrote there has since changed or gone.
//
// Not put back, and that is the point of the verdict rather than an omission:
// the deletion is a local change the sync flow carries to the Library, and a
// route that quietly re-placed the file would undo what somebody did to their
// own folder before anything had a chance to report it. Not a `500` either: the
// request was answerable, and a reader told only that the server could not
// answer has no way to learn which of their files this was about.
//
// The one Entry of its folder, so that the fill the first answer arms has
// nothing else to bring over and cannot put the file back behind the case's own
// deletion.
#[tokio::test]
async fn a_placed_file_that_is_gone_is_declined_rather_than_fetched_again() {
    let served = Served::library().await;

    let placed = served.get("/api/file?path=books/page-001.png").await;
    assert_eq!(placed.status(), 200);
    assert_eq!(bytes(placed).await, b"page one");
    served.fill_idle().await;
    assert!(served.holds("books/page-001.png"));

    std::fs::remove_file(served.local_path("books/page-001.png"))
        .expect("the placed file can be deleted the way a person would");
    assert!(!served.holds("books/page-001.png"));

    let (status, refusal) = body_of(served.get("/api/file?path=books/page-001.png").await).await;
    assert_eq!(
        status, 409,
        "a row whose file is gone is a finding about this device's folder, not a failure",
    );
    assert_eq!(refusal["error"], "declined");
    assert_eq!(refusal["reason"], "surfaced");
    assert_eq!(refusal["surfaced"], "LocallyChanged");
    assert_eq!(
        refusal["message"], "what this device wrote there has since changed or gone",
        "which is the half of the sentence this case is: gone, and said so",
    );
    served.fill_idle().await;
    assert!(
        !served.holds("books/page-001.png"),
        "and nothing put the file back behind the deletion (spec: EP-10)",
    );
}

// A reader that opens a page and prefetches it, two tabs on one folder, or the
// background fill and a click landing on one Entry: every one of them answers
// with the Entry, and the Container is read once (spec: PK-16).
//
// What one fetch costs is measured rather than assumed — a range read of one
// Entry is several reads of one object, and how many is the fetch's business
// and not this case's. So the case places one Entry on its own first, in a
// folder holding nothing else for the fill to go on with, and everything after
// that is counted in multiples of what it cost: the Containers are the same
// shape, so anything that ran the flow twice for one Entry would show up as
// double.
#[tokio::test]
async fn one_entry_asked_for_twice_at_once_is_fetched_once() {
    let served = Served::library().await;

    let alone = served.get("/api/file?path=books/page-001.png").await;
    assert_eq!(alone.status(), 200);
    served.fill_idle().await;
    let once = served.ranged_reads();
    assert!(once > 0, "fetching an Entry reads a range of its Container");

    // Two callers on one Entry, and the fill they arm going after the other
    // Entry of that folder at the same time. Three Containers are read in all,
    // once each.
    let (first, second) = served
        .get_twice("/api/file?path=albums/2026/summer.jpg")
        .await;
    assert_eq!(first.status(), 200);
    assert_eq!(second.status(), 200);
    assert_eq!(bytes(first).await, b"summer");
    assert_eq!(bytes(second).await, b"summer");
    served.fill_idle().await;
    assert!(
        served.holds("albums/2026/spring.jpg"),
        "the fill brought the rest of the folder over"
    );
    assert_eq!(
        served.ranged_reads(),
        once * 3,
        "everyone after the first waits for its verdict rather than reading again",
    );
}

// A name a browser draws nothing from is served as bytes with no claim about
// them, rather than refused.
#[tokio::test]
async fn a_file_no_browser_draws_is_served_as_bytes() {
    let served = Served::library().await;

    let answer = served.get("/api/file?path=albums/notes.txt").await;
    assert_eq!(answer.status(), 200);
    assert_eq!(header(&answer, "content-type"), "application/octet-stream");
    assert_eq!(bytes(answer).await, b"a note about the albums");
}

// Bytes a browser is handed as `application/octet-stream` are saved rather than
// shown, and with no name to go on a browser saves them as `file`. So the answer
// names them after the last component of the Entry Path, in both spellings a
// browser may read.
#[tokio::test]
async fn a_file_no_browser_draws_is_saved_under_its_own_name() {
    let served = Served::library().await;

    let answer = served.get("/api/file?path=albums/notes.txt").await;
    assert_eq!(answer.status(), 200);
    assert_eq!(
        header(&answer, "content-disposition"),
        "attachment; filename=\"notes.txt\"; filename*=UTF-8''notes.txt",
    );
}

// The name is the person's own and may hold anything a name may: here a quote,
// which would end a naively quoted fallback early, and a letter outside ASCII,
// which a quoted-string cannot carry at all. The fallback holds neither and
// stays one quoted-string; the name a browser uses carries both,
// percent-encoded (RFC 8187).
#[tokio::test]
async fn a_name_that_would_break_a_quoted_string_is_carried_whole_and_safely() {
    let served = Served::library().await;
    served.plant_locally("albums/say \"hi\" to caf\u{e9}.txt", b"hello");

    let answer = served
        .get("/api/file?path=albums/say%20%22hi%22%20to%20caf%C3%A9.txt")
        .await;
    assert_eq!(answer.status(), 200);
    assert_eq!(
        header(&answer, "content-disposition"),
        "attachment; filename=\"say _hi_ to caf_.txt\"; \
         filename*=UTF-8''say%20%22hi%22%20to%20caf%C3%A9.txt",
    );
    assert_eq!(bytes(answer).await, b"hello");
}

// A format the explorer draws is read by the explorer and drawn, never saved,
// so its answer names nothing: no disposition is the inline one already.
#[tokio::test]
async fn a_file_the_explorer_draws_is_answered_with_no_name() {
    let served = Served::library().await;

    let answer = served.get("/api/file?path=albums/cover.png").await;
    assert_eq!(answer.status(), 200);
    assert_eq!(header(&answer, "content-type"), "image/png");
    assert!(
        answer.headers().get("content-disposition").is_none(),
        "{:?}",
        answer.headers(),
    );
}

// A file this device does not have yet reaches Storage for its bytes, and a
// Storage that will not take this device's grant reaches the browser as
// `storage`, at `502`, with nothing placed on disk — and with the reason the
// reader offers a reconnect from, since no retry renews a grant that ran out.
#[tokio::test]
async fn a_file_storage_cannot_answer_for_is_a_bad_gateway() {
    let served = Served::library().await;
    served.halt_storage();

    let (status, refusal) = body_of(served.get("/api/file?path=albums/notes.txt").await).await;
    assert_eq!(status, 502, "{refusal}");
    assert_eq!(refusal["error"], "storage");
    assert_eq!(refusal["reason"], "unauthenticated");
    assert_eq!(
        refusal["message"],
        "the Library's Storage no longer accepts this device's grant"
    );
    assert!(!served.holds("albums/notes.txt"), "and nothing was placed");
}

// The other thing a file on this device is read through is its catalog, and
// one that cannot be used is this server's own state rather than Storage's: it
// is `server`, at `500`, with the sentence saying only that — never the
// `storage` a retry is offered from, which would have somebody pressing a
// button against a disk. Both halves of the route meet it. The read side is a
// file already here, whose place is asked of the catalog before it is opened;
// the open side is a file this device does not have, whose place is asked
// before anything is fetched — so nothing is placed either.
#[tokio::test]
async fn a_catalog_that_cannot_be_used_is_the_servers_own_failure_on_both_sides() {
    let served = Served::library().await;
    let fetched = served.get("/api/file?path=albums/notes.txt").await;
    assert_eq!(fetched.status(), 200, "the file is on this device first");
    let logs = CapturedLogs::capture();
    served.refuse_the_catalog();

    let (status, refusal) = body_of(served.get("/api/file?path=albums/notes.txt").await).await;
    assert_eq!(status, 500, "reading a file that is here: {refusal}");
    assert_eq!(refusal["error"], "server");

    let (status, refusal) = body_of(served.get("/api/file?path=albums/cover.png").await).await;
    assert_eq!(status, 500, "opening one that is not: {refusal}");
    assert_eq!(refusal["error"], "server");
    assert!(!served.holds("albums/cover.png"), "and nothing was placed");

    // What the log keeps is which layer failed, the same way both times.
    let recorded: Vec<String> = logs
        .at(Level::ERROR)
        .into_iter()
        .map(|event| event.field("error"))
        .collect();
    assert_eq!(recorded.len(), 2, "one record per refusal: {recorded:?}");
    for error in &recorded {
        assert!(
            error.starts_with("Device::Index"),
            "the catalog, named as the catalog: {recorded:?}",
        );
    }
}

#[tokio::test]
async fn present_added_and_newly_fetched_files_use_the_streaming_reader() {
    let served = Served::library().await;

    let fetched = served.get("/api/file?path=albums/notes.txt").await;
    assert_eq!(fetched.status(), 200);
    assert_eq!(header(&fetched, "content-length"), "23");
    assert_eq!(bytes(fetched).await, b"a note about the albums");

    let present = served.get("/api/file?path=albums/notes.txt").await;
    assert_eq!(present.status(), 200);
    assert_eq!(header(&present, "content-length"), "23");
    assert_eq!(bytes(present).await, b"a note about the albums");

    served.plant_locally("albums/just-added.txt", b"local addition");
    let added = served.get("/api/file?path=albums/just-added.txt").await;
    assert_eq!(added.status(), 200);
    assert_eq!(header(&added, "content-length"), "14");
    assert_eq!(bytes(added).await, b"local addition");
}

#[cfg(unix)]
#[tokio::test]
async fn the_file_route_never_serves_a_planted_final_symbolic_link() {
    let served = Served::library().await;
    let first = served.get("/api/file?path=albums/notes.txt").await;
    assert_eq!(first.status(), 200);
    assert_eq!(bytes(first).await, b"a note about the albums");

    let outside = tempfile::tempdir().expect("a temporary outside folder");
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, b"outside secret bytes").expect("the outside file");
    served.replace_with_symlink("albums/notes.txt", &secret);

    let refused = served.get("/api/file?path=albums/notes.txt").await;
    assert_eq!(refused.status(), 500);
    assert_ne!(bytes(refused).await, b"outside secret bytes");

    served.replace_with_symlink("albums/just-added.txt", &secret);
    let refused = served.get("/api/file?path=albums/just-added.txt").await;
    assert_ne!(refused.status(), 200);
    assert_ne!(bytes(refused).await, b"outside secret bytes");
}

// The file goes out as it is read rather than being gathered first, so this
// server's memory does not follow the size of what somebody opens — and the
// Library deliberately holds Entries larger than a process (spec: PK-3).
//
// Said about the mechanism, because what a process took in memory is not a thing
// a case can measure without measuring the allocator instead. A body built from
// a reader cannot say how long it is; a body somebody filled always can, which is
// what the listing beside it is here to show. The header still says, because that
// is read off the file rather than off the body.
#[tokio::test]
async fn a_file_is_handed_over_as_a_reader_rather_than_read_whole() {
    let served = Served::library().await;

    let answer = served.get("/api/file?path=albums/notes.txt").await;
    assert_eq!(answer.status(), 200);
    assert_eq!(
        answer.body().size_hint().exact(),
        None,
        "nothing has read the file, so nothing can say how much of it there is",
    );
    assert_eq!(
        header(&answer, "content-length"),
        "23",
        "and the browser is told anyway, out of what the file itself says",
    );

    let listing = served.get("/api/list?path=albums").await;
    assert!(
        listing.body().size_hint().exact().is_some(),
        "an answer this server did build in memory says its own length, which is \
         what the file route no longer does",
    );

    assert_eq!(
        bytes(answer).await,
        b"a note about the albums",
        "and what arrives is the file, byte for byte",
    );
}

// EP-1: a name arrives in whichever spelling the caller's platform keeps, and
// becomes the Library's on the way in. `caf%C3%A9` and `cafe%CC%81` are the two
// spellings of one file, and both name the Entry that is there — refusing the
// second would be telling somebody their own filename is malformed.
#[tokio::test]
async fn a_name_in_another_spelling_names_the_same_entry() {
    let served = Served::library().await;

    for uri in [
        "/api/file?path=albums/caf%C3%A9.jpg",
        "/api/file?path=albums/cafe%CC%81.jpg",
    ] {
        let answer = served.get(uri).await;
        assert_eq!(answer.status(), 200, "{uri}");
        assert_eq!(bytes(answer).await, b"a cafe", "{uri}");
    }
}

// EP-5: the Library holds at most one current Entry at a path, and none there is
// an answer about the request rather than a failure.
#[tokio::test]
async fn a_path_the_library_holds_nothing_at_is_not_found() {
    let served = Served::library().await;

    let (status, refusal) = body_of(served.get("/api/file?path=albums/nothing.jpg").await).await;
    assert_eq!(status, 404);
    assert_eq!(refusal["error"], "no_such_entry");
}

// EP-2: a path that is not one is refused before anything is asked of the
// Library, and the refusal says which rule it broke.
#[tokio::test]
async fn a_path_that_is_not_an_entry_path_is_refused() {
    let served = Served::library().await;

    for uri in [
        "/api/file?path=albums/../../etc/passwd",
        "/api/file?path=/albums/cover.png",
        "/api/file?path=",
        "/api/list?path=albums//2026",
    ] {
        let (status, refusal) = body_of(served.get(uri).await).await;
        assert_eq!(status, 400, "{uri}");
        assert_eq!(refusal["error"], "bad_path", "{uri}");
        assert!(
            refusal["message"]
                .as_str()
                .is_some_and(|message| !message.is_empty()),
            "the refusal says which rule went: {refusal}",
        );
    }
}

// EP-2: the folder routes are held to the shape too, and the message names the
// defect it found — a caller told only that their path was refused has no way
// to find the one component that made it so.
#[tokio::test]
async fn a_query_path_with_a_shape_ep_2_excludes_is_a_bad_path() {
    let served = Served::library().await;

    let (status, refusal) = body_of(served.get("/api/list?path=../x").await).await;
    assert_eq!(status, 400);
    assert_eq!(refusal["error"], "bad_path");
    let message = refusal["message"]
        .as_str()
        .expect("a refusal says something");
    assert!(
        message.starts_with("that is not an Entry Path"),
        "the refusal is the one the explorer branches on: {message}"
    );
    assert!(
        message.contains("`.` or `..` component"),
        "the refusal names the component that made it one: {message}"
    );
}
