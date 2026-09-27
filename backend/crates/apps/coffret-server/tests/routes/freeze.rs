//! The freeze a book drop arms, and the one a folder's button takes up again
//! (spec: PK-17).

use serde_json::json;

use crate::support::{freeze, json as body_of, rows_of, without_server, written, Served};

/// The three pages of the book the freeze cases bring in.
const BOOK: [(&str, &[u8]); 3] = [
    ("page-001.jpg", b"the first page"),
    ("page-002.jpg", b"the second page"),
    ("page-003.jpg", b"the third page"),
];

/// The whole of what a book drop is for. Three pages land in a folder somebody
/// made in the browser, the freeze the drop armed packs them, and what the
/// Library ends up holding is Packs rather than one Container per page
/// (spec: PK-1, PK-7, PK-17).
///
/// And the sync is not armed. That is not a detail: a sync over these files
/// would carry each of them in as a Container of its own, which is the shape the
/// freeze exists to avoid — so a drop that armed both would pack a book that had
/// already been committed one page at a time.
#[tokio::test]
async fn a_book_dropped_into_a_new_folder_is_packed_rather_than_synced() {
    let served = Served::library().await;

    let (status, answer) = body_of(served.upload_book("scans/vol-1", &BOOK).await).await;
    assert_eq!(status, 200);
    assert_eq!(
        written(&answer),
        [
            "scans/vol-1/page-001.jpg",
            "scans/vol-1/page-002.jpg",
            "scans/vol-1/page-003.jpg",
        ],
    );
    assert_eq!(answer["refused"], json!([]));

    served.freeze_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&work)["folder"], "scans/vol-1");
    assert_eq!(freeze(&work)["status"], "done");
    assert_eq!(freeze(&work)["packs"], 1);
    assert_eq!(freeze(&work)["entries"], 3);
    assert_eq!(freeze(&work)["findings"], json!([]));
    assert_eq!(freeze(&work)["stopped"], serde_json::Value::Null);
    assert_eq!(
        work["sync"],
        serde_json::Value::Null,
        "a book drop arms the freeze and nothing else: {work}",
    );

    assert_eq!(
        rows_of(&served, "scans/vol-1").await,
        BOOK.map(|(name, _)| (name.to_owned(), "present".to_owned(), "pack".to_owned())),
        "every page is an Entry this device has, and all of them live in Packs",
    );
    // And the folder is in the Library rather than only on this device: a folder
    // is what the separators of its Entries' paths imply (spec: EP-2), so the
    // tree draws it only once something under it is committed.
    let (_, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(
        listed,
        json!({ "folders": ["albums", "albums/2026", "books", "scans", "scans/vol-1"] }),
    );
}

// EP-9: a folder no mapping of this device reaches has nowhere to put a single
// page, so the whole drop is refused at once — before any byte, and whichever
// gesture it was.
#[tokio::test]
async fn a_book_dropped_where_this_device_has_no_folder_is_refused_whole() {
    let served = Served::mapping_only("albums").await;

    let (status, refusal) = body_of(served.upload_book("books/vol-1", &BOOK).await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "refused_placement");
    assert_eq!(refusal["reason"], "unmapped");

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        without_server(&work),
        json!({
            "library": "unlocked",
            "catalog": { "state": "caught_up", "stopped": null },
            "fill": null,
            "sync": null,
            "freeze": null,
        }),
        "nothing landed, so there is nothing to pack",
    );
}

// PK-10, PK-12: the per-part rules are the drop's, not the sync's, so a book
// drop meets them identically. A page whose name the Library already holds
// inside a Pack is refused by name — coffret cannot replace one yet — and the
// page beside it lands and is packed.
#[tokio::test]
async fn a_page_the_library_holds_inside_a_pack_is_refused_and_the_rest_is_packed() {
    let served = Served::packed_library().await;

    let (status, answer) = body_of(
        served
            .upload_book(
                "books",
                &[("page-001.png", b"mine"), ("page-002.png", b"next")],
            )
            .await,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer), ["books/page-002.png"]);
    assert_eq!(answer["refused"][0]["name"], "page-001.png");
    assert_eq!(answer["refused"][0]["reason"], "pack_resident");
    assert!(
        !served.holds("books/page-001.png"),
        "the refusal was decided before any byte was written",
    );

    served.freeze_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&work)["status"], "done");
    assert_eq!(freeze(&work)["entries"], 1);
    assert_eq!(
        rows_of(&served, "books").await,
        [
            (
                "page-001.png".to_owned(),
                "remote".to_owned(),
                "pack".to_owned()
            ),
            (
                "page-002.png".to_owned(),
                "present".to_owned(),
                "pack".to_owned()
            ),
        ],
        "the page that landed was packed, and the Pack beside it was left alone",
    );
}

