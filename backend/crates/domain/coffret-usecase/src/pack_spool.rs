use std::path::{Path, PathBuf};

use coffret_format::{
    generate_container_id, generate_container_key, wrap_container_key, ChunkSize, ContainerWriter,
    EncodePlan, EntryPlan,
};
use coffret_model::{ContainerId, ContainerKey, ContainerKind, EntryMetadata, KeyEnvelope};

use crate::device_state::{BatchId, DeviceTime, PendingRow, SpoolState};
use crate::index::Index;
use crate::index_error::IndexError;
use crate::library_keys::LibraryKeys;
use crate::local_io_error::LocalIoError;
use crate::spool::Spool;
use crate::spool_file::{Digests, SpoolFile, WRITE_CHUNK};

/// One new Pack being written into the spool, streaming.
///
/// The one writer both a freeze and a repack write their Packs with: a Pack's
/// pending row, Container ID, Container Key, entry table and ciphertext come
/// about in one order whoever feeds it. What differs between the two is only where each
/// member's plaintext comes from — a file on this device's disk, or an Entry
/// another Pack already holds — and that is the caller's, handed in a buffer
/// at a time through [`write`](Self::write).
///
/// The order is the sync's, and for the same reason: the pending row is
/// recorded before the spool file is created, so a run that dies anywhere in
/// the write — or is killed without an error path at all — leaves a row naming
/// what it may have put on disk (spec: OC-2, OC-3). The row is flipped to
/// [`Spooled`](SpoolState::Spooled) once the Pack is flushed, and the Container
/// Key is wrapped after that, so a wrap failure leaves a complete spool a row
/// already calls `Spooled`.
///
/// Nothing here holds a Pack, or an Entry: the entry table is fixed before the
/// first byte (spec: PK-18), so [`ContainerWriter`] writes the header and the
/// table at once and then takes the content a buffer at a time, handing back
/// ciphertext that goes straight to the spool with both digests folded in as
/// it passes. Every member's bytes are counted and hashed against its plan as
/// they pass, so content that is not what the table promises stops the Pack
/// rather than being committed under it.
pub(crate) struct PackSpool {
    container_id: ContainerId,
    container_key: ContainerKey,
    spool_path: PathBuf,
    file: SpoolFile,
    writer: ContainerWriter,
    /// Drained into the spool after every step, so what the run holds is one
    /// chunk of ciphertext rather than a Pack of it.
    sink: Vec<u8>,
}

/// A Pack whose ciphertext is on the device, ready to be described as a
/// [`SpooledContainer`](crate::spooled_container::SpooledContainer).
pub(crate) struct SpooledPack {
    /// The Pack's new Container ID.
    pub(crate) container_id: ContainerId,
    /// Where its ciphertext waits.
    pub(crate) spool_path: PathBuf,
    /// The entry table its encoder wrote, which is the one its record carries
    /// (spec: CP-11, FM-9).
    pub(crate) entries: Vec<EntryMetadata>,
    /// Its Container Key, sealed under the epoch's key (spec: FM-14).
    pub(crate) envelope: KeyEnvelope,
    /// The digests and the length taken as the ciphertext was written.
    pub(crate) digests: Digests,
}

/// Why a Pack could not be written into the spool.
///
/// Three layers, kept apart so that each flow names them in its own terms: a
/// freeze reads an encoder's refusal as a local file that moved, and a repack
/// reads the same refusal as a carried Entry that was not what its row said.
#[derive(Debug)]
pub(crate) enum PackSpoolError {
    /// The pending row could not be written or flipped (spec: OC-2).
    Index(IndexError),
    /// The spool file could not be created, written or flushed.
    Local(LocalIoError),
    /// The encoder refused the content, or a key could not be drawn or
    /// wrapped.
    Format(coffret_format::Error),
}

impl From<IndexError> for PackSpoolError {
    fn from(error: IndexError) -> Self {
        Self::Index(error)
    }
}

impl From<LocalIoError> for PackSpoolError {
    fn from(error: LocalIoError) -> Self {
        Self::Local(error)
    }
}

impl From<coffret_format::Error> for PackSpoolError {
    fn from(error: coffret_format::Error) -> Self {
        Self::Format(error)
    }
}

impl PackSpool {
    /// Draws a Container ID and Key for a new Pack holding `plans`, records the
    /// pending row naming it, and writes its header and entry table.
    ///
    /// `materializes` is what the row says about where the content came from:
    /// true for a freeze, whose members are files on this device's disk, and
    /// false for a repack, whose members came off Storage — so completing the
    /// row after an interrupted commit marks nothing present that this device
    /// never held (spec: OC-7, EP-10).
    pub(crate) async fn begin(
        index: &dyn Index,
        local: &dyn Spool,
        spool_dir: &Path,
        batch: &BatchId,
        now: DeviceTime,
        plans: &[EntryPlan],
        materializes: bool,
    ) -> Result<Self, PackSpoolError> {
        let container_id = generate_container_id()?;
        let container_key = generate_container_key()?;
        let spool_path = spool_dir.join(format!("{container_id}.spool"));
        index
            .record_pending_row(PendingRow {
                commit_attempted: false,
                materializes,
                container_id,
                spool_path: spool_path.clone(),
                batch: batch.clone(),
                created_at: now,
                state: SpoolState::Spooling,
            })
            .await?;

        let mut file = SpoolFile::create(local, &spool_path).await?;
        let mut sink = Vec::with_capacity(WRITE_CHUNK);
        let writer = ContainerWriter::begin(
            &EncodePlan {
                container_id,
                // Whatever fed it, what comes out is a Pack (spec: PK-15).
                kind: ContainerKind::Pack,
                key: &container_key,
                chunk_size: ChunkSize::DEFAULT,
                entries: plans,
            },
            &mut sink,
        )?;
        file.write(&sink).await?;
        sink.clear();
        Ok(Self {
            container_id,
            container_key,
            spool_path,
            file,
            writer,
            sink,
        })
    }

    /// Feeds the next stretch of the members' plaintext, in table order.
    ///
    /// A refusal of the encoder's comes back as
    /// [`PackSpoolError::Format`] for the caller to name: which member it was
    /// about is the caller's to know.
    pub(crate) async fn write(&mut self, plaintext: &[u8]) -> Result<(), PackSpoolError> {
        self.writer.write(plaintext, &mut self.sink)?;
        // Drained here rather than after the member, so what the run holds stays
        // one buffer whatever one Entry weighs.
        self.file.write(&self.sink).await?;
        self.sink.clear();
        Ok(())
    }

    /// Closes the Pack, flushes it to the device, flips its row to `Spooled`,
    /// and seals its key.
    ///
    /// The entry table comes off the encoder rather than out of the plans a
    /// second time: what the Journal record says the Pack holds is then the
    /// same table the meta section beside it carries, extents included
    /// (spec: CP-11, FM-9).
    pub(crate) async fn finish(
        mut self,
        index: &dyn Index,
        keys: &LibraryKeys,
    ) -> Result<SpooledPack, PackSpoolError> {
        let entries = self.writer.finish(&mut self.sink)?;
        self.file.write(&self.sink).await?;
        let digests = self.file.finish().await?;
        index.mark_spooled(self.container_id).await?;
        let envelope = wrap_container_key(
            keys.container_wrap(),
            &self.container_id,
            &self.container_key,
        )?;
        Ok(SpooledPack {
            container_id: self.container_id,
            spool_path: self.spool_path,
            entries,
            envelope,
            digests,
        })
    }
}
