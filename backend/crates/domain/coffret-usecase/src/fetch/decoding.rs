use coffret_format::{ChunkRunReader, ContainerOutline, Error as FormatError, Header};
use coffret_model::{ContainerId, ContainerKey};

use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::placement::Placement;
use crate::fetch::scatter::Scatter;
use crate::fetch::target::Target;
use crate::fetch::TRANSFER_BUFFER;

/// A Container being decoded out of a stream that is still arriving.
///
/// A Container says where everything in it is before any of it arrives: the
/// header gives the meta section's length, and the meta section gives the entry
/// table and the shape of the chunk sequence (spec: FM-2, FM-5, FM-9). So the
/// decode has two states — collecting that front, and then feeding chunks — and
/// the second one begins the moment the first has enough bytes, in the middle of
/// whatever piece of the transfer delivered them.
///
/// The first of those states is the one an untrusted number steers, so it is
/// bounded: the length the header declares is refused as the header is parsed if
/// it is past what a meta section may be, and the front stops growing there.
pub(super) struct Decoding<'k, 'a> {
    container_id: ContainerId,
    key: &'k ContainerKey,
    wanted: &'a [Target],
    /// The header and the meta section, until they are complete.
    front: Vec<u8>,
    /// How long the front is, once the header has said (spec: FM-2).
    front_len: Option<usize>,
    /// The chunk sequence, once the meta section has opened.
    chunks: Option<ChunkRunReader>,
    scatter: Option<Scatter<'a>>,
    /// One chunk's plaintext, reused between chunks.
    plaintext: Vec<u8>,
}

impl<'k, 'a> Decoding<'k, 'a> {
    pub(super) fn new(
        container_id: ContainerId,
        key: &'k ContainerKey,
        wanted: &'a [Target],
    ) -> Self {
        Self {
            container_id,
            key,
            wanted,
            front: Vec::with_capacity(Header::LEN),
            front_len: None,
            chunks: None,
            scatter: None,
            plaintext: Vec::new(),
        }
    }

    /// Takes the next ciphertext bytes off the object.
    pub(super) async fn absorb(&mut self, ciphertext: &[u8]) -> FetchResult<()> {
        let mut rest = ciphertext;
        while self.chunks.is_none() {
            let needed = self.front_len.unwrap_or(Header::LEN);
            let take = (needed - self.front.len()).min(rest.len());
            self.front.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.front.len() < needed {
                // The front is not all here yet, and the object has more to say.
                return Ok(());
            }
            match self.front_len {
                // The header is here and says how much meta section follows it.
                None => {
                    let front_len = ContainerOutline::prefix_len(&self.front)?;
                    self.front_len =
                        Some(usize::try_from(front_len).map_err(|_| FormatError::Truncated)?);
                }
                Some(_) => self.open().await?,
            }
        }

        if rest.is_empty() {
            return Ok(());
        }
        let chunks = self.chunks.as_mut().expect("opened just above");
        let mut plaintext = std::mem::take(&mut self.plaintext);
        plaintext.clear();
        let opened = chunks.read(rest, &mut plaintext);
        let scattered = match opened {
            Ok(()) => {
                self.scatter
                    .as_mut()
                    .expect("a scatter is opened with the chunk reader")
                    .absorb(&plaintext)
                    .await
            }
            Err(error) => Err(FetchError::Format(error)),
        };
        self.plaintext = plaintext;
        scattered
    }

    /// Opens the meta section and makes ready for the chunk sequence.
    async fn open(&mut self) -> FetchResult<()> {
        let outline = ContainerOutline::open(&self.front, self.key)?;
        let run = outline.all_chunks();
        let scatter = Scatter::open(&outline, self.container_id, self.wanted).await?;
        self.chunks = Some(ChunkRunReader::begin(&outline, self.key, &run));
        self.scatter = Some(scatter);
        // A chunk's plaintext is the largest single buffer a fetch holds, and
        // the object itself has just said how large that is (spec: FM-6).
        self.plaintext = Vec::with_capacity(
            usize::try_from(outline.chunk_size().get()).unwrap_or(TRANSFER_BUFFER),
        );
        Ok(())
    }

    /// Closes the decode: the chunk sequence has to have arrived whole, and
    /// every Entry has to be what the catalog names.
    pub(super) async fn verify(self) -> FetchResult<Vec<Placement<'a>>> {
        let (Some(chunks), Some(scatter)) = (self.chunks, self.scatter) else {
            // The object ended inside its own header or meta section, so there
            // was never a chunk sequence to read.
            return Err(FetchError::Format(FormatError::Truncated));
        };
        if let Err(error) = chunks.finish() {
            scatter.discard().await;
            return Err(FetchError::Format(error));
        }
        scatter.verify().await
    }

    /// Removes whatever temporary files this decode had made.
    pub(super) async fn discard(self) {
        if let Some(scatter) = self.scatter {
            scatter.discard().await;
        }
    }
}
