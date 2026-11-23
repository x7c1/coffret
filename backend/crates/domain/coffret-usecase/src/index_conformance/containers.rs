use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerKind};

use crate::index_conformance::fixtures::{addition, container_id, record};
use crate::index_conformance::index_under_test::IndexUnderTest;

/// The Containers an operation names are looked up by ID, and the current
/// ones are the whole of what is read off the catalog's Containers.
///
/// A commit asks whether the Containers it removes are still current
/// (spec: CP-18), a deletion reads the summaries of the ones it touches
/// (spec: PK-9), and the next Keyring generation maps every current one
/// (spec: KL-7). Each answer is the part of
/// [`containers_under(None)`](crate::Index::containers_under) it asks after —
/// every current Container holds at least one Entry (spec: FM-10) — so a
/// Container a record removed is in none of them, an ID the catalog never held
/// is left out rather than refused, and the order is the Container ID order
/// the whole set is read in.
pub async fn the_containers_named_are_looked_up_by_id(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![
                addition(3, ContainerKind::OneFile, &["notes.txt"]),
                addition(1, ContainerKind::Pack, &["albums/a.jpg", "albums/b.jpg"]),
                addition(2, ContainerKind::Pack, &["books/page-001.png"]),
            ],
            vec![],
        ))
        .await
        .expect("replaying a record must succeed");
    index
        .apply(record(1, vec![], vec![container_id(2)]))
        .await
        .expect("replaying a removal must succeed");

    let all = index
        .containers_under(None)
        .await
        .expect("reading the Library's Containers must succeed");
    assert_eq!(
        all.iter().map(|container| container.id).collect::<Vec<_>>(),
        [container_id(1), container_id(3)],
    );

    let current = index
        .containers()
        .await
        .expect("reading the current Containers must succeed");
    assert_eq!(current, all, "the current Containers are the whole set");

    let named: BTreeSet<ContainerId> = [
        container_id(3),
        container_id(2),
        container_id(1),
        container_id(9),
    ]
    .into();
    let among = index
        .containers_among(&named)
        .await
        .expect("looking the named Containers up must succeed");
    assert_eq!(
        among, all,
        "a removed Container and one never held are left out, in Container ID order",
    );

    let one = index
        .containers_among(&[container_id(3)].into())
        .await
        .expect("looking one Container up must succeed");
    assert_eq!(
        one.iter()
            .map(|container| (container.id, container.kind))
            .collect::<Vec<_>>(),
        [(container_id(3), ContainerKind::OneFile)],
        "one Container is answered with its own summary",
    );

    assert!(
        index
            .containers_among(&BTreeSet::new())
            .await
            .expect("looking no Container up must succeed")
            .is_empty(),
        "naming nothing finds nothing",
    );
}
