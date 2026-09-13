use coffret_model::EntryPath;

/// How a message names the mapping a refusal is about (spec: EP-13).
///
/// The Library-side prefix, or the Library root where the mapping stands for
/// that and there is no component to name. Never the local root: the message
/// names that itself, and this stands beside it rather than instead of it.
///
/// The clause inside [`RefusedRoot`](crate::RefusedRoot)'s own sentence, which
/// every reading that meets that state renders — the refusal a fetch reports
/// while placing the rest of the Library, and the one a device raises while
/// placing a file it was handed, being one state met twice. It is named apart
/// from the sentence because it is the half that has two shapes: EP-9 admits a
/// mapping for a top-level component and one that stands for the whole Library,
/// and only one of them has a component to be named by.
///
/// The prefix is quoted because it stands next to a local path in every sentence
/// that uses this, and a bare name beside one reads as a second path.
pub(crate) fn mapping_named(prefix: Option<&EntryPath>) -> String {
    match prefix {
        Some(prefix) => format!("the mapping for {:?}", prefix.as_str()),
        None => "the mapping for the Library root".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_paths::entry_path;

    // The two halves of EP-9: a mapping for a top-level component, and the one
    // that stands for the whole Library and has no component to be named by.
    #[test]
    fn a_mapping_is_named_by_its_prefix_or_by_the_library_root() {
        assert_eq!(
            mapping_named(Some(&entry_path("albums"))),
            "the mapping for \"albums\"",
        );
        assert_eq!(mapping_named(None), "the mapping for the Library root");
    }
}
