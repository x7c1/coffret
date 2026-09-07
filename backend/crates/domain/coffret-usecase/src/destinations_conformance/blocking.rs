use coffret_model::Mtime;

use crate::descent_error::DescentError;
use crate::destinations_conformance::components;
use crate::destinations_conformance::destinations_under_test::DestinationsUnderTest;

/// A component that is a symbolic link refuses the whole place, and the refusal
/// names it.
///
/// This is the case the confinement exists for. An Entry Path comes from
/// whichever device committed it and says nothing about this one:
/// `link/authorized_keys` is an ordinary folder and a file over there, and here
/// `link` happens to be a link to a folder of the person's own. A capability
/// that resolved the joined path would place bytes where the mappings never
/// pointed and could replace a file the Library has never held, so EP-4 refuses
/// the path rather than inventing a place for it (spec: EP-4, EP-11).
///
/// The component travels with the refusal because it is what a person acts on:
/// the Entry Path says which file went unplaced, and this says which name to go
/// and look at.
///
/// Unix-only in practice, and the reason a backend's test target says so: what
/// "neither a file nor a folder" means on a real filesystem is a symbolic link.
/// The fake has a planted marker instead, which is why the case itself is
/// portable and the gateway's target is not.
pub async fn a_symlink_on_the_way_blocks_the_reach_and_names_it(fixture: &DestinationsUnderTest) {
    let blocked = fixture.root().join("link");
    fixture.arrange().plant_other(&blocked);

    let refused = fixture
        .destinations()
        .reach(fixture.root(), &components(&["link", "authorized_keys"]))
        .await
        .err()
        .expect("nothing may be reached through a symbolic link");

    match refused {
        DescentError::Blocked { path } => assert_eq!(
            path, blocked,
            "the component the walk stopped at is what there is to look at",
        ),
        other => panic!("a link on the way is a blocked place, and the capability said {other:?}"),
    }
}

/// A component that is an ordinary file rather than a folder is refused the same
/// way.
///
/// The other half of what each step of the descent asks for, and it answers a
/// question EP-4 already has a verdict for: no file on this device can stand for
/// the Entry Path, because a name on the way to it is occupied by something that
/// is not a folder. Reported as that rather than as whatever the operating system
/// said about making a directory — the reading of an errno is the gateway's, and
/// this is the case that pins it.
pub async fn a_file_where_a_folder_must_be_blocks_the_reach(fixture: &DestinationsUnderTest) {
    let blocked = fixture.root().join("albums");
    fixture.arrange().write_file(
        &blocked,
        b"a file of the person's own, where a folder would go",
        Mtime::from_unix_seconds(1),
    );

    let refused = fixture
        .destinations()
        .reach(fixture.root(), &components(&["albums", "spring.jpg"]))
        .await
        .err()
        .expect("a name that is not a folder is not one to make a folder of");

    match refused {
        DescentError::Blocked { path } => assert_eq!(path, blocked),
        other => panic!(
            "a file where a folder must be is a blocked place, and the capability said {other:?}"
        ),
    }
}
