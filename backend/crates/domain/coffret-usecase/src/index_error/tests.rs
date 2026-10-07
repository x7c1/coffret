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
        assert!(message.contains("delete the Index file"), "{message}");
        assert!(message.contains("record those mappings again"), "{message}");
        assert!(message.contains("catch up from Storage"), "{message}");
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
        cause: "unable to open database file /home/someone/library/index.db".into(),
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
