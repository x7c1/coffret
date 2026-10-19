//! A dropped book's choice of a Pack, kept on the device across a server that
//! stopped, and the freeze a server started afterwards takes up from it
//! (spec: PK-17, LA-12).

use serde_json::json;

use crate::support::{freeze, json as body_of, listing_of, rows_of, sync, written, Served};

/// The three pages of the book these cases bring in.
const BOOK: [(&str, &[u8]); 3] = [
    ("page-001.jpg", b"the first page"),
    ("page-002.jpg", b"the second page"),
    ("page-003.jpg", b"the third page"),
];

/// Every row of `folder` is a page packed into a Pack and present here.
async fn packed(served: &Served, folder: &str) {
    assert_eq!(
        rows_of(served, folder).await,
        BOOK.map(|(name, _)| (name.to_owned(), "present".to_owned(), "pack".to_owned())),
        "every page of {folder} is an Entry this device has, inside a Pack",
    );
}

/// The whole of what the reservation is for. A drop asked for a Pack, its
/// pages landed, and the server stopped before the freeze committed — here a
/// freeze whose commit Storage refused, which leaves a Pack uploaded, its spool
/// on the disk and its pending row in the catalog, as a process killed during
/// the put would. A server started afterwards takes the book up on its own,
/// settles what the dead run left before it builds, and commits one Pack of
/// exactly the pages that landed: no page is ever a Container of its own
/// (spec: PK-17, OC-9, PK-7).
#[tokio::test]
async fn a_book_whose_server_stopped_is_packed_by_the_next_one() {
    let mut served = Served::library().await;
    served.refuse_commits();

    let (status, answer) = body_of(served.upload_book("scans/vol-1", &BOOK).await).await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer).len(), BOOK.len());
    served.freeze_idle().await;
    let (_, stopped) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&stopped)["status"], "stopped");
    assert_eq!(
        served.pending_rows().await,
        1,
        "the dead run's Pack is pending"
    );
    assert!(served.spools_left() > 0, "and its spool is on the disk");
    assert_eq!(served.reservations().await, 1);

    served.restart();
    served.accept_commits();
    let (_, fresh) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        fresh["freeze"],
        serde_json::Value::Null,
        "the new server holds nothing of the old one's work (spec: LA-12): {fresh}",
    );

    served.start_up().await;
    served.freeze_idle().await;
    let (_, resumed) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&resumed)["folder"], "scans/vol-1");
    assert_eq!(freeze(&resumed)["status"], "done");
    assert_eq!(freeze(&resumed)["packs"], 1);
    assert_eq!(freeze(&resumed)["entries"], BOOK.len());
    assert_eq!(
        resumed["sync"],
        serde_json::Value::Null,
        "nothing but the freeze carried the pages in",
    );
    packed(&served, "scans/vol-1").await;
    assert_eq!(
        served.pending_rows().await,
        0,
        "the dead run's row was settled, and the resumed run's went with its commit",
    );
    assert_eq!(served.spools_left(), 0, "and no spool is left behind");
    assert_eq!(
        served.reservations().await,
        0,
        "the commit released the book"
    );
}

/// A sync that runs before the book is taken up — one an unrelated drop arms,
/// say — carries in what was dropped beside the book and leaves the book's
/// pages alone, saying each waits for its Pack; and the listing marks them as
/// waiting rather than as files the next run takes in (spec: PK-17).
#[tokio::test]
async fn a_sync_before_the_book_is_taken_up_leaves_its_pages_alone() {
    let mut served = Served::library().await;
    served.halt_storage();
    let (status, _) = body_of(served.upload_book("scans/vol-1", &BOOK).await).await;
    assert_eq!(status, 200);
    served.freeze_idle().await;

    served.restart();
    served.resume_storage();
    let (status, _) = body_of(
        served
            .upload("albums", &[("loose.jpg", b"a photo on its own")])
            .await,
    )
    .await;
    assert_eq!(status, 200);
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(sync(&work)["status"], "done");
    let waiting: Vec<_> = sync(&work)["findings"]
        .as_array()
        .expect("a sync carries findings")
        .iter()
        .filter(|finding| finding["surfaced"] == "AwaitsPack")
        .map(|finding| finding["path"].as_str().expect("a path").to_owned())
        .collect();
    assert_eq!(
        waiting,
        BOOK.map(|(name, _)| format!("scans/vol-1/{name}")),
        "every page is said to wait for its Pack: {work}",
    );
    let listing = listing_of(&served, "scans/vol-1").await;
    for row in listing["files"]
        .as_array()
        .expect("a listing carries files")
    {
        assert_eq!(row["state"], "added", "no page was carried in: {row}");
        assert_eq!(
            row["reserved"], true,
            "and each is marked as waiting: {row}"
        );
    }
    let albums = listing_of(&served, "albums").await;
    assert!(
        albums["files"]
            .as_array()
            .expect("a listing carries files")
            .iter()
            .any(|row| row["name"] == "loose.jpg"
                && row["state"] == "present"
                && row["reserved"] == false),
        "the photo dropped on its own was carried in as it always is: {albums}",
    );

    served.start_up().await;
    served.freeze_idle().await;
    packed(&served, "scans/vol-1").await;
    let listing = listing_of(&served, "scans/vol-1").await;
    assert!(
        listing["files"]
            .as_array()
            .expect("a listing carries files")
            .iter()
            .all(|row| row["reserved"] == false),
        "a packed page waits for nothing: {listing}",
    );
}

