/// The query that selects the live objects of one Library.
///
/// Trashed files are excluded here rather than filtered afterwards, so a page
/// Drive calls full really is a page of live objects.
pub fn live_files_query(folder_id: &str) -> String {
    format!("{} in parents and trashed = false", quoted(folder_id))
}

/// The same query, narrowed to the live objects whose names contain any of
/// `prefixes`.
///
/// What asking a folder for objects by how their names start comes to on
/// Drive, which has no listing by prefix. Drive's `contains` on a name is
/// Drive's own matching of terms rather than a test of how the whole name
/// starts, so this narrows a listing without deciding it: a caller still reads
/// each name that comes back for what it is.
pub fn name_prefixes_query(folder_id: &str, prefixes: &[&str]) -> String {
    let names = prefixes
        .iter()
        .map(|prefix| format!("name contains {}", quoted(prefix)))
        .collect::<Vec<_>>()
        .join(" or ");
    format!("{} and ({names})", live_files_query(folder_id))
}

/// A value as Drive's query language spells one.
fn quoted(value: &str) -> String {
    // Drive's query language quotes with single quotes and escapes with a
    // backslash. Folder ids do not contain either and the names coffret writes
    // are a Container id or a generation, but both come from outside this
    // function, so they are escaped rather than trusted.
    let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listing_asks_only_for_live_objects_of_one_folder() {
        assert_eq!(
            live_files_query("folder-1"),
            "'folder-1' in parents and trashed = false"
        );
    }

    #[test]
    fn a_folder_id_cannot_break_out_of_the_query_it_sits_in() {
        assert_eq!(
            live_files_query("a' or name = 'b"),
            "'a\\' or name = \\'b' in parents and trashed = false"
        );
    }

    // The narrowed one is the same query with one clause on it, so a folder
    // that is not this one's cannot answer it however the prefixes are spelled.
    #[test]
    fn prefixes_are_asked_for_inside_that_same_folder() {
        assert_eq!(
            name_prefixes_query("folder-1", &["head-", "idx-"]),
            "'folder-1' in parents and trashed = false \
             and (name contains 'head-' or name contains 'idx-')"
        );
    }

    #[test]
    fn a_prefix_cannot_break_out_of_the_query_it_sits_in() {
        assert_eq!(
            name_prefixes_query("folder-1", &["a' or name = 'b"]),
            "'folder-1' in parents and trashed = false and (name contains 'a\\' or name = \\'b')"
        );
    }
}
