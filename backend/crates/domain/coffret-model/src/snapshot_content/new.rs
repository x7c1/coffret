use super::SnapshotContent;
use crate::canonical_order::{require_strictly_increasing, CONTAINERS, ENTRIES};
use crate::container_summary::ContainerSummary;
use crate::control_object_name::ControlObjectName;
use crate::entry_location::EntryLocation;
use crate::error::{Error, Result};
use crate::index_checkpoint::IndexCheckpoint;

impl SnapshotContent {
    /// The content a Snapshot carries, or a refusal where it is not content a
    /// Snapshot can carry.
    ///
    /// # Errors
    ///
    /// - [`Error::CollectionOutOfCanonicalOrder`] where `containers` is not
    ///   strictly increasing by Container ID, or `entries` not strictly
    ///   increasing by the canonical bytes of their Entry Path (spec: FM-16,
    ///   EP-3).
    /// - [`Error::SnapshotEntryWithoutContainer`] where an Entry names a
    ///   Container `containers` does not list (spec: FM-16).
    pub fn new(
        checkpoint: IndexCheckpoint,
        adopted_from: Option<ControlObjectName>,
        containers: Vec<ContainerSummary>,
        entries: Vec<EntryLocation>,
    ) -> Result<Self> {
        require_strictly_increasing(CONTAINERS, &containers, |left, right| {
            left.id.cmp(&right.id)
        })?;
        require_strictly_increasing(ENTRIES, &entries, |left, right| {
            left.path().as_str().cmp(right.path().as_str())
        })?;
        // The Containers are in ID order by the check just made, so membership
        // is a search rather than a second collection.
        for (entry, location) in entries.iter().enumerate() {
            if containers
                .binary_search_by_key(&location.container_id, |container| container.id)
                .is_err()
            {
                return Err(Error::SnapshotEntryWithoutContainer {
                    entry,
                    container_id: location.container_id,
                });
            }
        }
        Ok(Self {
            checkpoint,
            adopted_from,
            containers,
            entries,
        })
    }

    /// The same content from collections in whatever order an Index reported
    /// them: sorted, then held to [`new`](Self::new)'s rules.
    ///
    /// Ordering by Entry Path bytes is ordering as EP-3 defines it —
    /// lexicographic over the canonical UTF-8 and independent of locale — so two
    /// devices canonicalize one Library's content identically and a Snapshot
    /// they each encode compares as one value.
    ///
    /// Sorting cannot make a Container listed twice or an Entry Path held twice
    /// disappear, so what this refuses is exactly what `new` refuses once the
    /// order is no longer in question.
    ///
    /// # Errors
    ///
    /// [`new`](Self::new)'s, on its terms.
    pub fn canonical(
        checkpoint: IndexCheckpoint,
        adopted_from: Option<ControlObjectName>,
        mut containers: Vec<ContainerSummary>,
        mut entries: Vec<EntryLocation>,
    ) -> Result<Self> {
        containers.sort_by_key(|container| container.id);
        entries.sort_by(|left, right| left.path().as_str().cmp(right.path().as_str()));
        Self::new(checkpoint, adopted_from, containers, entries)
    }
}
