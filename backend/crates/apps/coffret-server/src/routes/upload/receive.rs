use std::collections::BTreeSet;
use std::future::Future;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::multipart::Field;
use coffret_device::{
    Btime, ContainerKind, EntryPath, Mtime, OpenLibrary, Redacted, Replacing, ReservationId,
};
use tracing::warn;

use crate::adds::{AddId, Adds, Replaced, Written};
use crate::allowance::Allowance;
use crate::api_error::ApiError;
use crate::entry_query::shaped;

use super::landed::Landed;
use super::modified::modified;
use super::outran::outran_as;
use super::refusal::Refusal;
use super::room_for::room_for;
use super::under::under;

/// Adds one file to the folder, or says why it was not added: the file one
/// part of a browser's drop carries, or one a drop by path read off this
/// device ([`Incoming`] says which).
///
/// One body for both, so that the room this device is asked to have, the name
/// and fence refusals, the per-file write and the stamp the file is kept with
/// are one implementation whichever way the bytes arrive.
///
/// The order is the point: the name is shaped, the catalog is asked what stands
/// at the path, and only then is anything opened. Everything that can refuse this
/// file has refused it before the first byte is written, so a refusal never
/// leaves a partial file behind — and a failure midway through does not either,
/// because the bytes are going to a scratch name that is removed when the
/// incoming file is dropped (spec: EP-11).
///
/// Two kinds of refusal come out of it, which is what [`Refusal`]'s first two
/// variants are for. One is about this file, and the rest of the drop carries on without
/// it. In the order this function meets them: its name is not an Entry Path; the
/// Library holds it inside a Pack; its name carries a component coffret keeps
/// for itself, refused by name before any disk is reached (spec: EP-11, EP-14);
/// or the way down from its mapped root passes through something that is not a
/// real folder of it, refused where the descent meets it (spec: EP-4, EP-11).
/// The whole set is enumerated once, in [the module's own](super) account of
/// what is refused before anything lands.
///
/// A failure is none of those — nothing about this file was decided — and it
/// reaches the caller the same way: a folder above it that could not be made, a
/// scratch name that could not be created, a catalog that would not answer.
/// [`Refusal`]'s own conversion makes each of them about this one file, so the
/// drop carries on without it rather than stopping at it.
///
/// The other kind is about the request: it has outrun a budget, this device has
/// not the room for what is still coming, or the part's mapped root is not the
/// root the mapping was recorded against, refused as that root is opened
/// (spec: EP-13). What the three have in common is the whole of why they reach
/// that far: none of them is truer of the next part than of this one — the
/// budget has already been passed, the disk is no roomier for the part behind
/// this one, and the root is the one every part of a drop onto a folder goes
/// through. Only a drop onto the Library root carries parts under mappings of
/// their own (spec: EP-9), and it stops at the first of those roots that is
/// refused: the request fails as a whole the way a declined placement fails a
/// single writer's (spec: EP-11).
///
/// And one thing comes out of it that is no refusal: the body stopping while
/// this part was in flight, which is handed back as
/// [`Interrupted`](Refusal::Interrupted) for the route to say as it says every
/// broken stream.
///
/// The file keeps its own modification time where the caller says what it is
/// ([`Times::modified`]), read before any of its bytes are: it is stamped on
/// the file before the rename, so what appears is the person's file in that
/// respect too, and recorded with the birth time the caller was told of, none
/// where it was told of none (spec: EP-11, FM-9).
///
/// `coming` is how much room the caller is to be asked to have. It is what the
/// request said is left of it, so a book being dropped asks for the rest of the
/// book and not for one page at a time.
///
/// `reserving` is the drop's reservation, where it asked for a Pack: the path
/// is added to it after every refusal has passed and before the rename puts the
/// file where a scan can find it, so no file of the drop ever stands in the
/// mapped folder without the reservation saying it waits for its Pack
/// (spec: PK-17). The path is noted in [`Reserving::paths`] as it is added,
/// whether or not the rename then succeeds, because that is what the
/// reservation names and so what the drop's freeze has to consider.
///
/// `adding` is the add the file is part of: what lands is noted on it — where
/// on this device, the Entry the Library held at the path before, and the
/// folders the descent made for it — so that the add can be undone if a person
/// stops it (spec: LA-36). A file standing where this one lands is kept aside
/// before the rename replaces it, so the undo can put it back; where this
/// folder's filesystem will not keep it aside, the add is noted as one a stop
/// cannot undo — unless a stop was asked already, which this file then
/// answers as [`Cancelled`](Refusal::Cancelled) without replacing anything. And a
/// stop asked while the file's bytes are arriving stops it between two of
/// them, as [`Cancelled`](Refusal::Cancelled), leaving nothing but the scratch the
/// incoming file removes as it is dropped (spec: EP-11).
#[allow(clippy::too_many_arguments)]
pub(super) async fn receive<S: Incoming>(
    library: &OpenLibrary,
    allowance: &Allowance,
    coming: u64,
    folder: Option<&EntryPath>,
    name: &str,
    reserving: Option<Reserving<'_>>,
    adding: &Adding<'_>,
    mut source: S,
) -> Result<Landed, Refusal> {
    let path = under(folder, &shaped(name)?);
    let times = source.times();
    if library.container_of(&path).await? == Some(ContainerKind::Pack) {
        return Err(ApiError::pack_resident().into());
    }
    let entry_before = library.entry_at(&path).await?;
    // Asked before the descent makes them, which is the one moment the
    // answer is still there to be read.
    let made = folders_missing(library, &path).await;

    let mut incoming = library.add_file(&path).await?;
    // Asked once the destination is open and before a byte of the part is
    // written, which is the one moment both halves of the question are answered:
    // the descent has arrived at the folder these bytes are going into, so what
    // is asked about is the volume they will land on rather than whatever a name
    // would have resolved to. A refusal here drops the incoming file, and
    // dropping it takes the empty scratch name with it (spec: EP-11).
    room_for(S::OPERATION, allowance, &incoming.scratch_path(), coming)?;

    let ceiling = source.ceiling(allowance);
    while let Some(chunk) = source.next_chunk().await? {
        if adding.stopping() {
            return Err(Refusal::Cancelled);
        }
        // Met before the bytes are written rather than after, so the file that is
        // refused is one this device never finished taking.
        //
        // Two sentences and not one: the first is the event's and names nothing,
        // the second is this person's and names their own file (spec: EL-1).
        // `outran_as` says why there are two of them.
        if ceiling
            .is_some_and(|ceiling| incoming.written().saturating_add(chunk.len() as u64) > ceiling)
        {
            return Err(Refusal::Request(outran_as(
                S::OPERATION,
                "one file in it is over that on its own, so dropping fewer beside it \
                 changes nothing",
                &format!(
                    "{name} is over that on its own, so dropping fewer beside it \
                     changes nothing"
                ),
            )));
        }
        incoming.write(&chunk).await?;
    }
    let bytes = incoming.written();
    // Kept aside once every byte is there and before the rename, the last
    // moment what stands at the name is still what the rename would replace.
    let replaced = match incoming.keep_aside()? {
        Replacing::Nothing => Replaced::Nothing,
        Replacing::Kept(kept) => Replaced::Kept(Arc::new(kept)),
        Replacing::Unkept(refused) => {
            if !adding.adds.writing_unkept(adding.id, &path) {
                return Err(Refusal::Cancelled);
            }
            // The path stays out of the event (spec: EL-1); a stop asked of
            // this add says which file it is.
            warn!(
                operation = S::OPERATION,
                reason = %refused.redacted(),
                "a file an add writes over could not be kept aside, so the add cannot be stopped",
            );
            Replaced::Unkept
        }
    };
    if let Some(reserving) = reserving {
        if let Err(refused) = library.extend_reservation(reserving.id, &path).await {
            unkept_not_written(adding, &replaced, &path);
            return Err(refused.into());
        }
        reserving.paths.insert(path.clone());
    }
    let local = incoming.landing_path();
    if let Err(refused) = incoming.keep_born(times.modified, times.born).await {
        unkept_not_written(adding, &replaced, &path);
        return Err(refused.into());
    }
    let identity = std::fs::symlink_metadata(&local)
        .ok()
        .map(|found| (found.dev(), found.ino()));
    adding.adds.wrote(
        adding.id,
        Written {
            path: path.clone(),
            local,
            identity,
            replaced,
            entry_before,
        },
        made,
    );
    Ok(Landed { path, bytes })
}

