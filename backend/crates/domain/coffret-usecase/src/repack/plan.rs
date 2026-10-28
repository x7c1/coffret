use coffret_format::EntryPlan;
use coffret_model::{
    ContainerId, ContainerKind, ContainerSummary, DepositedAt, EntryMetadata, EntryNumber,
    EntryPath,
};

use crate::freeze::segment::{segment, Planned, Segment};
use crate::index::Index;
use crate::index_error::IndexError;
use crate::repack::repack_error::RepackResult;

/// What a repack of one folder does, worked out from the catalog alone
/// (spec: PK-23).
///
/// Nothing here reads Storage or needs a key: which Packs take part, how their
/// Entries are cut, and which of the cut segments are a Pack already in place
/// are all questions about the catalog. Which Packs carry a key-lost marker is
/// the run's to ask of the Keyring afterwards, and only of the Packs it would
/// rewrite.
pub(super) struct Plan {
    /// Every current Pack holding at least one Entry under the folder, whole
    /// (spec: PK-23).
    pub(super) packs: Vec<Held>,
    /// The cut, in Entry Path order.
    pub(super) cuts: Vec<Cut>,
}

impl Plan {
    /// Whether the selected Pack at `pack` is a segment of the cut as it
    /// stands.
    pub(super) fn keeps(&self, pack: usize) -> bool {
        self.cuts
            .iter()
            .any(|cut| matches!(cut, Cut::Kept(kept) if *kept == pack))
    }
}

/// One selected Pack, as the catalog records it.
pub(super) struct Held {
    pub(super) summary: ContainerSummary,
    /// Its rows in entry number order, which is stream order (spec: FM-9):
    /// a row's position here is its entry number.
    pub(super) table: Vec<EntryMetadata>,
    /// When each row's content was deposited, row for row: what the new Pack
    /// carrying it keeps (spec: PK-10, FM-15).
    pub(super) deposit_times: Vec<DepositedAt>,
}

/// One segment of the cut.
pub(super) enum Cut {
    /// The selected Pack at this position already is this segment, and stays.
    Kept(usize),
    /// A new Pack to write, holding these Entries in this order.
    New(Segment<Member>),
}

/// One Entry of a selected Pack, as the cut places it.
pub(super) struct Member {
    /// Which selected Pack holds it now, by its position in [`Plan::packs`].
    pub(super) pack: usize,
    /// Its row's position in that Pack's table, which is its entry number
    /// there.
    pub(super) row: usize,
    /// What the new Pack's table will say about it: the row as it stands,
    /// under its current path (spec: PK-10).
    pub(super) plan: EntryPlan,
}

impl Planned for Member {
    fn plan(&self) -> &EntryPlan {
        &self.plan
    }
}

/// Selects, cuts, and sets aside what is already in place (spec: PK-23).
///
/// `folder` is the prefix the request names, `None` the Library root; the
/// selection is every current Pack holding an Entry under it — a one-file
/// Container is never one, whatever it holds, because the kind is recorded and
/// never inferred (spec: PK-15). The cut is the freeze's own over the selected
/// Entries sorted by current Entry Path, under the same target (spec: PK-3).
pub(super) async fn plan(
    index: &dyn Index,
    folder: Option<&EntryPath>,
    target: u64,
) -> RepackResult<Plan> {
    let mut packs = Vec::new();
    for summary in index.containers_under(folder).await? {
        if summary.kind != ContainerKind::Pack {
            continue;
        }
        // The Pack's whole table, not only its Entries under the folder: it is
        // replaced whole, so the rest has to go somewhere (spec: PK-23).
        let locations = index.entries_in(summary.id).await?;
        let mut table = Vec::with_capacity(locations.len());
        let mut deposit_times = Vec::with_capacity(locations.len());
        for (position, location) in locations.into_iter().enumerate() {
            // A current Pack's Entries are all current, and the catalog
            // answers them in entry number order, so the positions are the
            // numbers. Anything else is a catalog that cannot say which Entry
            // a new Pack carries.
            if EntryNumber::at_position(position).ok() != Some(location.entry_number) {
                return Err(gapped(summary.id).into());
            }
            table.push(location.entry);
            deposit_times.push(location.deposited_at);
        }
        packs.push(Held {
            summary,
            table,
            deposit_times,
        });
    }

    let mut members: Vec<Member> = packs
        .iter()
        .enumerate()
        .flat_map(|(pack, held)| {
            held.table
                .iter()
                .enumerate()
                .map(move |(row, entry)| Member {
                    pack,
                    row,
                    plan: EntryPlan::from(entry),
                })
        })
        .collect();
    // One Entry Path names at most one current Entry (spec: EP-5), so the
    // order is total.
    members.sort_by(|left, right| left.plan.path.cmp(&right.plan.path));

    let cuts = segment(members, target)?
        .into_iter()
        .map(|segment| match in_place(&packs, &segment.members) {
            Some(pack) => Cut::Kept(pack),
            None => Cut::New(segment),
        })
        .collect();
    Ok(Plan { packs, cuts })
}

/// The selected Pack a segment already is, if it is one.
///
/// Exactly one Pack's Entries, every one of them, in the order the Pack holds
/// them, each captured under the path it stands at now: rewriting it would
/// produce the same table and differ only in its key, so it is kept
/// (spec: PK-23). A Pack holding a renamed Entry is not in place even where
/// the order happens to survive the rename, because its meta section names
/// that Entry by a path the Library no longer uses (spec: FM-15).
fn in_place(packs: &[Held], members: &[Member]) -> Option<usize> {
    let pack = members.first()?.pack;
    let held = &packs[pack];
    let whole = members.len() == held.table.len()
        && members
            .iter()
            .enumerate()
            .all(|(position, member)| member.pack == pack && member.row == position);
    let current = held.table.iter().all(|row| row.captured_path == row.path);
    (whole && current).then_some(pack)
}

/// A catalog whose current Pack's Entries are not numbered from the first
/// without a gap.
fn gapped(container_id: ContainerId) -> IndexError {
    IndexError::UnreadableCatalog {
        operation: "reading the Packs a repack selects",
        cause: Box::<dyn std::error::Error + Send + Sync>::from(format!(
            "current Pack {container_id} does not answer every entry number from the first"
        ))
        .into(),
    }
}
