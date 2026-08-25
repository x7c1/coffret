use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, ControlObjectName, Generation, ObjectRef};

use crate::commit::commit_error::CommitResult;
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
    pub(super) async fn read(store: &dyn ObjectStore, retry: &RetryPolicy) -> CommitResult<Self> {
        let mut listing = Self::default();
        let mut token = None;
        let mut pages = 0;
        loop {
            let page = retry.run("list", || store.list(token.as_ref())).await?;
            for object in page.objects {
                listing.record(object.name, object.object_ref);
            }
            token = page.next;
            pages += 1;
            if token.is_none() {
                return Ok(listing);
            }
            if pages >= MAX_PAGES {
                return Err(crate::Error::MalformedResponse {
                    detail: format!("the listing did not end within {MAX_PAGES} pages"),
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
    fn record(&mut self, name: String, object_ref: ObjectRef) {
        match ControlObjectName::parse(&name) {
            Ok(ControlObjectName::Head { generation }) => {
                self.heads.insert(generation);
            }
            Ok(ControlObjectName::IndexSnapshot { generation }) => {
                self.snapshots.insert(generation);
            }
            Ok(ControlObjectName::KeyringReplica { .. }) | Err(_) => {}
        }
        self.handles.insert(name, object_ref);
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
