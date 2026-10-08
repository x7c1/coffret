use coffret_format::{
    ChunkRunReader, ContainerOutline, ContainerWriter, Error as FormatError, Header,
};
use coffret_model::{ContainerKey, ContainerSummary, ContentHash, EntryMetadata};

use crate::read_modify_replace::rebuild_error::RebuildError;
use crate::read_modify_replace::unverified::Unverified;
use crate::spool_file::SpoolFile;

/// Why one attempt at carrying a Container forward stopped.
///
/// Two answers that cost different things: a verdict about the old Container,
/// which costs that Container, and a fault of this device's own, which costs the
/// run.
pub(super) enum Stop {
    /// The old Container is not what the Library says it is.
    Unverified(Unverified),
    /// This device could not write what it was carrying.
    Fault(RebuildError),
}

impl From<Unverified> for Stop {
    fn from(unverified: Unverified) -> Self {
        Self::Unverified(unverified)
    }
}

/// One old Container being decoded as it arrives, and the kept part of its
/// plaintext being written into its replacement as it is decoded.
///
/// The decode is the fetch's — collect the header and the meta section, open
/// them, then feed the chunk sequence through an authenticating reader
/// (spec: FM-2, FM-5, FM-8) — and the encode is the freeze's: a
/// [`ContainerWriter`] whose entry table was fixed before the first byte
/// (spec: PK-18). Between them nothing holds more than one chunk of
/// plaintext, which is what a Pack measured in gigabytes needs (spec: PK-5).
///
/// Every Entry of the old Container is hashed as it passes and held against the
/// hash its row records, the deleted ones as well as the kept ones: PK-10 asks
/// for every Entry to be read and verified, and an old Container with one Entry
/// that does not verify is not one to vouch for any part of (spec: PK-10).
pub(super) struct Carrying<'a> {
    old: &'a ContainerSummary,
    key: &'a ContainerKey,
    /// The old Container's rows as the catalog records them, in stream order.
    table: &'a [EntryMetadata],
    /// Which of those rows the replacement carries.
    keep: &'a [bool],
    writer: Option<ContainerWriter>,
    /// The header and the meta section, until they are complete.
    front: Vec<u8>,
    front_len: Option<usize>,
    chunks: Option<ChunkRunReader>,
    /// One chunk's plaintext, reused between chunks.
    plaintext: Vec<u8>,
    /// Ciphertext of the replacement, drained into the spool after each step.
    sink: Vec<u8>,
    /// How far into the old plaintext stream the decode has got.
    position: u64,
    /// Which row the next plaintext byte belongs to.
    row: usize,
    hasher: blake3::Hasher,
}

impl<'a> Carrying<'a> {
    /// Starts an attempt, the replacement's header and meta section already
    /// written into `sink` by `writer`.
    pub(super) fn new(
        old: &'a ContainerSummary,
        key: &'a ContainerKey,
        table: &'a [EntryMetadata],
        keep: &'a [bool],
        writer: ContainerWriter,
    ) -> Self {
        Self {
            old,
            key,
            table,
            keep,
            writer: Some(writer),
            front: Vec::with_capacity(Header::LEN),
            front_len: None,
            chunks: None,
            plaintext: Vec::new(),
            sink: Vec::new(),
            position: 0,
            row: 0,
            hasher: blake3::Hasher::new(),
        }
    }

    /// Takes the next ciphertext bytes of the old Container.
    pub(super) async fn absorb(
        &mut self,
        ciphertext: &[u8],
        file: &mut SpoolFile,
    ) -> Result<(), Stop> {
        let mut rest = ciphertext;
        while self.chunks.is_none() {
            let needed = self.front_len.unwrap_or(Header::LEN);
            let take = (needed - self.front.len()).min(rest.len());
            self.front.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.front.len() < needed {
                return Ok(());
            }
            match self.front_len {
                None => {
                    let front_len = ContainerOutline::prefix_len(&self.front)
                        .map_err(Unverified::Unopenable)?;
                    self.front_len = Some(usize::try_from(front_len).map_err(|_| {
                        Unverified::Unopenable(FormatError::UnaddressableOnThisBuild {
                            what: "header and meta section",
                            declared: front_len,
                        })
                    })?);
                }
                Some(_) => self.open()?,
            }
        }
        if rest.is_empty() {
            return Ok(());
        }

        let chunks = self.chunks.as_mut().expect("opened just above");
        let mut plaintext = std::mem::take(&mut self.plaintext);
        plaintext.clear();
        let opened = chunks.read(rest, &mut plaintext);
        let carried = match opened {
            Ok(()) => self.carry(&plaintext, file).await,
            Err(error) => Err(Unverified::Unopenable(error).into()),
        };
        self.plaintext = plaintext;
        carried
    }

