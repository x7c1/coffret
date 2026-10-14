//! What a server's Entry fetches do not ask Storage again.
//!
//! A command line fetches once and catches up, lists, reads the Keyring and
//! reads the Container's front for that one Entry. A server fetches hundreds of
//! Entries over one catalog, and none of those answers changes between two of
//! them — so it keeps the listing its last catch-up walked, the committed key
//! table, and each Container's front, and reads each again only where it is no
//! longer true (spec: KL-1, FM-2, PK-20).
//!
//! Every case here counts what reached Storage, in order: a listing page, a
//! whole object (a Keyring replica), a Container's header, its meta section,
//! and a parcel.

use coffret_logging::testing::CapturedLogs;
use serde_json::json;
use tracing::Level;

use crate::support::{bytes, fill, json as body_of, sync, Asked, Served};

/// How many of the calls were whole-object reads, which is what reading a
/// Keyring replica is.
fn whole_reads(asked: &[Asked]) -> usize {
    asked.iter().filter(|asked| **asked == Asked::Whole).count()
}

/// How many were listing pages.
fn listings(asked: &[Asked]) -> usize {
    asked
        .iter()
        .filter(|asked| **asked == Asked::Listed)
        .count()
}

/// How many catch-ups an Entry fetch made, out of the line each one writes.
fn catch_ups_for_a_fetch(logs: &CapturedLogs) -> usize {
    logs.at(Level::INFO)
        .into_iter()
        .filter(|event| {
            event.message() == "the catalog was caught up with the Library"
                && event.field("operation") == "fetch"
        })
        .count()
}

/// Opens one file and waits for the fill it arms, so that what a case counts
/// next is its own.
async fn open(served: &Served, path: &str, content: &[u8]) {
    let answer = served.get(&format!("/api/file?path={path}")).await;
    assert_eq!(answer.status(), 200, "{path} opens");
    assert_eq!(bytes(answer).await, content);
    served.fill_idle().await;
}

/// A server whose catalog knows two Containers this device never read, each
/// alone in its folder so the fill a fetch arms has nothing to bring over.
async fn two_containers() -> Served {
    let served = Served::library().await;
    served.commit_elsewhere("solo/one.png", b"one").await;
    served.commit_elsewhere("solo2/two.png", b"two").await;
    served.start_up().await;
    served.forget_reads();
    served
}

// A reader on its second page of an unfetched book: the Container's front and
// the Keyring were read for the first, the catalog was caught up as the server
// started, and none of that has changed since. So the one thing asked of
// Storage is the parcel the page lies in.
#[tokio::test]
async fn a_second_entry_of_one_container_costs_its_parcel_and_nothing_else() {
    let served = Served::library().await;
    // One Pack, three folders: the first page in parcel 0, the last in parcel 1,
    // and between them one that runs across the boundary. Each alone in its
    // folder, so opening one arms a fill with nothing to bring over.
    for (index, (path, len)) in [
        ("scans/a/000.jpg", 512 * 1024),
        ("scans/b/001.jpg", 1024 * 1024),
        ("scans/c/002.jpg", 256 * 1024),
    ]
    .into_iter()
    .enumerate()
    {
        served
            .commit_elsewhere(path, &vec![0x40 + index as u8; len])
            .await;
    }
    served.pack_elsewhere("scans").await;
    served.start_up().await;
    served.forget_reads();

    open(&served, "scans/a/000.jpg", &vec![0x40; 512 * 1024]).await;
    served.forget_reads();

    open(&served, "scans/c/002.jpg", &vec![0x42; 256 * 1024]).await;
    let asked = served.asked();
    assert_eq!(asked.len(), 1, "one read and no more: {asked:?}");
    assert!(asked[0].is_parcel(), "and it is the parcel: {asked:?}");
}

// A second Container in the same process: its front is new, so it is read —
// header, then meta section — and then its parcel. The listing and the key
// table are the same ones the first fetch used.
#[tokio::test]
async fn an_entry_of_another_container_reads_its_front_and_parcel_and_no_listing_or_keyring() {
    let served = two_containers().await;

    open(&served, "solo/one.png", b"one").await;
    let first = served.asked();
    assert_eq!(
        listings(&first),
        0,
        "the listing the startup catch-up walked is the one used: {first:?}",
    );
    assert_eq!(whole_reads(&first), 1, "the Keyring, once: {first:?}");
    served.forget_reads();

    open(&served, "solo2/two.png", b"two").await;
    let second = served.asked();
    assert_eq!(second.len(), 3, "{second:?}");
    assert!(second[0].is_header(), "{second:?}");
    assert!(second[1].is_meta(), "{second:?}");
    assert!(second[2].is_parcel(), "{second:?}");
}

