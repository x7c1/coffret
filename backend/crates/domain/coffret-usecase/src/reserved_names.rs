//! Which names inside a mapped folder are not the person's to bring into a
//! Library: the one rule every reader of such a folder and every writer into
//! one asks, so that the scan, a drop, and the files a person drops by path all
//! draw the same line.
//!
//! Three answers, because the three readers do different things with them —
//! the scan steps over the first two and stops at the third, a file dropped
//! through the explorer is refused by name for any of them — and none of them
//! may draw the line somewhere else (spec: EP-11, EP-14).

use crate::{root_marker, scratch};

/// Why a name is coffret's rather than the person's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservedName {
    /// It carries the scratch prefix a local writer fills a file under before
    /// the rename that publishes it (spec: EP-11).
    Scratch,
    /// It is the device's own management area, which holds the marker giving a
    /// mapped root its identity (spec: EP-14).
    ManagementArea,
    /// It is not the management area's name, and folds to it under ASCII case
    /// folding: a case-folding volume would not tell the two apart
    /// (spec: EP-14).
    FoldsToManagementArea,
}

/// Which of the three, if any, one name inside a mapped folder is.
///
/// The exact names are asked about before the folded spelling, so a name that
/// is both — which only the management area's own name could be — is answered
/// as the more precise of the two.
pub fn reserved_name(name: &str) -> Option<ReservedName> {
    if scratch::is_scratch(name) {
        return Some(ReservedName::Scratch);
    }
    if root_marker::is_management_area(name) {
        return Some(ReservedName::ManagementArea);
    }
    if root_marker::folds_to_management_area(name) {
        return Some(ReservedName::FoldsToManagementArea);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // EP-11, EP-14: the three answers, and a name of the person's own.
    #[test]
    fn each_reserved_name_is_answered_by_what_reserves_it() {
        let scratch = scratch::incoming_name();
        assert_eq!(reserved_name(scratch.as_str()), Some(ReservedName::Scratch));
        assert_eq!(
            reserved_name(root_marker::MANAGEMENT_AREA),
            Some(ReservedName::ManagementArea)
        );
        assert_eq!(
            reserved_name(".COFFRET"),
            Some(ReservedName::FoldsToManagementArea)
        );
        assert_eq!(reserved_name("page-001.jpg"), None);
    }
}
