use coffret_model::EntryPath;

use super::Lapse;

/// A reserved file a freeze released its reservation without packing
/// (spec: PK-17).
///
/// What a reservation whose paths have none of them eligible any more leaves
/// behind, one per path: the reservation is released, so the files are no longer
/// waiting for a Pack, and the person who dropped them is owed the reason the
/// Pack they asked for was not built. A reservation that packs any of its paths
/// leaves none of these, whatever became of the rest — a book with pages
/// deleted since the drop packs the pages that remain.
///
/// The Entry Path travels in the value for the reason a
/// [`NotFrozen`](super::NotFrozen)'s does, and never into a diagnostic event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lapsed {
    /// Where in the Library the reserved file stands, or stood.
    pub path: EntryPath,
    /// Why it was not packed.
    pub lapse: Lapse,
}
