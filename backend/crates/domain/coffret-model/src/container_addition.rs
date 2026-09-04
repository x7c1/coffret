use std::collections::BTreeSet;

use crate::container_summary::ContainerSummary;
use crate::entry_metadata::EntryMetadata;
use crate::entry_path::EntryPath;
use crate::error::{Error, Result};

/// One Container a Journal record adds, with everything it holds.
///
/// A record carries each new Container's ciphertext hash, its kind, and its
/// entry table — the values the meta section records, under the catalog's own
/// field names (spec: FM-15) — which is exactly what lets a device replaying
/// the record rebuild its Index without opening a single Container
/// (spec: CP-11, CK-9, RV-5). The Container's authenticated
/// meta section remains the authority on what it holds; this is the copy the
/// record travels with.
///
/// No Key Envelope ever rides here: which Containers are current is the
/// Journal's business, and the committed Keyring is the only Storage home of
/// the keys that open them (spec: CP-11).
///
/// What makes an entry table one is stated by [`new`](Self::new) and nowhere
/// else: a reader handed one of these has a Container holding at least one
/// Entry whose Entries tile its plaintext stream, whether the value came off a
/// wire, out of a catalog, or out of a spool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerAddition {
    container: ContainerSummary,
    entries: Vec<EntryMetadata>,
}

impl ContainerAddition {
    /// The addition of `container` holding `entries`, or a refusal where the
    /// table is not one a Container can have.
    ///
    /// The table is walked once in the order it was handed over, which is the
    /// order the plaintext stream lays the Entries down in (spec: FM-9): every
    /// Entry starts where its predecessor ended, and the first starts at zero.
    /// A gap, an overlap, and a table beginning anywhere but the start of the
    /// stream are therefore one refusal, raised at the Entry that breaks the
    /// walk.
    ///
    /// # Errors
    ///
    /// - [`Error::AdditionWithoutEntries`] where the table is empty
    ///   (spec: FM-10).
    /// - [`Error::AdditionEntriesDoNotTile`] where an Entry does not begin
    ///   where the walk had reached (spec: FM-9).
    /// - [`Error::AdditionNamesOnePathTwice`] where two Entries of the table
    ///   name one Entry Path (spec: EP-5).
    pub fn new(container: ContainerSummary, entries: Vec<EntryMetadata>) -> Result<Self> {
        if entries.is_empty() {
            return Err(Error::AdditionWithoutEntries);
        }
        let mut expected = 0u64;
        let mut named: BTreeSet<&EntryPath> = BTreeSet::new();
        for (entry, metadata) in entries.iter().enumerate() {
            let found = metadata.extent.offset();
            if found != expected {
                return Err(Error::AdditionEntriesDoNotTile {
                    entry,
                    expected,
                    found,
                });
            }
            expected = metadata.extent.end();
            if !named.insert(&metadata.path) {
                return Err(Error::AdditionNamesOnePathTwice { entry });
            }
        }
        Ok(Self { container, entries })
    }

    /// What the record records about the Container itself.
    pub const fn container(&self) -> &ContainerSummary {
        &self.container
    }

    /// The Container's entry table, in the plaintext stream order FM-9 fixes
    /// (spec: FM-15).
    pub fn entries(&self) -> &[EntryMetadata] {
        &self.entries
    }

    /// The two halves, for a caller that consumes both — a replay inserting the
    /// Container and then every Entry it holds.
    pub fn into_parts(self) -> (ContainerSummary, Vec<EntryMetadata>) {
        (self.container, self.entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{container_summary, table, table_at};

    // FM-10: a Container is built out of Entries, so a table holding none
    // describes a Container no writer produces.
    #[test]
    fn an_addition_with_no_entries_cannot_exist() {
        let result = ContainerAddition::new(container_summary(1), Vec::new());

        assert!(
            matches!(result, Err(Error::AdditionWithoutEntries)),
            "expected an empty table to be refused, got {result:?}",
        );
    }

    // FM-9: the table tiles the plaintext stream from zero. A gap, an overlap,
    // and a table that starts late all break the same walk, and the refusal
    // names the Entry it broke at along with both offsets.
    #[test]
    fn an_addition_whose_entries_do_not_tile_cannot_exist() {
        let cases = [
            ("a gap", vec![(0, 10), (12, 5)], 1usize, 10u64, 12u64),
            ("an overlap", vec![(0, 10), (8, 5)], 1, 10, 8),
            ("a late start", vec![(4, 10)], 0, 0, 4),
        ];
        for (what, layout, entry, expected, found) in cases {
            let result = ContainerAddition::new(container_summary(1), table(&layout));
            assert!(
                matches!(
                    result,
                    Err(Error::AdditionEntriesDoNotTile {
                        entry: at,
                        expected: reached,
                        found: claimed,
                    }) if at == entry && reached == expected && claimed == found
                ),
                "expected {what} to be refused at entry {entry}, got {result:?}",
            );
        }
        ContainerAddition::new(container_summary(1), table(&[(0, 10), (10, 0), (10, 7)]))
            .expect("a table that tiles from zero, a file of no bytes among it, is a table");
    }

    // EP-5: one Entry Path holds at most one current Entry, so a Container
    // whose table names one twice would put two current Entries at one position
    // the moment its record is applied.
    #[test]
    fn an_addition_naming_one_path_twice_cannot_exist() {
        let entries = table_at(&[("albums/one.jpg", 0, 4), ("albums/one.jpg", 4, 4)]);
        let result = ContainerAddition::new(container_summary(1), entries);

        assert!(
            matches!(result, Err(Error::AdditionNamesOnePathTwice { entry: 1 })),
            "expected the second Entry to be refused, got {result:?}",
        );
    }
}