// One book at a time. A second folder asked for while one is being packed waits
// its turn rather than taking its place: a freeze commits one batch (spec:
// PK-7), so one abandoned half way brings in none of its book — where a
// fill, which does follow whoever is clicking, leaves behind exactly the files
// it had already brought over.
//
// Two armings with nothing awaited between them, which is the one way to say
// this as a case: anything awaited would let the worker finish the first.
#[tokio::test]
async fn a_second_book_waits_for_the_first_rather_than_taking_its_place() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    served.plant_locally("scans/vol-2/page-001.jpg", b"the second book");

    served.arm_freeze("scans/vol-1");
    served.arm_freeze("scans/vol-2");
    served.freeze_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&work)["folder"], "scans/vol-2");
    assert_eq!(freeze(&work)["status"], "done");
    assert_eq!(
        freeze(&work)["entries"],
        1,
        "the run on record is the second book's own, not one sweep of both",
    );

    for folder in ["scans/vol-1", "scans/vol-2"] {
        assert_eq!(
            rows_of(&served, folder).await,
            [(
                "page-001.jpg".to_owned(),
                "present".to_owned(),
                "pack".to_owned()
            )],
            "{folder} was packed, its run not superseded by the other's",
        );
    }
}

// A Storage that has gone stops a freeze the way it stops a fill and a sync, and
// it is reported the same way: the pages stay in the folder, the Library is
// untouched, and `POST /api/freeze` is the whole of the recovery — no restart
// and no dropping the book again.
#[tokio::test]
async fn storage_stops_a_freeze_and_the_book_can_be_packed_again() {
    let served = Served::library().await;
    served.halt_storage();

    let (status, answer) = body_of(served.upload_book("scans/vol-1", &BOOK).await).await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer).len(), 3);
    served.freeze_idle().await;

    let (_, stopped) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&stopped)["folder"], "scans/vol-1");
    assert_eq!(freeze(&stopped)["status"], "stopped");
    assert_eq!(freeze(&stopped)["stopped"]["error"], "storage");
    assert_eq!(freeze(&stopped)["packs"], 0);
    assert!(
        rows_of(&served, "scans/vol-1")
            .await
            .iter()
            .all(|(_, state, container)| state == "added" && container.is_empty()),
        "the pages are in the folder and the Library holds none of them",
    );

    served.resume_storage();
    let (status, armed) = body_of(served.post("/api/freeze?path=scans/vol-1").await).await;
    assert_eq!(status, 202);
    assert_eq!(
        freeze(&armed)["status"],
        "freezing",
        "the failure it is retrying is off the screen the moment the retry is armed",
    );

    served.freeze_idle().await;
    let (_, finished) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&finished)["status"], "done");
    assert_eq!(freeze(&finished)["entries"], 3);
    assert_eq!(freeze(&finished)["stopped"], serde_json::Value::Null);
    assert_eq!(
        rows_of(&served, "scans/vol-1").await,
        BOOK.map(|(name, _)| (name.to_owned(), "present".to_owned(), "pack".to_owned())),
    );
}