/// Two books waiting for their Pack are taken up in the order they were
/// dropped, each as a run of its own (spec: PK-17).
#[tokio::test]
async fn two_waiting_books_are_taken_up_in_the_order_they_were_dropped() {
    let mut served = Served::library().await;
    served.halt_storage();
    for folder in ["scans/vol-2", "scans/vol-1"] {
        let (status, _) = body_of(served.upload_book(folder, &BOOK).await).await;
        assert_eq!(status, 200);
        served.freeze_idle().await;
    }

    served.restart();
    served.resume_storage();
    served.start_up().await;
    let (_, armed) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        freeze(&armed)["folder"],
        "scans/vol-2",
        "the book dropped first is the one taken up first: {armed}",
    );
    assert_eq!(freeze(&armed)["waiting"], json!(["scans/vol-1"]));

    served.freeze_idle().await;
    let (_, done) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&done)["folder"], "scans/vol-1");
    assert_eq!(freeze(&done)["status"], "done");
    assert_eq!(freeze(&done)["entries"], BOOK.len());
    packed(&served, "scans/vol-1").await;
    packed(&served, "scans/vol-2").await;
    assert_eq!(served.reservations().await, 0);
}

/// A book whose pages were all deleted while no server was running is
/// released with a finding per page saying so, and nothing is committed
/// (spec: PK-17).
#[tokio::test]
async fn a_book_whose_pages_are_gone_is_released_with_a_finding() {
    let mut served = Served::library().await;
    served.halt_storage();
    let (status, _) = body_of(served.upload_book("scans/vol-1", &BOOK).await).await;
    assert_eq!(status, 200);
    served.freeze_idle().await;
    for (name, _) in BOOK {
        std::fs::remove_file(served.local_path(&format!("scans/vol-1/{name}")))
            .expect("a landed page can be deleted");
    }

    served.restart();
    served.resume_storage();
    served.start_up().await;
    served.freeze_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&work)["status"], "done");
    assert_eq!(freeze(&work)["packs"], 0);
    let gone: Vec<_> = freeze(&work)["findings"]
        .as_array()
        .expect("a freeze carries findings")
        .iter()
        .filter(|finding| finding["surfaced"] == "Unreserved")
        .map(|finding| finding["path"].as_str().expect("a path").to_owned())
        .collect();
    assert_eq!(gone, BOOK.map(|(name, _)| format!("scans/vol-1/{name}")));
    assert_eq!(served.reservations().await, 0);
}

/// A drop that asked for a Pack and landed nothing — every part of it refused
/// by name — leaves no reservation behind: nothing was armed for it, so
/// nothing waits for a Pack either (spec: PK-17).
#[tokio::test]
async fn a_book_drop_that_lands_nothing_leaves_no_reservation() {
    let served = Served::library().await;
    let (status, answer) = body_of(
        served
            .upload_book("scans/vol-1", &[(".coffret/page-001.jpg", b"a page")])
            .await,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer), Vec::<String>::new());
    assert_eq!(served.reservations().await, 0);
}