/// Takes back the note that the add writes over a file it cannot put back,
/// where the write did not happen after all: the file is still there.
fn unkept_not_written(adding: &Adding<'_>, replaced: &Replaced, path: &EntryPath) {
    if matches!(replaced, Replaced::Unkept) {
        adding.adds.not_written_unkept(adding.id, path);
    }
}

/// The folders on this device that do not stand yet on the way down to where
/// the file at `path` goes, outermost first: the ones adding it makes.
///
/// Nothing where the place cannot be said, which [`OpenLibrary::add_file`]
/// then refuses in words of its own.
async fn folders_missing(library: &OpenLibrary, path: &EntryPath) -> Vec<PathBuf> {
    let Ok(landing) = library.local_path_for(path).await else {
        return Vec::new();
    };
    let mut missing: Vec<PathBuf> = landing
        .ancestors()
        .skip(1)
        .take_while(|folder| std::fs::symlink_metadata(folder).is_err())
        .map(PathBuf::from)
        .collect();
    missing.reverse();
    missing
}

/// The add one file is part of (spec: LA-34).
pub(super) struct Adding<'a> {
    /// Where the server keeps its adds.
    pub(super) adds: &'a Adds,
    /// Which add this is.
    pub(super) id: &'a AddId,
}

impl Adding<'_> {
    /// Whether a person asked for the add to be stopped.
    pub(super) fn stopping(&self) -> bool {
        self.adds.stopping(self.id)
    }
}

