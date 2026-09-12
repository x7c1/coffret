use coffret_model::{ContentHash, EntryMetadata, EntryPath, Redacted};
use tracing::{debug, warn};

use crate::descent_error::DescentError;
use crate::destination::Destination;
use crate::destinations::Destinations;
use crate::device_state::{DeviceTime, LocalObservation};
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::target::Target;
use crate::flushed_file::FlushedFile;
use crate::index::Index;
use crate::local_operation::LocalOperation;
use crate::refused_root::RefusedRoot;
use crate::scratch;
use crate::scratch_file::ScratchFile;

/// One Entry on its way into a mapped folder.
///
/// Two questions are settled here. **Where** the bytes may go is the mappings'
/// answer (spec: EP-9): the folder is reached by descending the Entry Path's
/// components from the mapped root one at a time, refusing anything that is not
/// a real folder of that root, and every call afterwards is made against the
/// [`Destination`] the descent left open rather than against a path joined back
/// together. So the answer is taken once, with the folder held open, and cannot
/// go stale before the write — and a path that cannot be reached that way is
/// refused rather than placed somewhere else, on the no-silent-selection posture
/// EP-4 sets.
///
/// **When** the file becomes visible is EP-11's, and the order is the only one
/// that gives it. The bytes go into a temporary file *in that folder* as they
/// arrive, the file is flushed, it is checked against the hash the current
/// catalog records for the Entry, it is stamped with the Entry's own
/// modification time — and only then is it renamed onto the final name. A rename
/// within one directory is atomic, so what a reader can see at the target path is
/// either nothing or the whole verified file: never a prefix of one, never one
/// whose stamp has yet to be set, and never one whose content has not been held
/// against the catalog.
///
/// The first three of those steps are the capability's own types, and the
/// transitions between them are what keeps the order: a [`ScratchFile`] is the
/// only thing that can be written and a [`FlushedFile`] the only thing that can
/// be published, so nothing here can publish a file whose bytes are not on the
/// device.
///
/// The three steps a caller drives are three calls because the bytes arrive over
/// a transfer rather than all at once. [`write`](Self::write) takes whatever
/// piece of the Entry the last chunk carried, [`verify`](Self::verify) holds it
/// against the catalog, and [`publish`](Self::publish) is the moment the file
/// exists. Between the second and the third a caller can hold a whole
/// Container's worth of verified placements, which is what lets the object's own
/// hash be checked before any of them becomes visible (spec: FM-15, CP-11).
///
/// A placement that is not published has to be [`discard`](Self::discard)ed.
/// What must not be left behind is a half-written file inside a folder the sync
/// walks: the scratch prefix already keeps a scan from committing one (see
/// [`crate::scratch`]), and removing it keeps the folder from accumulating them.
pub(super) struct Placement<'a> {
    /// The Entry, and where in the Library it stands.
    target: &'a Target,
    /// What the Container's own entry table records about it (spec: FM-9).
    entry: EntryMetadata,
    /// The destination folder, held open from the descent until the rename.
    directory: Box<dyn Destination>,
    /// What the temporary file is called inside it.
    scratch_name: String,
    /// The temporary file until it is flushed, and then what may be published.
    /// Exactly one of the two is ever set.
    file: Option<Box<dyn ScratchFile>>,
    flushed: Option<Box<dyn FlushedFile>>,
    /// The Entry's plaintext as it passes, for the check before the rename.
    hasher: blake3::Hasher,
    written: u64,
}

/// What opening a placement came to.
///
/// A refused root is not an error here, and that is the whole reason this is an
/// enum rather than a `Result`. The mapped root not being the folder the mapping
/// was recorded against is a fact about *one mapping* — every other mapping of
/// the device is sound — so a folder fetch reports it once and goes on, while a
/// caller placing one file turns it into [`FetchError::RefusedRoot`] (spec:
/// EP-11, EP-13). Both readings need the refusal as a value rather than as a
/// failure that has already decided which of the two it is.
///
/// The placement is boxed because a [`Placement`] is two kilobytes of hasher
/// state and a refusal is a path and a word: the two sit side by side here for
/// one call's worth of matching, and carrying the larger of them on the stack
/// through every return would be paying the difference on the ordinary path.
pub(super) enum Opened<'a> {
    /// The folder was reached and a temporary file is open inside it.
    Ready(Box<Placement<'a>>),
    /// The mapped root would not vouch for itself, so nothing was opened.
    RootRefused(RefusedRoot),
}

/// What one Container's worth of placing came to: the files that are verified
/// and still invisible, and the mappings nothing was placed under.
///
/// The two travel together because a run needs both halves to say what it did:
/// a caller reading only the placements would report a folder as a copy of the
/// Library while a whole mapping of it went untouched (spec: EP-11, EP-13).
pub(super) struct Placed<'a> {
    /// The verified placements, in the order the stream reached them.
    pub(super) placements: Vec<Placement<'a>>,
    /// One refusal per mapped root that would not vouch for itself.
    pub(super) refused: Vec<RefusedRoot>,
}