// Another device commits, which names a new Keyring in the head it writes
// (spec: KL-1). A refresh brings this catalog there, and the next fetch reads
// the Keyring that head names: the Container it opens is keyed only there, so a
// fetch that reused the old table could not open it at all.
#[tokio::test]
async fn a_catch_up_to_a_new_keyring_has_the_next_fetch_read_it() {
    let served = two_containers().await;
    open(&served, "solo/one.png", b"one").await;

    served.commit_elsewhere("solo3/three.png", b"three").await;
    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(refreshed["advanced"], true);
    served.forget_reads();

    open(&served, "solo3/three.png", b"three").await;
    let asked = served.asked();
    assert_eq!(
        listings(&asked),
        0,
        "the refresh's listing is kept: {asked:?}"
    );
    assert_eq!(
        whole_reads(&asked),
        1,
        "the newly committed Keyring: {asked:?}"
    );

    // And kept in its turn: the same table opens the next Container.
    served.forget_reads();
    open(&served, "solo2/two.png", b"two").await;
    assert_eq!(whole_reads(&served.asked()), 0);
}

// A lock takes the keys and everything kept beside them (spec: DK-2, DK-4).
// The unlock starts from nothing, so the first fetch after it reads the
// Keyring again.
#[tokio::test]
async fn a_fetch_after_a_lock_reads_the_keyring_again() {
    let served = two_containers().await;
    open(&served, "solo/one.png", b"one").await;

    served.lock();
    served.unlock();
    served.forget_reads();

    open(&served, "solo2/two.png", b"two").await;
    let asked = served.asked();
    assert_eq!(whole_reads(&asked), 1, "the Keyring, read again: {asked:?}");
}

// A fill walking a book: the first Entry's fetch reads the Keyring and the
// Container's front, and every one after it reads parcels and nothing else.
#[tokio::test]
async fn a_fill_moving_through_one_container_reads_only_parcels_after_the_first() {
    let served = Served::library().await;
    let pages: [(&str, usize); 4] = [
        ("scans/vol/000.jpg", 300 * 1024),
        ("scans/vol/001.jpg", 1536 * 1024),
        ("scans/vol/002.jpg", 400 * 1024),
        ("scans/vol/003.jpg", 400 * 1024),
    ];
    for (index, (path, len)) in pages.iter().enumerate() {
        served
            .commit_elsewhere(path, &vec![0x40 + index as u8; *len])
            .await;
    }
    served.pack_elsewhere("scans").await;
    served.start_up().await;
    served.forget_reads();

    served.arm_fill("scans/vol");
    served.fill_idle().await;
    for (path, _) in pages {
        assert!(served.holds(path), "{path} was brought over");
    }

    let asked = served.asked();
    assert_eq!(listings(&asked), 0, "{asked:?}");
    assert_eq!(whole_reads(&asked), 1, "the Keyring, once: {asked:?}");
    assert_eq!(
        asked.iter().filter(|asked| asked.is_header()).count(),
        1,
        "{asked:?}"
    );
    assert_eq!(
        asked.iter().filter(|asked| asked.is_meta()).count(),
        1,
        "{asked:?}"
    );
    let first_parcel = asked
        .iter()
        .position(Asked::is_parcel)
        .expect("a parcel was read");
    assert!(
        asked[first_parcel..].iter().all(Asked::is_parcel),
        "after the first parcel, nothing but parcels: {asked:?}",
    );
}

// Another device packed a folder and the Containers its files lived in are
// gone from Storage. This catalog has not replayed that, so it names a
// Container that is not there; the fetch catches the catalog up once, through
// the run a refresh takes, and places the Entry out of the Pack holding it now.
#[tokio::test]
async fn a_container_gone_from_storage_is_caught_up_with_once_and_read_where_it_is_now() {
    let served = Served::library().await;
    served.pack_elsewhere("albums/2026").await;
    served
        .purge_served_container_of("albums/2026/spring.jpg")
        .await;
    let logs = CapturedLogs::capture();

    let answer = served.get("/api/file?path=albums/2026/spring.jpg").await;
    assert_eq!(answer.status(), 200);
    assert_eq!(bytes(answer).await, b"spring");
    served.fill_idle().await;

    assert_eq!(catch_ups_for_a_fetch(&logs), 1, "{}", logs.text());
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        work["catalog"],
        json!({ "state": "caught_up", "stopped": null }),
        "the catch-up is the catalog's own, and says so: {work}",
    );
}

