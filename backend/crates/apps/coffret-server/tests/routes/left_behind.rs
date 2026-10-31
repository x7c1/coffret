//! What a server that is gone left in the mapped folder, as the next one
//! opens the Library (spec: OC-8, LA-36).

use coffret_local_fs::UnixFs;
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::{scratch, Aside, Destinations};

use crate::support::Served;

/// The identity the fixture's mapped root carries (see `register_root`).
const ROOT_ID: RootMarkerId = RootMarkerId::from_bytes([0x2a; RootMarkerId::BYTE_LEN]);

// OC-8, LA-36: an add that writes over a file keeps that file aside under a
// scratch name until it ends, and a server that died before it ended leaves
// the link behind. The next server removes it as it opens the Library, before
// it serves anything, and puts nothing back: the file at the name is what it
// was. One a live add still holds — here, by its lock, as an add in another
// process holds it — is left where it is.
#[tokio::test]
async fn a_kept_file_a_dead_add_left_goes_as_the_library_opens_and_a_held_one_stays() {
    let mut served = Served::library().await;
    served.plant_locally("albums/over.jpg", b"what the add wrote");
    served.plant_locally("albums/held.jpg", b"the person's own");

    // What a server that died mid-add left: a second link to the file the add
    // wrote over, under a scratch name nothing holds any more.
    let left = scratch::incoming_name().as_str().to_owned();
    std::fs::hard_link(
        served.local_path("albums/over.jpg"),
        served.local_path(&format!("albums/{left}")),
    )
    .expect("the link is made");

    // And what a live add holds: a file kept aside, its lock held for as long
    // as it is kept.
    let fs = UnixFs::new();
    let destination = fs
        .reach(
            served.local_root(),
            Some(&ROOT_ID),
            &["albums".to_owned(), "held.jpg".to_owned()],
        )
        .await
        .expect("the destination is reachable");
    let kept_name = scratch::incoming_name().as_str().to_owned();
    let Aside::Kept(kept) = destination
        .keep_aside(&kept_name)
        .expect("the file is kept aside")
    else {
        panic!("a file stands at the name");
    };

    served.restart();
    served.start_up().await;

    let names = served.folder_names("albums");
    assert!(
        !names.contains(&left),
        "what the dead add kept aside is gone: {names:?}",
    );
    assert!(
        names.contains(&kept_name),
        "what a live add holds stays: {names:?}",
    );
    assert_eq!(
        std::fs::read(served.local_path("albums/over.jpg")).expect("the file is there"),
        b"what the add wrote",
        "nothing is put back",
    );
    assert_eq!(
        std::fs::read(served.local_path(&format!("albums/{kept_name}")))
            .expect("the kept file is there"),
        b"the person's own",
    );

    kept.remove().expect("letting it go succeeds");
}
