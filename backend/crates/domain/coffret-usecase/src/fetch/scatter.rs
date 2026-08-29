use coffret_format::{ContainerOutline, Error as FormatError};
use coffret_model::ContainerId;

use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::placement::{discard_all, Placement};
use crate::fetch::target::Target;

/// Routes a Container's plaintext stream past the Entries a run wants out of it.
///
/// The stream is every Entry's plaintext back to back in entry-table order,
/// followed by the zero padding the meta section records (spec: FM-4, FM-9). A
/// fetch wants some of those Entries and none of the rest, so what walks past
/// here is written to a temporary file where it belongs to a wanted Entry and
/// dropped where it does not — which is what keeps the memory a fetch spends
/// down to the piece of stream in hand, however large the Pack is.
///
/// The padding tail is verified rather than merely skipped. FM-4 makes zero
/// padding a rule a decoder checks, and this is the decoder on the download
/// path, so it makes the same accusation in the same words the whole-buffer
/// [`decode`](coffret_format::decode()) would.
pub(super) struct Scatter<'a> {
    /// Where in the plaintext stream the next byte stands.
    position: u64,
    /// Where the padding tail starts (spec: FM-4).
    padding_start: u64,
    /// The wanted Entries, in stream order.
    placements: Vec<Placement<'a>>,
    /// The first placement the stream has not yet walked past.
    next: usize,
}

impl<'a> Scatter<'a> {
    /// Opens a temporary file for every wanted Entry of one Container.
    ///
    /// Where each Entry's bytes are is the Container's own account of itself —
    /// the entry table inside the object — rather than the catalog's. An Entry
    /// the catalog places in this Container and the table does not hold means
    /// the two describe different states of the Library, and nothing is placed
    /// for it (spec: CP-11).
    pub(super) async fn open(
        outline: &ContainerOutline,
        container_id: ContainerId,
        wanted: &'a [Target],
    ) -> FetchResult<Self> {
        let mut placements: Vec<Placement<'a>> = Vec::with_capacity(wanted.len());
        for target in wanted {
            let entry = match outline.entry_at(target.path()) {
                Some(entry) => entry.clone(),
                None => {
                    discard_all(placements).await;
                    return Err(FetchError::EntryMissing {
                        container_id,
                        path: target.path().clone(),
                    });
                }
            };
            match Placement::open(target, entry).await {
                Ok(placement) => placements.push(placement),
                Err(error) => {
                    discard_all(placements).await;
                    return Err(error);
                }
            }
        }
        // The stream is walked once from front to back, so the placements are
        // held in the order it reaches them rather than in the order the run
        // selected them.
        placements.sort_by_key(Placement::start);

        Ok(Self {
            position: 0,
            padding_start: outline.plaintext_len() - outline.pad_len(),
            placements,
            next: 0,
        })
    }

    /// Takes the next piece of the plaintext stream.
    pub(super) async fn absorb(&mut self, plaintext: &[u8]) -> FetchResult<()> {
        // Whatever of this piece falls in the padding tail has to be zero
        // (spec: FM-4).
        let end = self.position + plaintext.len() as u64;
        if end > self.padding_start {
            let from = usize::try_from(self.padding_start.saturating_sub(self.position))
                .unwrap_or(usize::MAX)
                .min(plaintext.len());
            if plaintext[from..].iter().any(|byte| *byte != 0) {
                return Err(FetchError::Format(FormatError::NonZeroPadding));
            }
        }

        let mut rest = plaintext;
        while !rest.is_empty() {
            // Entries the stream has already carried past, empty ones included.
            while self
                .placements
                .get(self.next)
                .is_some_and(|placement| placement.end() <= self.position)
            {
                self.next += 1;
            }

            let taken = match self.placements.get_mut(self.next) {
                // Inside a wanted Entry: as much of it as this piece holds.
                Some(placement) if placement.start() <= self.position => {
                    let take = usize::try_from(placement.end() - self.position)
                        .unwrap_or(usize::MAX)
                        .min(rest.len());
                    placement.write(&rest[..take]).await?;
                    take
                }
                // In front of the next wanted Entry: bytes of an Entry this run
                // did not ask for, or of one it already has.
                Some(placement) => usize::try_from(placement.start() - self.position)
                    .unwrap_or(usize::MAX)
                    .min(rest.len()),
                // Past the last wanted Entry: the rest of the stream and its
                // padding tail.
                None => rest.len(),
            };
            self.position += taken as u64;
            rest = &rest[taken..];
        }
        Ok(())
    }

    /// Closes every temporary file and holds each against the catalog.
    ///
    /// Nothing is renamed here: what comes back is a Container's worth of files
    /// that are verified and still invisible, which is what lets the object's own
    /// hash be the last word before any of them appears (spec: FM-15, EP-11).
    pub(super) async fn verify(mut self) -> FetchResult<Vec<Placement<'a>>> {
        for index in 0..self.placements.len() {
            if let Err(error) = self.placements[index].verify().await {
                discard_all(self.placements).await;
                return Err(error);
            }
        }
        Ok(self.placements)
    }

    /// Removes every temporary file, the fetch having come to nothing.
    pub(super) async fn discard(self) {
        discard_all(self.placements).await;
    }
}
