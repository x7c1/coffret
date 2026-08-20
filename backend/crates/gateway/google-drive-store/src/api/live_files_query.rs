/// The query that selects the live objects of one Library.
///
/// Trashed files are excluded here rather than filtered afterwards, so a page
/// Drive calls full really is a page of live objects.
pub fn live_files_query(folder_id: &str) -> String {
    // Drive's query language quotes with single quotes and escapes with a
    // backslash. Folder ids do not contain either, but the id comes from the
    // provider, so it is escaped rather than trusted.
    let escaped = folder_id.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}' in parents and trashed = false")
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
}