/// What a refused descent means for the Entry one target stands for.
///
/// The pair [`FetchError::from_descent`] is written from — the Entry Path the
/// refusal is about, and the mapping that says where its file would have gone —
/// both hang off the target, so every site here hands the target over rather
/// than repeating the pair. A free function and not a method, because the
/// descent that opens a placement has no placement yet.
fn refusal(target: &Target, refused: DescentError) -> FetchError {
    FetchError::from_descent(refused, target.place.prefix(), target.path())
}

impl<'a> Placement<'a> {
    /// Descends to the folder the Entry's file belongs in and opens a temporary
    /// file inside it.
    ///
    /// The descent holds the mapped root's marker against the identity the
    /// mapping records before it touches anything below the root (spec: EP-13),
    /// then makes the folders that are not there yet, because an Entry Path's
    /// separators are the whole of what a folder is (spec: EP-2), and refuses to
    /// pass through anything that is not a folder of the mapped root
    /// (spec: EP-4, EP-11). A path it will not descend is reported as
    /// [`FetchError::UnmaterializablePath`], which is the verdict every other
    /// path this device cannot make a file for already gets.
    ///
    /// It fails the run rather than becoming a finding, and that is the
    /// difference between here and the selection. The selection descended to
    /// this same place and found it sound; a fence met now is a name that has
    /// become a symbolic link since, which is a race on the disk rather than the
    /// shape it was in when the run was planned. A refused root is the exception:
    /// it comes back as [`Opened::RootRefused`] rather than as a failure, for the
    /// reason [`Opened`] gives.
    ///
    /// `entry` is the Container's own account of the Entry rather than the
    /// catalog's: it says how many bytes of the plaintext stream belong to this
    /// Entry, and holding the two accounts against each other is
    /// [`verify`](Self::verify)'s.
    pub(super) async fn open(
        destinations: &dyn Destinations,
        target: &'a Target,
        entry: EntryMetadata,
    ) -> FetchResult<Opened<'a>> {
        let directory = match target.place.descend(destinations).await {
            Ok(directory) => directory,
            Err(DescentError::Refused { root, reason }) => {
                return Ok(Opened::RootRefused(RefusedRoot {
                    prefix: target.place.prefix().cloned(),
                    local_root: root,
                    reason,
                }))
            }
            Err(refused) => return Err(refusal(target, refused)),
        };

        let scratch_name = scratch::name(target.location.container_id);
        let file = directory
            .create(&scratch_name)
            .map_err(|refused| refusal(target, refused))?;

