use std::path::Path;

use coffret_format::{ChunkRun, ChunkRunReader, ContainerOutline};
use coffret_model::{ContainerKey, ObjectRef, Redacted};
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tracing::warn;

use crate::answer_length::AnswerLength;
use crate::byte_stream::ByteStream;
use crate::device_state::{DeviceTime, HeldParcel};
use crate::error::{Error, Result};
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::kept_parcels::KeptParcels;
use crate::fetch::opened_piece::OpenedPiece;
use crate::fetch::parcel_read::front::within_object;
use crate::fetch::parcel_read::let_go::let_go;
use crate::fetch::parcel_read::spread::Spread;
use crate::fetch::unheld_reason::UnheldReason;
use crate::fetch::TRANSFER_BUFFER;
use crate::index::Index;
use crate::local_operation::LocalOperation;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// What one Container's parcels are read against, from Storage or from this
/// device: the object and its outline, the key its chunks open with, the
/// catalog and the clock a placement is recorded with, and where kept parcels
/// live.
pub(super) struct Source<'r> {
    pub(super) store: &'r dyn ObjectStore,
    pub(super) retry: &'r RetryPolicy,
    pub(super) object: &'r ObjectRef,
    pub(super) object_len: u64,
    pub(super) outline: &'r ContainerOutline,
    pub(super) key: &'r ContainerKey,
    pub(super) index: &'r dyn Index,
    pub(super) now: DeviceTime,
    pub(super) kept: &'r KeptParcels<'r>,
}

