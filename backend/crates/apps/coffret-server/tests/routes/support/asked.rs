use std::ops::Range;

/// One call a case may count, in the order it was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Asked {
    /// One page of the listing.
    Listed,
    /// A whole object: a control object, a Keyring replica among them.
    Whole,
    /// A range of one.
    Range(Range<u64>),
}

impl Asked {
    /// A read of an object's header, the first of its front's two
    /// (spec: FM-2).
    pub fn is_header(&self) -> bool {
        matches!(self, Self::Range(range) if range.start == 0 && range.end == HEADER_LEN)
    }

    /// A read of the meta section behind it, the second.
    pub fn is_meta(&self) -> bool {
        matches!(self, Self::Range(range) if range.start == HEADER_LEN)
    }

    /// A read of a parcel: any other range of a Container (spec: PK-16,
    /// PK-19).
    pub fn is_parcel(&self) -> bool {
        matches!(self, Self::Range(_)) && !self.is_header() && !self.is_meta()
    }
}

/// How long a Container's header is (spec: FM-2).
const HEADER_LEN: u64 = 32;