// The same for the books, and the case that made it a daily path: a browser
// queues a second book whenever somebody drops two in a session, and a freeze
// Storage stopped leaves its pages in a folder the browser made and the Library
// has never heard of. If the next book took that run off the wire, the folder
// would be named nowhere — a page that came back would draw no row for it, offer
// no way to walk in, and make no second attempt at it.
#[tokio::test]
async fn a_freeze_storage_stopped_is_still_named_once_the_next_book_runs() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    served.plant_locally("scans/vol-2/page-001.jpg", b"the second book");
    served.halt_storage();

    served.arm_freeze("scans/vol-1");
    served.arm_freeze("scans/vol-2");
    served.freeze_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    let latest = freeze(&work);
    assert_eq!(latest["folder"], "scans/vol-2");
    assert_eq!(latest["status"], "stopped");

    let displaced = latest["displaced"].as_array().expect("an array");
    assert_eq!(displaced.len(), 1, "the book the record was taken from");
    assert_eq!(displaced[0]["folder"], "scans/vol-1");
    assert_eq!(displaced[0]["status"], "stopped");
    assert_eq!(displaced[0]["stopped"]["error"], "storage");
    assert_eq!(displaced[0]["run"], 1);
    for list in ["waiting", "discarded", "displaced"] {
        assert!(
            displaced[0].get(list).is_none(),
            "the queue is the flow's, so a displaced book carries no `{list}`",
        );
    }
    assert_eq!(
        latest["discarded"].as_array().map(Vec::len),
        Some(0),
        "and not among the books that were thrown away before anything started \
         on them, which is a different thing and says so",
    );

    // Packing it again is what ends its notice, and the pages are still there to
    // pack: the folder was never the Library's, and nothing was committed. The
    // second book, which stopped in its turn, keeps its own line — it is still a
    // folder of pages outside the Library, and the one on record now says
    // nothing about it.
    served.resume_storage();
    assert_eq!(
        served.post("/api/freeze?path=scans/vol-1").await.status(),
        202
    );
    served.freeze_idle().await;

    let (_, finished) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&finished)["folder"], "scans/vol-1");
    assert_eq!(freeze(&finished)["status"], "done");
    assert_eq!(
        freeze(&finished)["displaced"]
            .as_array()
            .expect("an array")
            .iter()
            .map(|run| run["folder"].clone())
            .collect::<Vec<_>>(),
        ["scans/vol-2"],
        "the book pressed is off the list, and the one it took the record from is on it",
    );
    assert_eq!(
        rows_of(&served, "scans/vol-1").await,
        [(
            "page-001.jpg".to_owned(),
            "present".to_owned(),
            "pack".to_owned()
        )],
    );
}

// EP-9: a freeze of a folder no mapping reaches would walk to select nothing and
// commit nothing, so it is refused rather than armed — a `202` for work that
// cannot happen is a browser told to follow a run that will never say anything.
#[tokio::test]
async fn a_freeze_of_a_folder_no_mapping_of_this_device_reaches_is_refused() {
    let served = Served::mapping_only("albums").await;

    let (status, refusal) = body_of(served.post("/api/freeze?path=books").await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "refused_placement");
    assert_eq!(refusal["reason"], "unmapped");

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["freeze"], serde_json::Value::Null);
}

// EP-2: the folder a freeze is asked for is held to the same shape every other
// path on these routes is, and refused before anything is armed.
#[tokio::test]
async fn a_freeze_of_something_that_is_not_a_folder_is_refused() {
    let served = Served::library().await;

    let (status, refusal) = body_of(served.post("/api/freeze?path=albums/../etc").await).await;
    assert_eq!(status, 400);
    assert_eq!(refusal["error"], "bad_path");

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["freeze"], serde_json::Value::Null);
}

// PK-17: a freeze is of one folder, and a prefix narrowed to nothing selects
// every eligible Entry the mappings reach. An absent `?path=` is the Library
// root everywhere else on these routes, so leaving it out here would be the
// command line's whole-Library run arrived at by omission — and this fixture
// maps the Library root, which is the device on which nothing else stands
// between the two.
#[tokio::test]
async fn a_freeze_that_names_no_folder_is_refused() {
    let served = Served::library().await;

    let (status, refusal) = body_of(served.post("/api/freeze").await).await;
    assert_eq!(status, 400);
    assert_eq!(refusal["error"], "bad_path");

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["freeze"], serde_json::Value::Null);
}

// The same rule reached through the drop, which is how a browser reaches this at
// all: a book is brought into the folder made for it, so `freeze=true` naming no
// folder is refused before a byte is read rather than packing the whole Library
// around the pages that were dropped.
#[tokio::test]
async fn a_book_dropped_onto_the_library_root_is_refused_whole() {
    let served = Served::library().await;

    let (status, refusal) = body_of(served.upload_book("", &BOOK).await).await;
    assert_eq!(status, 400);
    assert_eq!(refusal["error"], "bad_path");
    assert!(
        !served.holds("page-001.jpg"),
        "the refusal was decided before any byte was written",
    );

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        without_server(&work),
        json!({
            "library": "unlocked",
            "catalog": { "state": "caught_up", "stopped": null },
            "fill": null,
            "sync": null,
            "freeze": null,
        }),
        "nothing landed, and nothing was armed",
    );
}
