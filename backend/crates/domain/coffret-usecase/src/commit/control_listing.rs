use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, ControlObjectName, Generation, ObjectRef};
use tracing::{debug, warn};

use crate::commit::commit_error::{CommitError, CommitResult};
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// One walk of Storage, read for what the commit flow needs from it.
///
/// Recovery discovers control objects by name before any Index exists
/// (spec: FM-12, RV-1), and catching up is the same discovery: which head is
/// newest, and which checkpoints there are to start from. So the flow lists
/// once and keeps three things from it.
///
/// The handles are the third, and they are the reason this is a type rather
/// than two sets of generations. A store that mints identifiers does not name
/// objects by their names, so a caller cannot turn `head-7.cfrt` into something
/// [`get`](crate::ObjectStore::get) accepts — only the listing can, and it
/// already did.
#[derive(Debug, Default)]
pub(crate) struct ControlListing {
    handles: BTreeMap<String, ObjectRef>,
    heads: BTreeSet<Generation>,
    snapshots: BTreeSet<Generation>,
}

/// How many pages a listing may take before the walk calls Storage broken.
///
/// A provider that keeps handing back a token that makes no progress would
/// otherwise leave a commit spinning instead of failing.
const MAX_PAGES: usize = 100_000;

impl ControlListing {
    /// Walks the whole listing, page by page to its end.
    pub(crate) async fn read(store: &dyn ObjectStore, retry: &RetryPolicy) -> CommitResult<Self> {
        let mut listing = Self::default();
        let mut token = None;
        let mut pages = 0;
        loop {
            let page = retry.run("list", || store.list(token.as_ref())).await?;
            for object in page.objects {
                listing.record(object.name, object.object_ref)?;
            }
            token = page.next;
            pages += 1;
            if token.is_none() {
                return Ok(listing);
            }
            if pages >= MAX_PAGES {
                // Every page was answered, so this is not an answer that could
                // not be read: it is the listing outrunning the cap, which the
                // port has a word for.
                return Err(crate::Error::ListingPastCap {
                    pages,
                    source: None,
                }
                .into());
            }
        }
    }

    /// Files one listed object under whichever role its name states.
    ///
    /// A name that parses as no control object is a Container or something else
    /// entirely; either way it is kept only by name, because the removals a
    /// commit trashes are reached that way.
    ///
    /// A head or a checkpoint listed twice is refused. On a Storage that mints
    /// identifiers names are not unique, and two objects under one of those
    /// names say the exclusion the name stands for did not hold: two successors
    /// of one head (spec: CP-3), or two checkpoints of one (spec: CK-11). Which
    /// of the two is the Library's is nothing a reader can tell — each is a
    /// control object some writer committed — and whichever one it read, the
    /// other would be invisible to everything that reads by name, so the
    /// Library is inconsistent and the run stops rather than pick one
    /// (spec: FM-12, RV-1).
    ///
    /// A Keyring replica written twice is not that, and is only noted: its name
    /// fixes its key table, so a duplicate is the benign outcome of two repairs
    /// of one position, or of a repair beside a copy that did not read back
    /// valid, and either copy that does read back valid is the replica
    /// (spec: KL-14). The handle listed last is the one kept.
    fn record(&mut self, name: String, object_ref: ObjectRef) -> CommitResult<()> {
        let parsed = ControlObjectName::parse(&name);
        let listed_before = self.handles.contains_key(&name);
        match parsed {
            Ok(
                ref object @ (ControlObjectName::Head { generation }
                | ControlObjectName::IndexSnapshot { generation }),
            ) if listed_before => {
                warn!(
                    object = %name,
                    generation = generation.get(),
                    "Storage lists two control objects under one name; refusing the Library",
                );
                return Err(CommitError::DuplicateControlObject {
                    object: object.clone(),
                });
            }
            Ok(ControlObjectName::Head { generation }) => {
                self.heads.insert(generation);
            }
            Ok(ControlObjectName::IndexSnapshot { generation }) => {
                self.snapshots.insert(generation);
            }
            Ok(ControlObjectName::KeyringReplica { .. }) if listed_before => debug!(
                object = %name,
                "Storage lists a Keyring replica twice; only one of them is read",
            ),
            Ok(ControlObjectName::KeyringReplica { .. }) | Err(_) => {}
        }
        self.handles.insert(name, object_ref);
        Ok(())
    }

    /// The newest link in the control-head chain, or `None` in a Library that
    /// has committed nothing.
    pub(super) fn newest_head(&self) -> Option<Generation> {
        self.heads.last().copied()
    }

