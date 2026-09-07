use std::path::Path;

use crate::device_state::RootIdentity;
use crate::in_memory_fs::state::lock;
use crate::in_memory_fs::InMemoryFs;
use crate::local_operation::LocalOperation;

impl InMemoryFs {
    /// Makes the `nth` (1-based) invocation of `operation` fail.
    ///
    /// The count is per operation, so scripting the second
    /// [`Writing`](LocalOperation::Writing) fails the second write whatever else
    /// the run did in between. Nine operations are worth scripting, and they are
    /// the ones the three capabilities perform:
    ///
    /// - [`Creating`](LocalOperation::Creating) for
    ///   [`create`](crate::Spool::create),
    ///   [`Writing`](LocalOperation::Writing) and
    ///   [`Flushing`](LocalOperation::Flushing) for the writer it hands back,
    ///   and [`Removing`](LocalOperation::Removing) for
    ///   [`discard`](crate::Spool::discard).
    /// - [`Stating`](LocalOperation::Stating) for
    ///   [`probe_root`](crate::MappedRoots::probe_root) and
    ///   [`Listing`](LocalOperation::Listing) for
    ///   [`list_folder`](crate::MappedRoots::list_folder).
    /// - The placement's own six, which are what EP-11's recovery rules are
    ///   about: [`Creating`](LocalOperation::Creating) for
    ///   [`Destination::create`](crate::Destination::create),
    ///   [`Writing`](LocalOperation::Writing) and
    ///   [`Flushing`](LocalOperation::Flushing) for the
    ///   [`ScratchFile`](crate::ScratchFile) it hands back,
    ///   [`Stamping`](LocalOperation::Stamping) and
    ///   [`Renaming`](LocalOperation::Renaming) for the
    ///   [`FlushedFile`](crate::FlushedFile) behind that, and
    ///   [`Removing`](LocalOperation::Removing) for
    ///   [`Destination::remove`](crate::Destination::remove).
    ///
    ///   Four of those six share a counter with the spool's, because there is
    ///   one counter per operation and not one per capability: a case that
    ///   scripts the placement's create counts every spool file the same run
    ///   made on the way there. A case about a fetch alone meets none of them —
    ///   a fetching device spools nothing — and a case about both drives two
    ///   devices, each with a disk of its own.
    /// - [`Reading`](LocalOperation::Reading) for three things at once:
    ///   [`open`](crate::Spool::open),
    ///   [`open_source`](crate::MappedRoots::open_source), and every
    ///   [`SourceReader::read`](crate::SourceReader::read) a run makes. They
    ///   share one counter, so the `nth` is the nth of *whichever comes first* —
    ///   a case that scripts a read partway through a Pack's member stream
    ///   counts the scan's own opens and reads on the way there.
    ///
    /// [`prepare_dir`](crate::Spool::prepare_dir) is deliberately not counted or
    /// scripted: it is one call at the top of a run, and a case that wants the
    /// spool directory to be missing simply never prepares it — which is what a
    /// device with no such directory does to [`create`](crate::Spool::create)
    /// anyway. Neither is [`reach`](crate::Destinations::reach) or
    /// [`look_up`](crate::Destinations::look_up), for a related reason: what a
    /// case wants of a descent is a *shape* rather than a refusal, and
    /// [`plant_other`](Self::plant_other) and
    /// [`write_file`](Self::write_file) are what arrange one.
    ///
    /// The refusal's cause carries
    /// [`io::ErrorKind::Other`](std::io::ErrorKind::Other), because no
    /// operating system reported it: what the case is about is the operation
    /// that failed, never which errno stood behind it.
    pub fn fail_on(&self, operation: LocalOperation, nth: usize) {
        lock(&self.state).fail_on(operation, nth);
    }

    /// Puts a whole file at `path`, making the folders above it.
    ///
    /// What a case arranging a mapped folder writes with. The file carries a
    /// fixed modification and birth time until [`set_mtime`](Self::set_mtime) or
    /// [`set_btime`](Self::set_btime) moves it, so what reaches a record is the
    /// same on every machine.
    pub fn write_file(&self, path: &Path, bytes: &[u8]) {
        lock(&self.state).write_file(path, bytes);
    }

    /// Moves a file's modification time without touching a byte of it.
    ///
    /// What "somebody touched the file" is: EP-10's cheap comparison is length
    /// and modification time, so this is the one gesture that makes a scan look
    /// at a file whose content did not move.
    pub fn set_mtime(&self, path: &Path, seconds: i64) {
        lock(&self.state).set_mtime(path, seconds);
    }

    /// Sets, or clears, what the fake reports as a file's birth time
    /// (spec: FM-9).
    ///
    /// `None` is a filesystem that keeps no creation time — a tmpfs, an older
    /// platform — which is the shape an absent field on a record stands for.
    pub fn set_btime(&self, path: &Path, seconds: Option<i64>) {
        lock(&self.state).set_btime(path, seconds);
    }

    /// Makes a folder, and the folders above it.
    pub fn create_dir(&self, path: &Path) {
        lock(&self.state).prepare_dir(path);
    }

    /// Removes a folder and everything under it.
    pub fn remove_dir_all(&self, path: &Path) {
        lock(&self.state).remove_dir_all(path);
    }

    /// Removes one file.
    pub fn remove_file(&self, path: &Path) {
        lock(&self.state).remove(path);
    }

    /// Plants a name that is neither a file nor a folder.
    ///
    /// What the fake has instead of a symbolic link, so a case can arrange EP-8's
    /// rule — never followed, never given an Entry Path of its own — without a
    /// real filesystem to make a link on.
    pub fn plant_other(&self, path: &Path) {
        lock(&self.state).plant_other(path);
    }

    /// Records what [`probe_root`](crate::MappedRoots::probe_root) answers for
    /// one path (spec: EP-12).
    ///
    /// Every other path answers with one fixed identity, so a case only names
    /// this where the *difference* between two roots is what it is about.
    pub fn set_root_identity(&self, path: &Path, identity: RootIdentity) {
        lock(&self.state).set_root_identity(path, identity);
    }
}
