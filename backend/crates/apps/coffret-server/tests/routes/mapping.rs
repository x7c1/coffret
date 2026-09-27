//! What this device's mappings make of the routes: which folders a listing
//! says it has one for, a fetch no folder of it holds, and a mapped root whose
//! marker it refuses (spec: EP-9, EP-11, EP-13).

use serde_json::Value;

use crate::support::{files, fill, folders, folders_mapped, json as body_of, Served};

// EP-9: a mapping is what makes a local path exist at all, so an Entry outside
// every one of them is a fact about this device — which the explorer shows as
// "this folder is not on this device".
#[tokio::test]
async fn an_entry_no_folder_on_this_device_holds_is_declined() {
    let served = Served::mapping_only("albums").await;

    let (status, refusal) = body_of(served.get("/api/file?path=books/page-001.png").await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "declined");
    assert_eq!(refusal["reason"], "unmapped");

    // The Library still holds it, and the catalog still lists it: what is
    // missing is a folder here to put it in.
    let (status, listing) = body_of(served.get("/api/list?path=books").await).await;
    assert_eq!(status, 200);
    assert_eq!(folders(&listing), Vec::<String>::new());
    assert_eq!(files(&listing).len(), 1);
    assert_eq!(
        listing["mapped"], false,
        "the listing said so before anything was clicked",
    );
}

// EP-9: the mappings answer "is this folder on this device" out of the catalog,
// so the listing carries it and a browser never has to find out by being
// declined. The children of the Library root are where two siblings differ,
// since a mapping is made at the top level.
#[tokio::test]
async fn a_listing_says_which_folders_this_device_has_one_for() {
    let served = Served::mapping_only("albums").await;

    let (status, root) = body_of(served.get("/api/list").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        root["mapped"], false,
        "a top-level mapping stands for its own subtree and not for what sits beside it",
    );
    assert_eq!(
        folders_mapped(&root),
        [("albums".to_owned(), true), ("books".to_owned(), false)],
    );

    let (_, albums) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(albums["mapped"], true);
    assert_eq!(folders_mapped(&albums), [("2026".to_owned(), true)]);
}

// A mapping at the Library root represents everything the top-level ones do
// not, so with one present every folder is on this device (spec: EP-9).
#[tokio::test]
async fn a_device_that_maps_the_library_root_has_a_folder_for_everything() {
    let served = Served::library().await;

    for uri in ["/api/list", "/api/list?path=albums", "/api/list?path=books"] {
        let (status, listing) = body_of(served.get(uri).await).await;
        assert_eq!(status, 200, "{uri}");
        assert_eq!(listing["mapped"], true, "{uri}");
        assert!(
            folders_mapped(&listing).iter().all(|(_, mapped)| *mapped),
            "{uri}: {listing}",
        );
    }
}

// EP-11: a fetch places a file only where this device can vouch for what is
// there, and a file it did not put there may be content the Library has never
// held. Left byte-for-byte as it is, and the browser told what was found.
#[tokio::test]
async fn a_file_this_device_did_not_place_is_never_overwritten() {
    let served = Served::library().await;
    served.plant_locally("albums/cover.png", b"something of my own");

    let (status, refusal) = body_of(served.get("/api/file?path=albums/cover.png").await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "declined");
    assert_eq!(refusal["reason"], "surfaced");
    assert_eq!(refusal["surfaced"], "ForeignFile");
    assert_eq!(
        std::fs::read(served.local_path("albums/cover.png")).expect("the file is still there"),
        b"something of my own",
    );
}

// EP-13: a mapped folder whose marker names another identity is not the folder
// the mapping was recorded against — a copied disk, a mount that came back
// different — and nothing is placed into it. What the browser is told is the
// point of this case: a refusal of its own, whose sentence names the one gesture
// that remedies it, rather than the `500` that says only that the server could
// not answer and leaves a person with nothing to go on.
//
// The same refusal reaches the fill, which stops on it: every Entry under that
// mapping meets it identically, so asking for the next file would be asking the
// broken question again.
#[tokio::test]
async fn a_refused_root_reaches_the_browser_as_a_refused_placement() {
    let served = Served::library().await;

    // The folder is now somebody else's copy of the one that was registered: the
    // marker is a marker, and it names another identity.
    std::fs::write(served.local_path(".coffret/root"), "0011223344556677\n")
        .expect("the mapped root's marker can be rewritten");

    let (status, refusal) = body_of(served.get("/api/file?path=albums/notes.txt").await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "refused_placement");
    assert_eq!(refusal["reason"], "refused_root");
    assert_eq!(refusal["surfaced"], Value::Null);
    let message = refusal["message"]
        .as_str()
        .expect("a refusal says something")
        .to_owned();
    assert!(
        message.contains("coffret map"),
        "the sentence names the gesture that remedies it: {message}",
    );
    assert!(
        !served.holds("albums/notes.txt"),
        "nothing was placed into a folder that will not vouch for itself",
    );

    // Armed by hand, because the route that would have armed it is the one that
    // was refused: a fetch that placed nothing starts nothing.
    assert_eq!(served.post("/api/fill?path=albums").await.status(), 202);
    served.fill_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    let stopped = fill(&work);
    assert_eq!(
        stopped["status"], "stopped",
        "every Entry under the mapping meets the same refusal: {work}",
    );
    assert_eq!(stopped["stopped"]["error"], "refused_placement");
    assert_eq!(stopped["stopped"]["reason"], "refused_root");
    assert_eq!(stopped["stopped"]["message"], message);
}

// EP-13: the sentence that refusal is put in front of a person as names *which*
// mapping will not vouch for itself, because `coffret map` has to be aimed at
// one of them and a device has as many mappings as its owner gave it. What names
// it is the Library-side prefix — a name inside the Library, which is the
// person's own — and never the folder on this device, which is a local path this
// boundary does not carry (spec: EL-1).
//
// Both mappings EP-9 admits are covered, because the sentence differs between
// them: one standing for a top-level component, and one standing for the Library
// root, where there is no component to name.
#[tokio::test]
async fn a_refused_root_names_the_mapping_in_the_sentence() {
    for (served, named) in [
        (Served::mapping_only("albums").await, "\"albums\""),
        (Served::library().await, "the Library root"),
    ] {
        // The folder in front of the device is somebody else's copy of the one
        // that was registered, exactly as in the case above: the marker is a
        // marker, and it names another identity.
        std::fs::write(served.local_path(".coffret/root"), "0011223344556677\n")
            .expect("the mapped root's marker can be rewritten");

        let (status, refusal) = body_of(served.get("/api/file?path=albums/notes.txt").await).await;
        assert_eq!(status, 409);
        assert_eq!(refusal["reason"], "refused_root");
        let message = refusal["message"]
            .as_str()
            .expect("a refusal says something")
            .to_owned();
        assert!(
            message.contains(named),
            "the sentence names the mapping that would not vouch for itself: {message}",
        );

        let area = served.local_path(".coffret");
        let folder = area
            .parent()
            .expect("the management area stands inside the mapped folder")
            .to_string_lossy()
            .into_owned();
        assert!(
            !message.contains(folder.as_str()),
            "and never the folder the mapping names, which is a local path: {message}",
        );
    }
}