/// A lock that lands before a book's freeze takes the keys stops it, and the
/// reservation it leaves is taken up again once the Library is unlocked in
/// place (spec: PK-17, DK-2).
#[tokio::test(start_paused = true)]
async fn a_waiting_book_is_taken_up_when_the_library_is_unlocked_in_place() {
    const QUIET: std::time::Duration = std::time::Duration::from_secs(15 * 60);
    let served = Served::in_the_app().await;
    served.halt_storage();
    let (status, _) = body_of(served.upload_book("scans/vol-1", &BOOK).await).await;
    assert_eq!(status, 200);
    served.freeze_idle().await;
    served.resume_storage();

    served.watch_idle(QUIET).await;
    tokio::time::advance(QUIET + std::time::Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    let (status, _) = crate::support::route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 423, "locked by the clock");

    assert_eq!(served.unlock(), coffret_server::Unlocked::Now);
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    served.freeze_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&work)["status"], "done");
    assert_eq!(freeze(&work)["entries"], BOOK.len());
    packed(&served, "scans/vol-1").await;
    assert_eq!(served.reservations().await, 0);
}

/// A freeze of the whole folder that runs while a drop into it is still
/// receiving packs the pages landed so far and leaves the drop's reservation
/// alone; once the drop stops receiving, its own run packs the pages that
/// landed after and releases it (spec: PK-17). Released by the earlier run, the
/// later pages would land unreserved, for a sync after a restart to carry in
/// one Container each.
#[tokio::test]
async fn a_freeze_beside_a_drop_still_landing_leaves_its_reservation_to_the_drop() {
    let served = Served::library().await;
    let drop = served.upload_book_streamed("scans/vol-1");
    for (name, content) in &BOOK[..2] {
        served.until_landed(&drop.send(name, content)).await;
    }

    served.arm_freeze("scans/vol-1");
    served.freeze_idle().await;
    let (_, beside) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&beside)["status"], "done");
    assert_eq!(
        freeze(&beside)["entries"],
        2,
        "the pages landed so far are packed"
    );
    assert_eq!(
        served.reservations().await,
        1,
        "and the drop under way keeps its reservation",
    );

    let (name, content) = BOOK[2];
    served.until_landed(&drop.send(name, content)).await;
    let (status, answer) = body_of(drop.finish().await).await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer).len(), BOOK.len());
    served.freeze_idle().await;

    let (_, own) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&own)["status"], "done");
    assert_eq!(
        freeze(&own)["entries"],
        1,
        "the drop's own run packs the page that landed after",
    );
    packed(&served, "scans/vol-1").await;
    assert_eq!(
        served.reservations().await,
        0,
        "and releases the reservation"
    );
}

/// The run a drop arms once it stops receiving is a run of its own, queued
/// behind a run of its folder already under way rather than taken for it
/// (spec: PK-17): that run read the reservations while the drop was still
/// landing, so it may have walked past pages the drop wrote after.
#[tokio::test]
async fn a_drop_does_not_join_the_freeze_under_way() {
    let served = Served::library().await;
    let drop = served.upload_book_streamed("scans/vol-1");
    for (name, content) in BOOK {
        served.until_landed(&drop.send(name, content)).await;
    }

    served.hold_storage();
    served.arm_freeze("scans/vol-1");
    for _ in 0..2_000 {
        if served.held_reads() > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(served.held_reads() > 0, "the folder's run is under way");
    let (status, _) = body_of(drop.finish().await).await;
    assert_eq!(status, 200);
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&work)["folder"], "scans/vol-1");
    assert_eq!(freeze(&work)["status"], "freezing");
    assert_eq!(
        freeze(&work)["waiting"],
        json!(["scans/vol-1"]),
        "the drop's book waits its turn behind the run under way: {work}",
    );

    served.release_storage();
    served.freeze_idle().await;
    packed(&served, "scans/vol-1").await;
    assert_eq!(served.reservations().await, 0);
}

/// A drop that was still receiving when its server stopped holds its
/// reservation live in that server alone: the next one holds nothing live,
/// and takes the reservation up as it starts, packing the pages that landed
/// (spec: PK-17, LA-12).
#[tokio::test]
async fn a_reservation_live_when_its_server_stopped_is_taken_up_by_the_next() {
    let mut served = Served::library().await;
    let drop = served.upload_book_streamed("scans/vol-1");
    for (name, content) in &BOOK[..2] {
        served.until_landed(&drop.send(name, content)).await;
    }
    drop.abandon();

    served.restart();
    served.start_up().await;
    served.freeze_idle().await;
    let (_, resumed) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&resumed)["status"], "done");
    assert_eq!(freeze(&resumed)["entries"], 2, "the pages that landed");
    assert_eq!(served.reservations().await, 0);
    assert_eq!(
        rows_of(&served, "scans/vol-1").await,
        BOOK[..2]
            .iter()
            .map(|(name, _)| ((*name).to_owned(), "present".to_owned(), "pack".to_owned()))
            .collect::<Vec<_>>(),
    );
}