/// A drop's reservation, as one part of it is taken (spec: PK-17).
pub(super) struct Reserving<'a> {
    /// The reservation the drop made before its first part.
    pub(super) id: ReservationId,
    /// Every path the drop has added to it so far.
    pub(super) paths: &'a mut BTreeSet<EntryPath>,
}

/// The times one file is kept with.
pub(super) struct Times {
    /// Its own modification time, where the caller knows it (spec: EP-11).
    pub(super) modified: Option<Mtime>,
    /// The person's file's own birth time, where the caller knows it
    /// (spec: FM-9).
    pub(super) born: Option<Btime>,
}

/// Where the bytes of one file come from, as [`receive`] takes them.
pub(super) trait Incoming: Send {
    /// The route the file arrives through, as its events name it: `upload`
    /// or `import`.
    const OPERATION: &'static str;

    /// The times the file is to be kept with, read before any of its bytes.
    fn times(&self) -> Times;

    /// The most one file may carry this way, or `None` where no ceiling of
    /// this route's bounds it.
    fn ceiling(&self, allowance: &Allowance) -> Option<u64>;

    /// The next stretch of the file, or `None` at its end.
    fn next_chunk(&mut self) -> impl Future<Output = Result<Option<Bytes>, Refusal>> + Send;
}

/// One part of a browser's multipart drop.
///
/// Its field name says its modification time ([`modified`]), and a browser
/// says no birth time. One part is held to the per-part budget (spec: LA-9).
impl Incoming for Field<'_> {
    const OPERATION: &'static str = super::OPERATION;

    fn times(&self) -> Times {
        Times {
            modified: modified(self.name()),
            born: None,
        }
    }

    fn ceiling(&self, allowance: &Allowance) -> Option<u64> {
        Some(allowance.part_bytes)
    }

    async fn next_chunk(&mut self) -> Result<Option<Bytes>, Refusal> {
        self.chunk().await.map_err(Refusal::reading)
    }
}
