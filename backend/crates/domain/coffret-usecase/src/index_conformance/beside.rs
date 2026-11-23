use coffret_model::ContainerKind;

use crate::folder_figures::Side;
use crate::index_conformance::fixtures::{addition, container_id, path, record};
use crate::index_conformance::index_under_test::IndexUnderTest;

/// The nearest folder and the nearest Entry standing directly in a folder on
/// either side of a bound are looked up in EP-3 order, one child at a time
/// (spec: EP-2, EP-3, LA-32).
///
/// What a reader stepping back or reading on asks of each folder it passes,
/// so the answers have to be [`folder_contents`](crate::Index::folder_contents)'
/// own children, nearest first: a folder or an Entry below the folder's
/// children is neither, the bound is on neither side whether or not anything
/// stands there, and a folder whose last Entry a record removed is gone.
pub async fn the_children_beside_a_bound_are_looked_up_in_order(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![
                addition(
                    1,
                    ContainerKind::Pack,
                    &["a/1.jpg", "b/c/1.jpg", "b/1.jpg", "b/2.jpg", "b"],
                ),
                addition(2, ContainerKind::OneFile, &["bb/1.jpg"]),
                addition(3, ContainerKind::OneFile, &["b.jpg"]),
            ],
            vec![],
        ))
        .await
        .expect("replaying a record must succeed");

    let folder = |folder: Option<&'static str>, bound: Option<&'static str>, side: Side| {
        let folder = folder.map(path);
        let bound = bound.map(path);
        async move {
            index
                .folder_beside(folder.as_ref(), bound.as_ref(), side)
                .await
                .expect("reading the nearest folder must succeed")
                .map(|found| found.as_str().to_owned())
        }
    };
    let entry = |folder: Option<&'static str>, bound: Option<&'static str>, side: Side| {
        let folder = folder.map(path);
        let bound = bound.map(path);
        async move {
            index
                .entry_beside(folder.as_ref(), bound.as_ref(), side)
                .await
                .expect("reading the nearest Entry must succeed")
                .map(|found| found.as_str().to_owned())
        }
    };
    let some = |text: &str| Some(text.to_owned());

    assert_eq!(folder(None, None, Side::After).await, some("a"));
    assert_eq!(folder(None, None, Side::Before).await, some("bb"));
    assert_eq!(folder(None, Some("b"), Side::After).await, some("bb"));
    assert_eq!(folder(None, Some("b"), Side::Before).await, some("a"));
    assert_eq!(
        folder(None, Some("ba"), Side::Before).await,
        some("b"),
        "a bound nothing stands at names a place all the same",
    );
    assert_eq!(folder(None, Some("bb"), Side::After).await, None);
    assert_eq!(folder(None, Some("a"), Side::Before).await, None);
    assert_eq!(folder(Some("b"), None, Side::After).await, some("b/c"));
    assert_eq!(folder(Some("b/c"), None, Side::Before).await, None);
    assert_eq!(folder(Some("nowhere"), None, Side::After).await, None);

    assert_eq!(
        entry(None, None, Side::After).await,
        some("b"),
        "the Entry at a folder's path stands in the root, beside the folder",
    );
    assert_eq!(entry(None, None, Side::Before).await, some("b.jpg"));
    assert_eq!(entry(None, Some("b"), Side::After).await, some("b.jpg"));
    assert_eq!(entry(None, Some("b"), Side::Before).await, None);
    assert_eq!(
        entry(Some("b"), None, Side::After).await,
        some("b/1.jpg"),
        "an Entry in a child folder is not one of the folder's own",
    );
    assert_eq!(entry(Some("b"), None, Side::Before).await, some("b/2.jpg"));
    assert_eq!(
        entry(Some("b"), Some("b/1.jpg"), Side::After).await,
        some("b/2.jpg")
    );
    assert_eq!(entry(Some("b"), Some("b/1.jpg"), Side::Before).await, None);
    assert_eq!(
        entry(Some("b"), Some("b/15.jpg"), Side::Before).await,
        some("b/1.jpg"),
    );
    assert_eq!(
        entry(Some("b/c"), Some("b/c/1.jpg"), Side::After).await,
        None
    );

    index
        .apply(record(1, vec![], vec![container_id(2)]))
        .await
        .expect("replaying a removal must succeed");
    assert_eq!(
        folder(None, Some("b"), Side::After).await,
        None,
        "a folder whose last Entry was removed is gone",
    );
    assert_eq!(folder(None, None, Side::Before).await, some("b"));
}