        Ok(Opened::Ready(Box::new(Self {
            target,
            entry,
            directory,
            scratch_name,
            file: Some(file),
            flushed: None,
            hasher: blake3::Hasher::new(),
            written: 0,
        })))
    }

    /// Where this Entry's plaintext starts in its Container's stream
    /// (spec: FM-9).
    pub(super) fn start(&self) -> u64 {
        self.entry.extent.offset()
    }

    /// Where it ends.
    pub(super) fn end(&self) -> u64 {
        self.entry.extent.end()
    }

    /// Takes the next piece of the Entry's plaintext.
    pub(super) async fn write(&mut self, bytes: &[u8]) -> FetchResult<()> {
        let file = self
            .file
            .as_mut()
            .expect("a placement is written to before it is verified");
        // The refusal is turned into a verdict once the write has let go of the
        // handle, because reading the placement is what says which Entry it was
        // about.
        if let Err(refused) = file.write(bytes).await {
            return Err(refusal(self.target, refused));
        }
        self.hasher.update(bytes);
        self.written += bytes.len() as u64;
        Ok(())
    }

    /// Flushes the temporary file to the device and holds it against the
    /// catalog.
    ///
    /// This is the check EP-11 makes the condition of a file becoming visible,
    /// and it is the second half of a pair. The chunks the bytes came out of
    /// authenticated, which proves they are a coffret object sealed under the
    /// key that opens this Container (spec: FM-5, FM-8). The hash the Index
    /// carries came out of the Journal record (spec: CP-11), so comparing the
    /// two is what proves the bytes are the *committed content this catalog
    /// stands for* rather than merely a well-formed Container.
    ///
    /// Flushed before the comparison rather than after, because the rename that
    /// follows is what publishes the file: a crash that reordered the two would
    /// leave a name promising content the device never wrote.
    pub(super) async fn verify(&mut self) -> FetchResult<()> {
        let file = self
            .file
            .take()
            .expect("a placement is verified exactly once");
        let mut flushed = file
            .flush()
            .await
            .map_err(|refused| refusal(self.target, refused))?;

        let hash = ContentHash::from_bytes(*self.hasher.finalize().as_bytes());
        if self.written != self.entry.extent.size() || hash != self.target.location.entry.hash {
            return Err(FetchError::ContentMismatch {
                container_id: self.target.location.container_id,
                path: self.path().clone(),
            });
        }

        // Stamped on the handle this run has been writing to rather than by
        // opening the name again, and before the rename rather than after, so
        // that the file appearing at the final path is already the Entry's in
        // every respect (spec: FM-9, EP-11).
        flushed
            .stamp(self.entry.mtime)
            .await
            .map_err(|refused| refusal(self.target, refused))?;
        self.flushed = Some(flushed);
        Ok(())
    }

    /// Renames the verified file onto its final name and records having placed
    /// it.
    ///
    /// Both names are resolved against the folder the descent left open, so the
    /// file lands where the descent arrived whatever the path above it has
    /// become since (spec: EP-4, EP-11).
    ///
    /// The bookkeeping is not optional: fetching a file is exactly the second
    /// way a device materializes an Entry, so from here on the device may report
    /// the file as deleted if it goes missing, and the sync flow may offer a
    /// change to it back to the Library (spec: EP-10). What is recorded is the
    /// Entry's own size and modification time — which is what is now on disk —
    /// so the next scan and the next fetch both answer from the cheap comparison
    /// and open nothing.
    ///
    /// A rename that the operating system refuses takes the temporary file with
    /// it. This call consumes the placement, so no caller is left holding one to
    /// [`discard`](Self::discard), and what would otherwise stay behind is a
    /// scratch file inside a folder the sync walks. Once the rename has
    /// happened the file is the Entry's, and a bookkeeping failure after it
    /// leaves that file where it belongs rather than removing content this
    /// device verified.
    pub(super) async fn publish(
        mut self,
        index: &dyn Index,
        now: DeviceTime,
    ) -> FetchResult<EntryPath> {
        let flushed = self
            .flushed
            .take()
            .expect("a placement is verified before it is published");
        if let Err(cause) = flushed.publish() {
            let refused = refusal(self.target, cause);
            discard_all(vec![self]);
            return Err(refused);
        }

        index
            .mark_present(LocalObservation {
                path: self.path().clone(),
                size: self.entry.extent.size(),
                mtime: self.entry.mtime,
                at: now,
            })
            .await?;

        debug!(
            container = %self.target.location.container_id,
            bytes = self.written,
            "placed a fetched Entry and marked it present",
        );
        Ok(self.path().clone())
    }

    /// Removes the temporary file, this placement having come to nothing.
    ///
    /// One that is already gone is the same outcome as one this call removed, so
    /// a cleanup that races the failure it is cleaning up after still succeeds.
    ///
    /// Synchronous, because the capability's removal is: it is one call against
    /// a folder that has been held open all along.
    pub(super) fn discard(self) -> FetchResult<()> {
        // Whichever handle on the file is still open is closed before the name
        // it holds is removed.
        drop(self.file);
        drop(self.flushed);
        self.directory
            .remove(&self.scratch_name)
            .map_err(|refused| refusal(self.target, refused))
    }

    /// Where in the Library this placement stands.
    fn path(&self) -> &EntryPath {
        self.target.path()
    }
}

/// Publishes a whole Container's verified placements, discarding the rest if
/// one of them cannot be published.
///
/// The renames happen one after another and are not undone: each is a file that
/// is fully verified whichever of its neighbours fails, and a run that stopped
/// half way has placed those and reported the failure. What it does not do is
/// walk away from the temporary files it had not got to yet.
pub(super) async fn publish_all(
    index: &dyn Index,
    now: DeviceTime,
    placements: Vec<Placement<'_>>,
) -> FetchResult<Vec<EntryPath>> {
    let mut placed = Vec::with_capacity(placements.len());
    let mut left = placements.into_iter();
    while let Some(placement) = left.next() {
        match placement.publish(index, now).await {
            Ok(path) => placed.push(path),
            Err(error) => {
                discard_all(left.collect());
                return Err(error);
            }
        }
    }
    Ok(placed)
}

/// Removes every temporary file a failed fetch left.
///
/// A cleanup failure is reported and not raised: what the caller is about to
/// report is the failure that made the cleanup necessary, and replacing it with
/// "and the temporary file would not go either" would lose the verdict. The path
/// stays out of the event, as it stays out of a message (spec: EL-1).
pub(super) fn discard_all(placements: Vec<Placement<'_>>) {
    for placement in placements {
        if let Err(error) = placement.discard() {
            warn!(
                operation = %LocalOperation::Removing,
                error = %error.redacted(),
                "a fetch could not remove one of its own temporary files",
            );
        }
    }
}