impl Source<'_> {
    /// Opens a parcel this device holds, handing its plaintext to `spread`.
    ///
    /// `None` is the parcel read whole and every chunk of it authenticated
    /// (spec: FM-5, FM-7, FM-8). A file that is not there, or whose ciphertext
    /// does not authenticate as exactly that parcel of this Container — or is
    /// short or long — is answered with why: the parcel is then not held, and
    /// the caller asks Storage for it again (spec: PK-21). Whatever of it
    /// authenticated before the failure has already gone to `spread`, which
    /// steps over those bytes when the parcel comes again.
    ///
    /// A disk that would not read the file at all is not one of those: it is
    /// the device's own refusal, and fails the read.
    pub(super) async fn read_held(
        &self,
        parcel: &HeldParcel,
        run: &ChunkRun,
        spread: &mut Spread<'_, '_>,
    ) -> FetchResult<Option<UnheldReason>> {
        let Some(mut reader) = self.kept.files.reader(&parcel.path).await? else {
            return Ok(Some(UnheldReason::Missing));
        };
        let mut chunks = ChunkRunReader::begin(self.outline, self.key, run);
        let mut buffer = vec![0u8; TRANSFER_BUFFER];
        let mut plaintext = Vec::new();
        let mut position = run.plaintext_start();
        loop {
            let read = reader
                .read(&mut buffer)
                .await
                .map_err(|cause| FetchError::Io {
                    operation: LocalOperation::Reading,
                    path: parcel.path.clone(),
                    cause,
                })?;
            if read == 0 {
                break;
            }
            plaintext.clear();
            if let Err(cause) = chunks.read(&buffer[..read], &mut plaintext) {
                return Ok(Some(UnheldReason::Unauthenticated { cause }));
            }
            let piece = OpenedPiece::at(position, &plaintext)?;
            position = piece.end();
            spread.absorb(piece, self.index, self.now).await?;
        }
        if let Err(cause) = chunks.finish() {
            return Ok(Some(UnheldReason::Unauthenticated { cause }));
        }
        Ok(None)
    }

    /// Asks Storage for one parcel, keeps it, and hands its plaintext to
    /// `spread` as it arrives.
    ///
    /// Exactly the parcel's ciphertext range and never less (spec: PK-16): the
    /// Entry the caller wanted may end long before the parcel does, and is
    /// published by `spread` the moment its chunks have passed, but the read
    /// goes on to the parcel's end. The row is recorded before the file is
    /// created, the order a pending row keeps with its spool (spec: OC-2), and
    /// a parcel that did not arrive whole is let go — file and row — rather
    /// than held, so it is asked for again whole by whoever wants it next
    /// (spec: PK-21).
    ///
    /// The retry policy may ask again after an answer that stopped, and it asks
    /// for the whole parcel again rather than for the rest of it (spec: PK-21);
    /// what `spread` already placed out of the first answer stays placed.
    pub(super) async fn fetch(
        &self,
        index_in_container: u64,
        run: &ChunkRun,
        spread: &mut Spread<'_, '_>,
    ) -> FetchResult<()> {
        let parcel_bytes = run.ciphertext();
        within_object(&parcel_bytes, self.object_len)?;
        let container_id = self.outline.container_id();
        let parcel = HeldParcel {
            container_id,
            index: index_in_container,
            plaintext: run.plaintext(),
            path: self.kept.path_of(container_id, index_in_container),
        };
        self.kept.files.prepare_dir(self.kept.dir).await?;
        self.index.hold_parcel(parcel.clone()).await?;

        let spread = Mutex::new(spread);
        let fetched = self
            .retry
            .run("get", || {
                let parcel_bytes = parcel_bytes.clone();
                let spread = &spread;
                let path = parcel.path.as_path();
                async move {
                    let stream = self.store.get(self.object, Some(parcel_bytes)).await?;
                    let mut spread = spread.lock().await;
                    self.keep(stream, path, run, &mut spread).await
                }
            })
            .await;
        let failed = match fetched {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(refused)) => refused,
            Err(storage) => FetchError::Storage(storage),
        };
        // Not held: the row and whatever the file came to go together, and a
        // refusal to let go is said after the refusal that caused it rather
        // than in its place.
        if let Err(error) = let_go(self.index, self.kept, &parcel).await {
            warn!(
                error = %error.redacted(),
                "a parcel that did not arrive whole could not be let go",
            );
        }
        Err(failed)
    }

    /// One attempt: write the parcel's ciphertext to its file and its
    /// plaintext to `spread`, as it arrives.
    ///
    /// The two error channels are the two answers every Container read draws.
    /// The outer one is Storage's — a transfer that failed or came up short,
    /// which the policy may attempt again — and the inner one is a verdict about
    /// the Library or this device, which no later attempt would change. An
    /// answer of another length than the parcel is held to the parcel's own
    /// length here, so it lands on the outer side however the chunk decoder
    /// would have called it.
    async fn keep(
        &self,
        stream: ByteStream,
        path: &Path,
        run: &ChunkRun,
        spread: &mut Spread<'_, '_>,
    ) -> Result<FetchResult<()>> {
        let parcel_bytes = run.ciphertext();
        let mut writer = match self.kept.files.create(path).await {
            Ok(writer) => writer,
            Err(refused) => return Ok(Err(refused.into())),
        };
        let mut length = AnswerLength::new(stream.len(), parcel_bytes.end - parcel_bytes.start);
        let mut reader = stream.into_reader();
        let mut buffer = vec![0u8; TRANSFER_BUFFER];
        let mut chunks = ChunkRunReader::begin(self.outline, self.key, run);
        let mut plaintext = Vec::new();
        let mut position = run.plaintext_start();

        loop {
            let read = reader.read(&mut buffer).await.map_err(Error::from)?;
            if read == 0 {
                break;
            }
            length.count(read)?;
            if let Err(refused) = writer.write(&buffer[..read]).await {
                return Ok(Err(refused.into()));
            }
            plaintext.clear();
            if let Err(error) = chunks.read(&buffer[..read], &mut plaintext) {
                return Ok(Err(FetchError::Format(error)));
            }
            let piece = match OpenedPiece::at(position, &plaintext) {
                Ok(piece) => piece,
                Err(error) => return Ok(Err(error)),
            };
            position = piece.end();
            if let Err(error) = spread.absorb(piece, self.index, self.now).await {
                return Ok(Err(error));
            }
        }
        length.finish()?;

        // Every byte of the parcel has arrived and none past it, so what the
        // decoder could still refuse here is the parcel itself — the Library's.
        if let Err(error) = chunks.finish() {
            return Ok(Err(FetchError::Format(error)));
        }
        if let Err(refused) = writer.finish().await {
            return Ok(Err(refused.into()));
        }
        Ok(Ok(()))
    }
}