// The same, where the other device deleted the Entry: once caught up, the
// catalog holds nothing at that path, and that is the answer.
#[tokio::test]
async fn a_container_gone_with_its_entry_answers_that_the_entry_is_not_current() {
    let served = Served::library().await;
    served.delete_elsewhere("books/page-001.png").await;
    served.purge_served_container_of("books/page-001.png").await;
    let logs = CapturedLogs::capture();

    let (status, refusal) = body_of(served.get("/api/file?path=books/page-001.png").await).await;
    assert_eq!(status, 404);
    assert_eq!(refusal["error"], "no_such_entry");

    assert_eq!(catch_ups_for_a_fetch(&logs), 1, "{}", logs.text());
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["catalog"]["state"], "caught_up", "{work}");
}

// And where the catalog was not stale: a catalog at the head that names a
// Container Storage does not hold has met a Library that does not hold what it
// says. One catch-up, one more attempt, and the refusal is the answer — not
// another round.
#[tokio::test]
async fn a_container_gone_from_a_current_catalog_is_refused_after_one_catch_up() {
    let served = Served::library().await;
    served.purge_served_container_of("books/page-001.png").await;
    let logs = CapturedLogs::capture();

    let answer = served.get("/api/file?path=books/page-001.png").await;
    assert!(
        answer.status().is_server_error(),
        "a Container the Library names and Storage does not hold is refused: {}",
        answer.status(),
    );

    assert_eq!(catch_ups_for_a_fetch(&logs), 1, "{}", logs.text());
}

/// Whether the fill the last opened file armed says the Keyring is short,
/// which is where a person who only opens files hears of it (spec: KL-15).
async fn says_short(served: &Served) -> bool {
    let (_, work) = body_of(served.get("/api/work").await).await;
    fill(&work)["findings"].as_array().is_some_and(|findings| {
        findings
            .iter()
            .any(|finding| finding["reason"] == "keyring_degraded")
    })
}

// A refresh that leaves the head where it was still leaves a whole set's key
// table kept: nothing a catch-up learns makes a whole set short, so the saving
// holds in the case that matters for reading (spec: KL-1).
#[tokio::test]
async fn a_refresh_that_advances_nothing_keeps_a_whole_sets_key_table() {
    let served = two_containers().await;
    open(&served, "solo/one.png", b"one").await;

    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(refreshed["advanced"], false);
    served.forget_reads();

    open(&served, "solo2/two.png", b"two").await;
    let asked = served.asked();
    assert_eq!(whole_reads(&asked), 0, "no Keyring read: {asked:?}");
}

// KL-15: a finding is reported while a read still observes the condition, and
// no longer. The set is found short, then repaired elsewhere under the same
// commitment — a repair rewrites replicas and leaves the head alone
// (spec: KL-13, KL-14) — and a refresh is where this process learns of it. The
// next fetch reads the Keyring again rather than repeating what the first read
// found.
#[tokio::test]
async fn a_refresh_after_a_repair_elsewhere_stops_the_short_finding() {
    let served = two_containers().await;
    let lost = served.degrade_the_keyring().await;
    open(&served, "solo/one.png", b"one").await;
    assert!(says_short(&served).await, "the first read finds it short");

    served.put_back(lost).await;
    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(refreshed["advanced"], false, "a repair commits nothing");
    served.forget_reads();

    open(&served, "solo2/two.png", b"two").await;
    let asked = served.asked();
    assert_eq!(whole_reads(&asked), 1, "the Keyring, read again: {asked:?}");
    assert!(
        !says_short(&served).await,
        "and found whole, so not said short again",
    );
}

// The repair this device makes itself: a sync examines the committed set and
// repairs it before it commits (spec: KL-11, KL-13). Its commit is refused
// after that, so the commitment stays where it was with the set whole again,
// and the next fetch learns so by reading it rather than repeating the finding
// the key table was kept with.
#[tokio::test]
async fn a_sync_that_repaired_the_set_stops_the_short_finding() {
    let served = two_containers().await;
    served.degrade_the_keyring().await;
    open(&served, "solo/one.png", b"one").await;
    assert!(says_short(&served).await, "the first read finds it short");

    served.refuse_commits();
    served.upload("albums", &[("late.jpg", b"late")]).await;
    served.sync_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        sync(&work)["status"],
        "stopped",
        "the commit was refused after the repair: {work}",
    );
    served.forget_reads();

    open(&served, "solo2/two.png", b"two").await;
    let asked = served.asked();
    assert_eq!(whole_reads(&asked), 1, "the Keyring, read again: {asked:?}");
    assert!(
        !says_short(&served).await,
        "and found whole, so not said short again",
    );
}