    /// Opens the meta section and holds it against what the catalog records
    /// (spec: CP-11, FM-9).
    fn open(&mut self) -> Result<(), Stop> {
        let outline =
            ContainerOutline::open(&self.front, self.key).map_err(Unverified::Unopenable)?;
        if outline.container_id() != self.old.id
            || outline.kind() != self.old.kind
            || outline.entries() != self.table
        {
            return Err(Unverified::Disagrees.into());
        }
        self.chunks = Some(ChunkRunReader::begin(
            &outline,
            self.key,
            &outline.all_chunks(),
        ));
        self.plaintext = Vec::with_capacity(
            usize::try_from(outline.chunk_size().get()).unwrap_or(super::TRANSFER_BUFFER),
        );
        Ok(())
    }

    /// Walks one stretch of the old plaintext stream across the rows it covers.
    ///
    /// The rows tile the stream from its start (spec: FM-9), and the meta section
    /// has just been held to the catalog's table, so where one row ends the
    /// next begins; what is past the last row is padding (spec: FM-4) and is
    /// authenticated by the chunk reader and carried nowhere.
    async fn carry(&mut self, plaintext: &[u8], file: &mut SpoolFile) -> Result<(), Stop> {
        let mut rest = plaintext;
        loop {
            self.close_filled_rows()?;
            let Some(row) = self.table.get(self.row) else {
                return Ok(());
            };
            if rest.is_empty() {
                return Ok(());
            }
            let left = row.extent.end() - self.position;
            let take = usize::try_from(left).unwrap_or(usize::MAX).min(rest.len());
            let piece = &rest[..take];
            self.hasher.update(piece);
            if self.keep[self.row] {
                let writer = self.writer.as_mut().expect("held until the attempt closes");
                writer
                    .write(piece, &mut self.sink)
                    .map_err(|error| Stop::Fault(error.into()))?;
                file.write(&self.sink)
                    .await
                    .map_err(|error| Stop::Fault(error.into()))?;
                self.sink.clear();
            }
            self.position += take as u64;
            rest = &rest[take..];
        }
    }

    /// Closes every row whose bytes have all passed, holding each one's hash to
    /// what the row records (spec: CP-11, PK-10).
    ///
    /// A run of them can close at once, because a row of length zero is full
    /// the moment it starts.
    fn close_filled_rows(&mut self) -> Result<(), Stop> {
        while let Some(row) = self.table.get(self.row) {
            if self.position != row.extent.end() {
                return Ok(());
            }
            let hash = ContentHash::from_bytes(*self.hasher.finalize().as_bytes());
            if hash != row.hash {
                return Err(Unverified::EntryMismatch {
                    path: row.path.clone(),
                }
                .into());
            }
            self.hasher.reset();
            self.row += 1;
        }
        Ok(())
    }

    /// Closes the attempt once the whole object has arrived: the chunk
    /// sequence has to be whole and every row verified, and the replacement's
    /// last chunk goes into the spool.
    ///
    /// What comes back is the replacement's entry table as its encoder wrote
    /// it, for the record to carry (spec: CP-11, FM-9).
    pub(super) async fn finish(mut self, file: &mut SpoolFile) -> Result<Vec<EntryMetadata>, Stop> {
        let Some(chunks) = self.chunks.take() else {
            // The object ended inside its own header or meta section.
            return Err(Unverified::Unopenable(FormatError::Truncated).into());
        };
        chunks.finish().map_err(Unverified::Unopenable)?;
        self.close_filled_rows()?;
        if self.row < self.table.len() {
            // A stream that ended before its last row: the layout pads, so a
            // whole chunk sequence always covers every row (spec: FM-4).
            return Err(Unverified::Unopenable(FormatError::Truncated).into());
        }
        let writer = self.writer.take().expect("held until the attempt closes");
        let entries = writer
            .finish(&mut self.sink)
            .map_err(|error| Stop::Fault(error.into()))?;
        file.write(&self.sink)
            .await
            .map_err(|error| Stop::Fault(error.into()))?;
        self.sink.clear();
        Ok(entries)
    }
}
