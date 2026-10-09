use coffret_format::Error as FormatError;
use coffret_model::{EntryPath, Redacted};
use tracing::warn;

use crate::device_state::DeviceTime;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::opened_piece::OpenedPiece;
use crate::fetch::parcel_read::herald::Herald;
use crate::fetch::placement::{discard_all, Placement};
use crate::index::Index;

/// The Entries one stretch of adjacent parcels holds, placed as the stretch's
/// plaintext passes.
///
/// A parcel is read whole whatever Entry it was read for (spec: PK-16), so the
/// bytes of every Entry inside it pass this way whether anybody asked for them
/// or not. The ones this device would place anyway — mapped, nothing in the
/// way (spec: EP-10, EP-11) — are written out as they pass, which is what lets
/// the rest of a volume be on disk by the time a reader turns the page.
///
/// Each Entry is published the moment its last byte has passed, not when the
/// stretch ends: the page a reader asked for appears as soon as the chunks
/// covering it have arrived, while the rest of its parcel is still on the way
/// (spec: PK-16) — and the caller hears of it then, through the [`Herald`], so
/// a reader is answered without waiting for the parcel's end. Publishing is
/// EP-11's discipline unchanged — scratch, flush, plaintext hash against the
/// catalog, the Entry's own time, rename, marked present — and per Entry, so a
/// placement held against the catalog is all a published file ever needed: the
/// object's own hash is not, because a read of parcels is not a read of every
/// byte the hash is over (spec: PK-22).
///
/// The Entry somebody asked for and the ones that came with it differ in one
/// respect: a refusal about the first fails the read, and a refusal about one
/// of the others is logged and that one is left unplaced, because the caller
/// asked for none of them and a later read of the same parcels — which the
/// device still holds — places it then.
pub(super) struct Spread<'a, 'h> {
    /// The next plaintext position the stretch has not yet delivered.
    position: u64,
    /// Where the stream's padding tail starts (spec: FM-4).
    padding_start: u64,
    /// Placements still waiting for bytes, in stream order.
    pending: Vec<Placement<'a>>,
    /// The Entry the caller asked for.
    asked: EntryPath,
    /// What has been published so far.
    placed: Vec<EntryPath>,
    /// Who is told the moment the Entry asked for is published.
    herald: &'h Herald<'h>,
}

impl<'a, 'h> Spread<'a, 'h> {
    /// A stretch starting at plaintext position `start`, placing `placements`.
    pub(super) fn new(
        start: u64,
        padding_start: u64,
        mut placements: Vec<Placement<'a>>,
        asked: &EntryPath,
        herald: &'h Herald<'h>,
    ) -> Self {
        // The stretch is walked once from front to back, so the placements are
        // held in the order it reaches them.
        placements.sort_by_key(|placement| (placement.start(), placement.end()));
        Self {
            position: start,
            padding_start,
            pending: placements,
            asked: asked.clone(),
            placed: Vec::new(),
            herald,
        }
    }

    /// Takes the next piece of the stretch's plaintext, writes it to every
    /// Entry it belongs to, and publishes every Entry it completes.
    ///
    /// A piece that starts before where the stretch has reached is the same
    /// parcel asked for again whole after an answer that stopped (spec: PK-21):
    /// the bytes already delivered are stepped over, every chunk of the second
    /// answer having authenticated as exactly the chunk the first one held
    /// (spec: FM-7).
    pub(super) async fn absorb(
        &mut self,
        piece: OpenedPiece<'_>,
        index: &dyn Index,
        now: DeviceTime,
    ) -> FetchResult<()> {
        let Some(piece) = piece.from(self.position) else {
            return Ok(());
        };
        assert_eq!(
            piece.start(),
            self.position,
            "the parcels of one stretch are read in order and without a gap",
        );

        // Whatever of this piece falls in the padding tail has to be zero
        // (spec: FM-4).
        if let Some(padding) = piece.overlapping(&(self.padding_start..u64::MAX)) {
            if padding.iter().any(|byte| *byte != 0) {
                return Err(FetchError::Format(FormatError::NonZeroPadding));
            }
        }

        for placement in &mut self.pending {
            if let Some(bytes) = piece.overlapping(&(placement.start()..placement.end())) {
                placement.write(bytes).await?;
            }
        }
        self.position = piece.end();
        self.publish_complete(index, now).await
    }

    /// Verifies and publishes every placement whose last byte has passed.
    async fn publish_complete(&mut self, index: &dyn Index, now: DeviceTime) -> FetchResult<()> {
        while self
            .pending
            .first()
            .is_some_and(|placement| placement.end() <= self.position)
        {
            let mut placement = self.pending.remove(0);
            let asked = placement.path() == &self.asked;
            let published = match placement.verify().await {
                Ok(()) => placement.publish(index, now).await,
                Err(error) => {
                    discard_all(vec![placement]);
                    Err(error)
                }
            };
            match published {
                // The reader waiting for it is answered now, while the rest of
                // the parcel is still to come (spec: PK-16).
                Ok(path) if asked => {
                    self.herald.published(&self.placed);
                    self.placed.push(path);
                }
                Ok(path) => self.placed.push(path),
                Err(error) if asked => return Err(error),
                // One of the Entries that came with the parcel, which nobody
                // asked for: said, and left for a later read of the parcels
                // the device still holds.
                Err(error) => warn!(
                    error = %error.redacted(),
                    "an Entry read alongside the one asked for was not placed",
                ),
            }
        }
        Ok(())
    }

    /// Where the stretch has reached.
    pub(super) fn position(&self) -> u64 {
        self.position
    }

    /// What the stretch published, once every parcel of it has passed.
    ///
    /// An Entry of no bytes is complete before any byte of it passes, and one
    /// standing where the stream ends — after the last byte any parcel carries
    /// — is published here rather than by a piece that never comes.
    pub(super) async fn finish(
        mut self,
        index: &dyn Index,
        now: DeviceTime,
    ) -> FetchResult<Vec<EntryPath>> {
        if let Err(error) = self.publish_complete(index, now).await {
            discard_all(self.pending);
            return Err(error);
        }
        debug_assert!(
            self.pending.is_empty(),
            "every Entry of a stretch lies inside it, so its last byte has passed",
        );
        discard_all(self.pending);
        Ok(self.placed)
    }

    /// Gives up on what the stretch has not published, keeping what it has.
    pub(super) fn abandon(self) -> Vec<EntryPath> {
        discard_all(self.pending);
        self.placed
    }
}
