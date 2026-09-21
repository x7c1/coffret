/// The query that selects the live objects of one Library.
///
/// Trashed files are excluded here rather than filtered afterwards, so a page
/// Drive calls full really is a page of live objects.
pub fn live_files_query(folder_id: &str) -> String {
    format!("{} in parents and trashed = false", quoted(folder_id))
}

/// The same query, narrowed to the live objects of one name.
///
/// What asking whether a folder holds a named object comes to on Drive. Names
/// are not identity here — a folder may hold several files of one name, and the
/// id is what addresses any of them — so the question is this listing with a
/// name on it rather than a lookup, and its answer is how many came back rather
/// than which.
pub fn named_file_query(folder_id: &str, name: &str) -> String {
    format!(
        "{} and name = {}",
        live_files_query(folder_id),
        quoted(name)
    )
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
    // that is not this one's cannot answer it however the name is spelled.
    #[test]
    fn one_name_is_asked_for_inside_that_same_folder() {
        assert_eq!(
            named_file_query("folder-1", "head-0.cfrt"),
            "'folder-1' in parents and trashed = false and name = 'head-0.cfrt'"
        );
    }

    #[test]
    fn a_name_cannot_break_out_of_the_query_it_sits_in() {
        assert_eq!(
            named_file_query("folder-1", "a' or name = 'b"),
            "'folder-1' in parents and trashed = false and name = 'a\\' or name = \\'b'"
        );
    }
}
