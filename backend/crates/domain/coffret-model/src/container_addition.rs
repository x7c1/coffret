use std::collections::BTreeSet;

use crate::container_summary::ContainerSummary;
use crate::deposited_at::DepositedAt;
use crate::entry_location::EntryLocation;
use crate::entry_metadata::EntryMetadata;
use crate::entry_number::EntryNumber;
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
/// Beside the table rides each Entry's deposit time, one per Entry and in the
/// table's order: the catalog's value rather than the Container's, which is
/// why a record carries it and the meta section does not (spec: FM-9, FM-15).
///
/// What makes an entry table one is stated by [`new`](Self::new) and nowhere
/// else: a reader handed one of these has a Container holding at least one
/// Entry whose Entries tile its plaintext stream, each with its deposit time,
/// whether the value came off a wire, out of a catalog, or out of a spool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerAddition {
    container: ContainerSummary,
    entries: Vec<EntryMetadata>,
    deposit_times: Vec<DepositedAt>,
}

impl ContainerAddition {
    /// The addition of `container` holding `entries`, deposited at `deposit_times`
    /// — one time per Entry, in the table's order — or a refusal where the
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
    /// - [`Error::EntryNumberOutOfRange`] where the table is longer than entry
    ///   numbers reach, so that some Entry of it would have none (spec: FM-9).
    /// - [`Error::AdditionDepositTimesDoNotMatchEntries`] where there is not one
    ///   deposit time per Entry (spec: FM-15).
    pub fn new(
        container: ContainerSummary,
        entries: Vec<EntryMetadata>,
        deposit_times: Vec<DepositedAt>,
    ) -> Result<Self> {
        if entries.is_empty() {
            return Err(Error::AdditionWithoutEntries);
        }
        if deposit_times.len() != entries.len() {
            return Err(Error::AdditionDepositTimesDoNotMatchEntries {
                entries: entries.len(),
                deposit_times: deposit_times.len(),
            });
        }
        EntryNumber::at_position(entries.len() - 1)?;
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
        Ok(Self {
            container,
            entries,
            deposit_times,
        })
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

    /// When each Entry of [`entries`](Self::entries) was deposited, in the same
    /// order (spec: FM-15).
    pub fn deposit_times(&self) -> &[DepositedAt] {
        &self.deposit_times
    }

    /// The three parts, for a caller that rebuilds the addition around them —
    /// a rebase taking a renamed Entry over at its new path.
    pub fn into_parts(self) -> (ContainerSummary, Vec<EntryMetadata>, Vec<DepositedAt>) {
        (self.container, self.entries, self.deposit_times)
    }

    /// The Container and its Entries as the catalog locates them, each with
    /// the entry number its position in the table gives it (spec: FM-9,
    /// FM-15).
    ///
    /// An addition carries its numbers implicitly, in the order of the table it
    /// copies, so this is where a replay reads them off: the first Entry is
    /// number 0, and so on without gaps.
    pub fn into_locations(self) -> (ContainerSummary, Vec<EntryLocation>) {
        let container_id = self.container.id;
        let locations = self
            .entries
            .into_iter()
            .zip(self.deposit_times)
            .enumerate()
            .map(|(position, (entry, deposited_at))| EntryLocation {
                container_id,
                entry_number: EntryNumber::at_position(position)
                    .expect("`new` refused a table longer than entry numbers reach"),
                entry,
                deposited_at,
            })
            .collect();
        (self.container, locations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{container_summary, deposit_times, table, table_at};

    // FM-10: a Container is built out of Entries, so a table holding none
    // describes a Container no writer produces.
    #[test]
    fn an_addition_numbers_its_entries_by_position() {
        let addition = ContainerAddition::new(
            container_summary(1),
            table(&[(0, 4), (4, 4), (8, 0)]),
            deposit_times(3),
        )
        .expect("a table that tiles");
        let (_, locations) = addition.into_locations();
        let numbers: Vec<u32> = locations
            .iter()
            .map(|location| location.entry_number.get())
            .collect();
        assert_eq!(numbers, [0, 1, 2]);
        assert!(locations
            .iter()
            .all(|location| location.container_id == container_summary(1).id));
    }

    #[test]
    fn an_addition_with_no_entries_cannot_exist() {
        let result = ContainerAddition::new(container_summary(1), Vec::new(), Vec::new());

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
            let entries = table(&layout);
            let count = entries.len();
            let result =
                ContainerAddition::new(container_summary(1), entries, deposit_times(count));
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
        ContainerAddition::new(
            container_summary(1),
            table(&[(0, 10), (10, 0), (10, 7)]),
            deposit_times(3),
        )
        .expect("a table that tiles from zero, a file of no bytes among it, is a table");
    }

    // EP-5: one Entry Path holds at most one current Entry, so a Container
    // whose table names one twice would put two current Entries at one position
    // the moment its record is applied.
    #[test]
    fn an_addition_naming_one_path_twice_cannot_exist() {
        let entries = table_at(&[("albums/one.jpg", 0, 4), ("albums/one.jpg", 4, 4)]);
        let result = ContainerAddition::new(container_summary(1), entries, deposit_times(2));

        assert!(
            matches!(result, Err(Error::AdditionNamesOnePathTwice { entry: 1 })),
            "expected the second Entry to be refused, got {result:?}",
        );
    }

    // FM-15: every Entry an addition carries has its own deposit time, so a
    // table and its times disagreeing in length is no addition, and the times
    // reach the catalog's locations Entry for Entry.
    #[test]
    fn an_addition_carries_one_deposit_time_per_entry() {
        let result = ContainerAddition::new(
            container_summary(1),
            table(&[(0, 4), (4, 4)]),
            deposit_times(1),
        );
        assert!(
            matches!(
                result,
                Err(Error::AdditionDepositTimesDoNotMatchEntries {
                    entries: 2,
                    deposit_times: 1
                })
            ),
            "expected a short list of deposit times to be refused, got {result:?}",
        );

        let times = vec![
            DepositedAt::from_unix_seconds(1_800_000_000),
            DepositedAt::from_unix_seconds(1_500_000_000),
        ];
        let addition = ContainerAddition::new(
            container_summary(1),
            table(&[(0, 4), (4, 4)]),
            times.clone(),
        )
        .expect("one deposit time per Entry");
        let (_, locations) = addition.into_locations();
        let found: Vec<DepositedAt> = locations
            .iter()
            .map(|location| location.deposited_at)
            .collect();
        assert_eq!(found, times);
    }
}
