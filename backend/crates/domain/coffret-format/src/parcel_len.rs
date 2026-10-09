use std::num::NonZeroU64;

/// How much of a Container's chunk sequence one parcel is meant to hold — the
/// `S` of PK-19.
///
/// A parcel is the unit every read of a Container's chunks asks for and the
/// granularity the Storage provider observes (spec: PK-16, PK-20), so this is
/// the value that decides both how long the first page of an unfetched book
/// takes to appear and how much a provider can tell about which pages were
/// read. It is a constant of the spec's register rather than of any one
/// Container: no header or meta section records it, and every reader of every
/// Container divides the chunk sequence the same way (spec: PK-19).
///
/// It is a type rather than a bare number for the one reason a caller passes
/// it at all: the register's value is provisional and set by measurement, and
/// a test that wants a Pack of several parcels out of a few megabytes of
/// content lowers it rather than writing a gigabyte. Every production caller
/// passes [`PARCEL_LEN`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParcelLen(NonZeroU64);

/// The parcel length the spec's register sets: provisionally 32 MiB
/// (spec: PK-19).
pub const PARCEL_LEN: ParcelLen = ParcelLen(match NonZeroU64::new(32 * 1024 * 1024) {
    Some(len) => len,
    None => panic!("the register's parcel length is not zero"),
});

impl ParcelLen {
    /// A parcel length of `bytes`.
    ///
    /// Zero is unrepresentable rather than refused: a parcel of no bytes is
    /// still one chunk long (spec: PK-19), so the type takes a length that
    /// already says it is not zero and there is no error to answer with.
    pub const fn new(bytes: NonZeroU64) -> Self {
        Self(bytes)
    }

    /// The length in bytes.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl Default for ParcelLen {
    fn default() -> Self {
        PARCEL_LEN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // PK-19: `S` is provisionally 32 MiB.
    #[test]
    fn the_register_value_is_thirty_two_mebibytes() {
        assert_eq!(PARCEL_LEN.get(), 32 * 1024 * 1024);
        assert_eq!(ParcelLen::default(), PARCEL_LEN);
    }
}
