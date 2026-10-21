//! A drop the shell hands over by path, walked and held for the explorer's
//! import (spec: LA-26).

use super::*;

// LA-26: nothing is walked while no Library is open, and once one is, a drop
// is answered with a handle and its files named relative to the folder they
// were dropped from — never by a path on this device.
#[test]
fn a_drop_is_held_once_a_library_is_open() {
    let case = Case::new();
    let source = tempfile::tempdir().expect("a temporary directory must be available");
    let page = source.path().join("book/page-001.jpg");
    std::fs::create_dir_all(page.parent().unwrap()).unwrap();
    std::fs::write(&page, b"a page").unwrap();
    let dropped = vec![source.path().join("book").to_str().unwrap().to_owned()];

    assert!(
        case.desktop.take_drop(&dropped).is_err(),
        "no Library is open to take a drop into"
    );

    let token = case.token(DoorMode::Open);
    case.run(async {
        create("drops").await;
        let answered = case
            .send(case.submission(&token, opening("drops", PASSPHRASE)))
            .await;
        assert_eq!(answered.status, StatusCode::OK, "{}", answered.text);
    });

    let held = case.desktop.take_drop(&dropped).expect("the drop is held");
    assert!(held.handle.is_some());
    assert_eq!(held.files.len(), 1);
    assert_eq!(held.files[0].path, "book/page-001.jpg");
    assert_eq!(held.files[0].size, 6);
    assert!(
        !format!("{held:?}").contains(source.path().to_str().unwrap()),
        "the answer names no path on this device"
    );
    case.desktop.retire();
}
