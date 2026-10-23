use std::error::{self, Error as _};

use coffret_model::{ContainerId, Generation, Redacted};

use super::fixtures::{provider_fault, unopenable};
use super::*;
use crate::commit::EntryRef;
use crate::entry_paths::entry_path;

// EP-6: the path is what identifies the conflict to a person, and it is
// the one thing in this vocabulary a diagnostic event may not carry.
#[test]
fn two_entries_claiming_one_path_are_recorded_without_it() {
    let error = CommitError::EntryPathCollision {
        path: entry_path("albums/spring.jpg"),
    };

    assert!(error.to_string().contains("albums/spring.jpg"));
    assert_eq!(error.redacted(), "Commit::EntryPathCollision(path_len=17)");
}

// EP-7: a rebase conflict names the change, the record and the rule to a
// person along with the path the two sides meet at, and a diagnostic event
// gets everything but the path.
#[test]
fn a_rebase_conflict_is_recorded_without_its_path() {
    let entry = EntryRef::new(
        ContainerId::from_bytes([0x11; ContainerId::BYTE_LEN]),
        coffret_model::EntryNumber::FIRST,
    );
    let error = CommitError::RebaseConflict {
        pending: PendingChange::Rename(entry),
        record: Generation::new(7).expect("a small generation is one"),
        rule: RebaseRule::RenamedEntryRenamed,
        path: entry_path("albums/spring.jpg"),
    };

    let said = error.to_string();
    assert!(said.contains("albums/spring.jpg"), "{said}");
    assert!(said.contains("generation 7"), "{said}");
    assert!(said.contains("renamed the same Entry"), "{said}");
    assert_eq!(
        error.redacted(),
        format!(
            "Commit::RebaseConflict(pending=Rename(container={}, entry=0), record=7, \
             rule=RenamedEntryRenamed, path_len=17)",
            entry.container_id
        ),
    );
}

// FM-15: a rename of the batch's own that the head does not admit says
// which one and which rule, and no path either way.
#[test]
fn a_refused_rename_names_its_position_and_rule() {
    let error = CommitError::RenameRefused {
        element: 2,
        rule: RenameRule::PathTaken,
    };

    assert_eq!(
        error.to_string(),
        "rename 2 of the batch is refused: another Entry would stand at the same path"
    );
    assert_eq!(
        error.redacted(),
        "Commit::RenameRefused(element=2, rule=PathTaken)"
    );
}

// CP-18: the Containers a refused batch would have removed are what a
// person and a diagnostic event both need, and neither carries a path. The
// sentence ends the chain, so it carries the next step itself.
#[test]
fn a_removal_no_longer_current_names_every_container() {
    let first = ContainerId::from_bytes([1; 16]);
    let second = ContainerId::from_bytes([2; 16]);
    let error = CommitError::RemovalNotCurrent {
        container_ids: vec![first, second],
    };

    let said = error.to_string();
    assert!(said.contains(&first.to_string()), "{said}");
    assert!(said.contains(&second.to_string()), "{said}");
    assert!(
        said.ends_with("running again plans afresh from the Library as it stands now"),
        "{said}"
    );
    assert_eq!(
        error.redacted(),
        format!("Commit::RemovalNotCurrent(containers=[{first}, {second}])"),
    );
    assert!(error.source().is_none());
    assert_eq!(error.advice(), None);
}

// A commit's own bookkeeping is worth reading whole: nothing in it is
// anybody's name for anything.
#[test]
fn a_keyring_that_would_not_read_keeps_its_generation_and_its_replica() {
    let error = CommitError::KeyringUnreadable {
        generation: Generation::FIRST,
        replica: 2,
        cause: UnusableReplica::Absent,
    };

    assert_eq!(
        error.redacted(),
        "Commit::KeyringUnreadable(generation=0, replica=2): Replica::Absent",
    );
}