    /// The newest ordinary checkpoint's generation, from the names alone
    /// (spec: CK-8, FM-12).
    ///
    /// An activation Snapshot under a `head-` name is equally a checkpoint
    /// (spec: CK-9), and no name says so — only its authenticated header does.
    /// Recognizing one costs a fetch, which the catch-up pays when it is looking
    /// for a starting point; this is what the checkpoint policy counts from
    /// before that, and the catch-up raises it if it adopted something newer.
    pub(super) fn newest_snapshot(&self) -> Option<Generation> {
        self.snapshots.last().copied()
    }

    /// Whether an ordinary Snapshot of this head is on Storage.
    pub(super) fn has_snapshot(&self, generation: Generation) -> bool {
        self.snapshots.contains(&generation)
    }

    /// Whether the head chain has a link at this generation.
    pub(super) fn has_head(&self, generation: Generation) -> bool {
        self.heads.contains(&generation)
    }

    /// Every generation that could hold a checkpoint, newest first.
    ///
    /// Both kinds of name are candidates, because both kinds of object can be
    /// one: `idx-<generation>` always, `head-<generation>` when its header says
    /// activation Snapshot (spec: CK-9, FM-12).
    pub(super) fn checkpoint_candidates(&self) -> Vec<Generation> {
        let mut candidates: Vec<Generation> = self
            .snapshots
            .union(&self.heads)
            .copied()
            .collect::<Vec<_>>();
        candidates.sort_unstable_by(|left, right| right.cmp(left));
        candidates
    }

    /// The handle Storage named an object by, or `None` if the walk did not see
    /// it.
    pub(crate) fn handle(&self, name: &str) -> Option<&ObjectRef> {
        self.handles.get(name)
    }

    /// The handle Storage named one Container by (spec: FM-3).
    pub(crate) fn container(&self, container_id: ContainerId) -> Option<&ObjectRef> {
        self.handle(&container_id.object_name())
    }
}

#[cfg(test)]
mod tests {
    use coffret_format::keyring_set_digest;
    use coffret_logging::testing::CapturedLogs;
    use coffret_model::{KeyTable, ReplicaPosition};
    use tracing::Level;

    use super::*;
    use crate::generations::generation;

    // CP-3, CK-11, FM-12: two heads, or two checkpoints, under one name are an
    // exclusion that did not hold. Neither is the Library's more than the
    // other, so the listing refuses the Library and names the object.
    #[test]
    fn a_head_or_a_checkpoint_listed_twice_is_refused() {
        for (name, expected) in [
            ("head-3.cfrt", ControlObjectName::head(generation(3))),
            (
                "idx-5.cfrt",
                ControlObjectName::index_snapshot(generation(5)),
            ),
        ] {
            let logs = CapturedLogs::capture();
            let mut listing = ControlListing::default();

            listing
                .record(name.to_owned(), ObjectRef::new("first"))
                .expect("the first object under a name is the Library's");
            match listing.record(name.to_owned(), ObjectRef::new("second")) {
                Err(CommitError::DuplicateControlObject { object }) => {
                    assert_eq!(object, expected)
                }
                other => panic!("expected {name} listed twice to be refused, got {other:?}"),
            }
            let event = logs.only(Level::WARN);
            assert_eq!(event.field("object"), name, "{event}");
        }
    }

    // KL-14: a Keyring replica's name fixes its key table, so one written twice
    // is a harmless duplicate rather than a broken exclusion: it is neither
    // refused nor warned about. Nor are two distinct names, or a name that is no
    // control object's at all.
    #[test]
    fn nothing_else_listed_is_refused_or_warned_about() {
        let logs = CapturedLogs::capture();
        let mut listing = ControlListing::default();

        let replica = ControlObjectName::keyring_replica(
            Generation::FIRST,
            &keyring_set_digest(&KeyTable::default()).expect("a key table always digests"),
            ReplicaPosition::SINGLE,
        )
        .expect("a real digest names a replica")
        .to_string();
        for (name, object) in [
            (replica.as_str(), "first"),
            (replica.as_str(), "second"),
            ("head-1.cfrt", "one"),
            ("head-2.cfrt", "two"),
            ("not-a-control-object", "a"),
            ("not-a-control-object", "b"),
        ] {
            listing
                .record(name.to_owned(), ObjectRef::new(object))
                .expect("nothing here is two control objects of one name");
        }

        assert_eq!(
            listing.handle(&replica),
            Some(&ObjectRef::new("second")),
            "the replica listed last is the one kept",
        );
        assert_eq!(listing.newest_head(), Some(generation(2)));
        assert!(logs.at(Level::WARN).is_empty(), "{}", logs.text());
    }
}
