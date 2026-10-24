use coffret_model::{ContainerKind, ControlObjectName, Generation};

use crate::commit::{CommitOutcome, KeyringRepair, PreparedBatch};
use crate::commit_conformance::counting_store::CountingStore;
use crate::commit_conformance::fixtures::prepared;

mod rewrites;
pub use rewrites::{
    a_complete_set_costs_no_writes_and_reports_no_repair,
    a_lost_replica_is_rewritten_before_the_next_commit, an_unreadable_replica_is_replaced,
};

mod duplicates;
pub use duplicates::{
    a_replica_listed_twice_reads_as_its_valid_copy, two_devices_repairing_one_position_both_commit,
};

mod refusals;
pub use refusals::{
    a_keyring_no_replica_answers_stays_unreadable, a_repair_that_stops_reports_what_it_put_back,
    a_repair_the_provider_refuses_stops_the_commit,
    an_unfetchable_replica_stops_the_commit_unrewritten,
};

mod reports;
pub use reports::{
    a_repair_before_a_lost_slot_is_still_reported,
    a_repair_before_a_refused_record_is_still_reported,
    a_repair_before_the_rebases_ran_out_is_still_reported,
};

/// The one repair a run performed, or a panic saying what it reported instead.
///
/// A run carries a repair per attempt that put a position back, so "one repair"
/// is an assertion of its own: a case that read the first entry of the list
/// would pass just as well for a run that repaired the same set twice.
pub(super) fn one_repair(outcome: &CommitOutcome) -> &KeyringRepair {
    one_repair_of(&outcome.repairs)
}

/// The one repair a list of them holds, whichever of an outcome or a failure
/// it came on.
pub(super) fn one_repair_of(repairs: &[KeyringRepair]) -> &KeyringRepair {
    match repairs {
        [repair] => repair,
        other => panic!("expected the run to report one repair, got {other:?}"),
    }
}

/// The batch each round of these cases commits.
///
/// One Container at a path of its own, so that no round is refused for an Entry
/// Path another round already claimed (spec: EP-6) and every commit has
/// something to commit.
pub(super) fn adding(seed: u8) -> PreparedBatch {
    let path = format!("albums/{seed}.jpg");
    PreparedBatch::adding(vec![prepared(seed, ContainerKind::OneFile, &[&path])])
}

/// Every Keyring replica of one generation the run wrote, in the order it wrote
/// them.
///
/// The names rather than a count, because "which position" is the whole of what
/// a repair case is about: a run that rewrote the valid replicas alongside the
/// lost one would answer a count exactly as the right run does.
pub(super) fn replicas_written(
    counting: &CountingStore<'_>,
    generation: Generation,
) -> Vec<String> {
    counting
        .written()
        .into_iter()
        .filter(|name| is_replica_of(name, generation))
        .collect()
}

/// Whether a name is a Keyring replica of one generation (spec: FM-12).
fn is_replica_of(name: &str, generation: Generation) -> bool {
    matches!(
        ControlObjectName::parse(name),
        Ok(ControlObjectName::KeyringReplica { generation: found, .. }) if found == generation
    )
}