// KL-16: the refusal is one a person acts on, so it has to carry all three
// of what is wrong, what it cost them, and what ends it — the first two in
// the sentence, and the third as advice a shell prints after the chain,
// because the sentence has a source and advice inside it would be read
// before the cause. The diagnostic event carries the same verdict as facts
// and none of the advice, and the chain reaches what Storage said.
//
// KL-15: and where the same examination did put positions back before it
// stopped, the sentence says so, because that work is a repair performed and
// this refusal is the only thing a run that was refused hands back. A run
// that put none back says nothing about it rather than reporting a zero.
#[test]
fn a_keyring_that_could_not_be_repaired_says_what_to_do_about_it() {
    let error = CommitError::UnrepairedKeyring {
        generation: Generation::FIRST,
        needed: vec![1, 2],
        rewritten: Vec::new(),
        replica: 1,
        cause: UnrepairedReplica::Unwritten(Box::new(provider_fault())),
    };

    let said = error.to_string();
    assert!(
        said.contains("short of a valid replica at 2 of its positions"),
        "{said}"
    );
    assert!(
        said.contains("nothing of this batch was committed"),
        "{said}"
    );
    assert!(
        said.contains("reads and restores go on"),
        "a write the gate refuses leaves reading alone, and the person told \
         their backup will not take a write asks about reading next \
         (spec: KL-16): {said}",
    );
    assert!(
        !said.contains("running again"),
        "what to do is advice, printed after the cause rather than before it: {said}",
    );
    assert_eq!(
        error.advice(),
        Some("running again examines the committed Keyring and repairs it afresh"),
    );
    assert!(
        !said.contains("other replica"),
        "a run that put nothing back reports no repair rather than a zero: {said}",
    );
    assert!(
        error.source().is_some(),
        "the reason the repair stopped is the refusal's source",
    );
    assert_eq!(
        error.redacted(),
        format!(
            "Commit::UnrepairedKeyring(generation=0, needed=2, rewritten=0, replica=1): \
             Repair::Unwritten: {}",
            provider_fault().redacted(),
        ),
    );

    let partial = CommitError::UnrepairedKeyring {
        generation: Generation::FIRST,
        needed: vec![2],
        rewritten: vec![0, 1],
        replica: 2,
        cause: UnrepairedReplica::Unwritten(Box::new(provider_fault())),
    };

    let said = partial.to_string();
    assert!(
        said.contains("short of a valid replica at 1 of its positions"),
        "{said}"
    );
    assert!(
        said.contains("2 other replicas were rewritten and stand"),
        "the repair it did perform is in the sentence: {said}",
    );
    assert_eq!(
        partial.redacted(),
        format!(
            "Commit::UnrepairedKeyring(generation=0, needed=1, rewritten=2, replica=2): \
             Repair::Unwritten: {}",
            provider_fault().redacted(),
        ),
    );
}

/// The links a caller printing `{error:#}` reads, outermost first.
fn chain(error: &dyn error::Error) -> Vec<String> {
    let mut links = vec![error.to_string()];
    let mut below = error.source();
    while let Some(link) = below {
        links.push(link.to_string());
        below = link.source();
    }
    links
}

// A wrapper says which layer, the cause says what that layer answered, and
// the chain a caller prints holds each of those once — all the way down
// through the repair's own vocabulary to what Storage said. What to do
// about it is in no link: it is the advice a shell prints after the last.
#[test]
fn a_stopped_repair_reaches_a_caller_as_one_sentence_per_layer() {
    let error = CommitError::UnrepairedKeyring {
        generation: Generation::FIRST,
        needed: vec![1, 2],
        rewritten: Vec::new(),
        replica: 1,
        cause: UnrepairedReplica::Unwritten(Box::new(provider_fault())),
    };

    assert_eq!(
        chain(&error),
        vec![
            "the committed Keyring generation 0 is short of a valid replica at 2 of its \
             positions and could not be repaired: the repair stopped at replica 1; nothing \
             of this batch was committed, and reads and restores go on from the replicas \
             that survive"
                .to_owned(),
            "that replica could not be written".to_owned(),
            "the commit did not get what it asked of Storage".to_owned(),
            "Storage failed with status 503: backendError".to_owned(),
        ],
    );
    assert!(
        chain(&error)
            .iter()
            .all(|link| !link.contains("running again")),
        "the advice is said once, after the chain, and in none of its links",
    );
    assert!(error.advice().is_some());
}

// FM-15: a Library whose Journal records predate the schema this build
// reads is told to be made again, after the chain that says which schema
// was found, and not left reading as a Library that is broken.
#[test]
fn a_record_of_an_older_schema_advises_making_the_library_again() {
    let error =
        CommitError::Format(coffret_format::Error::UnsupportedJournalRecordSchema { schema: 2 });
    let advice = error.advice().expect("an older schema has advice");
    assert!(advice.contains("predates this build's format"), "{advice}");
    assert!(advice.contains("coffret init"), "{advice}");
    assert!(
        chain(&error).iter().all(|link| !link.contains("predates")),
        "the advice is said once, after the chain, and in none of its links",
    );
}

// Every other refusal either has no source, so its sentence is already the
// last line a person reads, or says nothing a person can do: none of them
// has advice to print after the chain.
#[test]
fn only_a_stopped_repair_and_an_older_schema_have_advice_to_print_after_the_chain() {
    for error in [
        provider_fault(),
        unopenable(),
        CommitError::ConflictLimitReached { attempts: 8 },
        CommitError::EpochActivated {
            generation: Generation::FIRST,
        },
        CommitError::MissingHead {
            generation: Generation::FIRST,
        },
    ] {
        assert_eq!(error.advice(), None, "{error:?}");
    }
}
