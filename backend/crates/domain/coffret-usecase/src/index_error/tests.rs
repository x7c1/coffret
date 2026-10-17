use coffret_model::Redacted;

use super::*;
use crate::entry_paths::entry_path;

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

// EL-1: the path is what identifies the conflict to whoever is keeping the
// Library, and it is the one thing a diagnostic event may not say.
#[test]
fn a_conflict_over_one_path_says_how_long_it_was_and_no_more() {
    let error = IndexError::DuplicatePath {
        path: entry_path("albums/spring.jpg"),
    };

    assert!(error.to_string().contains("albums/spring.jpg"));
    assert_eq!(error.redacted(), "Index::DuplicatePath(path_len=17)");
}

// A build older than the file and a file older than the build are two
// different situations, and the one thing neither message may do is name a
// command: what the steps are called belongs to whatever shows the refusal.
#[test]
fn an_older_layout_and_a_newer_one_ask_for_different_things() {
    let older = IndexError::UnsupportedSchema {
        found: 3,
        supported: 5,
    };
    let newer = IndexError::UnsupportedSchema {
        found: 9,
        supported: 5,
    };

    assert!(older.to_string().contains("an older layout"), "{older}");
    assert!(newer.to_string().contains("newer than"), "{newer}");
    assert!(
        newer.to_string().contains("use the build that wrote it"),
        "{newer}"
    );
    for message in [older.to_string(), newer.to_string()] {
        assert!(
            message.contains("keep the Index file and its spools intact"),
            "{message}"
        );
        assert!(
            message.contains("migration that preserves device-local records"),
            "{message}"
        );
        assert!(
            message.contains("Storage can rebuild only the Catalog"),
            "{message}"
        );
        assert!(!message.contains("delete the Index file"), "{message}");
    }
}

// A value past what a column can hold is arithmetic and not Library
// content, so both renderings name the column and the number: a log saying
// only that something was refused would leave the reader unable to tell
// which. The one column that can still reach this is a device-observed
// size, every format number being bounded before it arrives (spec: FM-19).
#[test]
fn a_value_no_column_can_hold_is_named_with_the_number_and_the_column() {
    let error = IndexError::UnrepresentableValue {
        operation: "recording a materialized file",
        column: "observed_size",
        value: 1 << 63,
    };

    let message = error.to_string();
    assert!(message.contains("9223372036854775808"), "{message}");
    assert!(message.contains("observed_size"), "{message}");
    assert_eq!(
        error.redacted(),
        "Index::UnrepresentableValue(operation=recording a materialized file, \
         column=observed_size, value=9223372036854775808)",
    );
}

// The store's own message may name the catalog file, so what survives is
// the statement that was running. The store's answer still reaches a
// person, under this line rather than inside it: the operation is this
// layer's half and what the store said is the store's.
#[test]
fn what_the_index_store_reported_is_named_by_its_operation() {
    let error = IndexError::Backend {
        operation: "recording a mapping",
        cause: std::sync::Arc::new(std::io::Error::other(
            "unable to open database file /home/someone/library/index.db",
        )),
    };

    assert_eq!(
        chain(&error),
        vec![
            "the Index store failed while recording a mapping".to_owned(),
            "unable to open database file /home/someone/library/index.db".to_owned(),
        ],
    );
    assert_eq!(
        error.redacted(),
        "Index::Backend(operation=recording a mapping)",
    );
}

// FM-15, EL-1: a refused rename says which element broke which rule, in the
// message and in a diagnostic event alike, and neither carries an Entry Path:
// the element's position is what identifies it.
#[test]
fn a_refused_rename_names_its_element_and_its_rule_and_no_path() {
    let container_id = coffret_model::ContainerId::from_bytes([0xab; 16]);
    let error = IndexError::RenameRefused {
        element: 2,
        rule: RenameRule::EntryNumberOutsideTable {
            container_id,
            entry_number: 7,
        },
    };

    let message = error.to_string();
    assert!(
        message.starts_with("rename 2 of the record is refused"),
        "{message}"
    );
    assert!(message.contains("numbered 7"), "{message}");
    assert_eq!(
        error.redacted(),
        format!(
            "Index::RenameRefused(element=2, rule=EntryNumberOutsideTable(container={container_id}, entry=7))"
        )
    );
    assert_eq!(
        IndexError::RenameRefused {
            element: 1,
            rule: RenameRule::PathClaimedTwice { earlier: 0 },
        }
        .redacted(),
        "Index::RenameRefused(element=1, rule=PathClaimedTwice(earlier=0))"
    );
}

// CK-9, EL-1: a record refused as already applied says which generation it was
// and where the Index stands, and nothing else — two numbers are the whole of
// what a reader needs to see the replay was a second one.
#[test]
fn an_already_applied_record_names_its_generation_and_the_checkpoint() {
    let error = IndexError::AlreadyApplied {
        generation: Generation::new(3).expect("a literal generation"),
        checkpoint: Generation::new(5).expect("a literal generation"),
    };

    assert_eq!(
        error.to_string(),
        "the Journal record at generation 3 is already applied: the Index stands at head 5"
    );
    assert_eq!(
        error.redacted(),
        "Index::AlreadyApplied(generation=3, checkpoint=5)"
    );
}
