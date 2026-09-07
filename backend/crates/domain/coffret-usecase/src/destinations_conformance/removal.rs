use crate::destinations_conformance::components;
use crate::destinations_conformance::destinations_under_test::DestinationsUnderTest;

/// Removing a temporary file that is already gone is success (spec: OC-6,
/// EP-11).
///
/// The rule the whole of a placement's cleanup rests on. A run that failed
/// mid-write cleans up after itself, a run that failed at the rename cleans up
/// the file the rename did not move, and either cleanup may itself be
/// interrupted — so disposal has to treat absence as the outcome it wanted. A
/// capability that refused instead would replace the verdict a caller is about
/// to report with "and the temporary file would not go either".
pub async fn a_removal_of_a_name_that_is_already_gone_succeeds(fixture: &DestinationsUnderTest) {
    let destination = fixture
        .destinations()
        .reach(fixture.root(), &components(&["spring.jpg"]))
        .await
        .expect("reaching a place directly under the root must succeed");

    destination
        .remove(".coffret-fetch-never-created.part")
        .expect("a temporary file that was never created is already disposed of");

    let scratch = ".coffret-fetch-a-case.part";
    destination
        .create(scratch)
        .expect("creating a temporary file must succeed");
    destination
        .remove(scratch)
        .expect("removing it must succeed");
    destination
        .remove(scratch)
        .expect("removing it again must succeed too");
    assert!(
        !fixture.arrange().holds(&fixture.root().join(scratch)),
        "and it really is gone",
    );
}
