use coffret_model::ContainerKind;

use crate::aead::TAG_LEN;
use crate::entry_plan::EntryPlan;
use crate::error::{Error, Result};
use crate::header::Header;
use crate::meta;
use crate::padme;

#[cfg(test)]
mod tests;

/// How large a Container is before anything is padded (spec: PK-6).
///
/// The pack policy cuts Entries into Packs around a size target, and the target
/// applies to this: the Entry contents, the canonical metadata that describes
/// them, and the framing they sit in. Authentication tags and Padmé padding come
/// after and can carry the stored ciphertext somewhat past the target, which is
/// why the target is a target and not a maximum.
///
/// It lives here rather than in the policy layer because it is framing
/// knowledge: how many bytes an entry table spends on one more row is the format
/// crate's answer, and a segmentation that re-derived it would be a second
/// reading of FM-9 with nothing holding the two together.
///
/// It accumulates rather than being computed from a slice, because that is how
/// segmentation asks: append the next Entry while the result stays at or below
/// the target, and close the Pack when it would not (spec: PK-3). Each step
/// costs one small serialization instead of one of the whole table.
///
/// ```
/// use coffret_format::{ContainerFootprint, EntryPlan};
/// use coffret_model::{ContainerKind, ContentHash, EntryPath, Mtime};
///
/// # fn main() -> coffret_format::Result<()> {
/// let entry = EntryPlan::new(
///     EntryPath::nfc("albums/spring.jpg"),
///     Mtime::from_unix_seconds(1_700_000_000),
///     4096,
///     ContentHash::from_bytes([0x11; ContentHash::BYTE_LEN]),
/// );
///
/// let empty = ContainerFootprint::empty(ContainerKind::Pack)?;
/// let one = empty.extended(&entry)?;
/// assert!(one.bytes() > empty.bytes() + entry.size);
/// assert_eq!(one.entries(), 1);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerFootprint {
    kind: ContainerKind,
    entries: usize,
    /// What the entry table's rows come to, in CBOR bytes.
    table_bytes: u64,
    /// What the Entries themselves come to.
    content_bytes: u64,
    /// What the meta section's plaintext comes to, before its padding.
    meta_bytes: u64,
    bytes: u64,
}

impl ContainerFootprint {
    /// A Container of this kind holding nothing yet.
    ///
    /// Not a Container that could be written — an empty entry table is refused
    /// (spec: FM-10) — but the point a segmentation starts each Pack from.
    pub fn empty(kind: ContainerKind) -> Result<Self> {
        Self {
            kind,
            entries: 0,
            table_bytes: 0,
            content_bytes: 0,
            meta_bytes: 0,
            bytes: 0,
        }
        .settled()
    }

    /// The footprint of a Container holding exactly these Entries, in order.
    pub fn of(kind: ContainerKind, entries: &[EntryPlan]) -> Result<Self> {
        entries
            .iter()
            .try_fold(Self::empty(kind)?, |footprint, entry| {
                footprint.extended(entry)
            })
    }

    /// The footprint this becomes with one more Entry appended.
    ///
    /// The Entry lands after everything already here, which is what fixes its
    /// offset — and the offset is part of what the entry table spends bytes on,
    /// so appending the same Entry to a fuller Container costs marginally more.
    pub fn extended(&self, entry: &EntryPlan) -> Result<Self> {
        let table_bytes = self
            .table_bytes
            .checked_add(meta::entry_len(&entry.to_metadata(self.content_bytes))?)
            .ok_or(Error::StreamTooLong)?;
        let content_bytes = self
            .content_bytes
            .checked_add(entry.size)
            .ok_or(Error::StreamTooLong)?;
        Self {
            kind: self.kind,
            entries: self.entries + 1,
            table_bytes,
            content_bytes,
            meta_bytes: 0,
            bytes: 0,
        }
        .settled()
    }

    /// The pre-padding footprint in bytes (spec: PK-6).
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    /// How many Entries it counts.
    pub const fn entries(&self) -> usize {
        self.entries
    }

    /// What the Entries themselves come to, without metadata or framing.
    pub const fn content_bytes(&self) -> u64 {
        self.content_bytes
    }

    /// What the header will declare for this Container's meta section: the
    /// section carried to its Padmé bucket, with its tag (spec: FM-2, FM-9).
    ///
    /// The one number [`Header::MAX_META_LEN`] is about, so it is what a caller
    /// deciding whether one more Entry still leaves a readable Container asks —
    /// segmentation (spec: PK-3) does, closing a Pack before its entry table
    /// could reach the ceiling rather than laying out a Container that would be
    /// refused. Closing on the table is a reason of this build's own: the size
    /// target PK-3 cuts on counts the table in but does not bound it (spec:
    /// PK-6).
    pub fn meta_len(&self) -> u64 {
        padme::padded_len(self.meta_bytes) + TAG_LEN as u64
    }

    /// Works out the total, which is the only part that cannot be summed.
    ///
    /// The meta map's own fields include `pad_len`, and the padding follows from
    /// the stream length — so how many bytes the map spends on describing itself
    /// moves as the Container grows.
    fn settled(mut self) -> Result<Self> {
        let pad_len = padme::padded_len(self.content_bytes) - self.content_bytes;
        let meta_bytes = meta::envelope_len(self.kind, pad_len, self.entries)?
            .checked_add(self.table_bytes)
            .ok_or(Error::StreamTooLong)?;
        self.meta_bytes = meta_bytes;
        self.bytes = (Header::LEN as u64)
            .checked_add(meta_bytes)
            .and_then(|framed| framed.checked_add(self.content_bytes))
            .ok_or(Error::StreamTooLong)?;
        Ok(self)
    }
}
