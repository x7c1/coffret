use std::error;
use std::fmt;

use crate::header::Header;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong encoding or decoding a Container.
///
/// The header-shape variants are raised before any key is touched, so an object
/// that is not a Container v1 at all is distinguishable from one that is but
/// fails to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Fewer bytes than a Container header occupies.
    HeaderTooShort {
        /// Bytes available.
        actual: usize,
    },
    /// The leading bytes are not the Container magic.
    UnknownMagic {
        /// The bytes found where the magic should be.
        actual: [u8; Header::MAGIC_LEN],
    },
    /// The format version byte names a version this build cannot read.
    UnsupportedVersion {
        /// The version byte found.
        actual: u8,
    },
    /// The header's reserved bytes are not zero.
    ReservedNotZero,
    /// The header records a chunk size of zero, which cuts no stream.
    InvalidChunkSize,
    /// The object ends before the lengths its header declares.
    Truncated,
    /// The object carries a header and meta section but no chunks.
    MissingChunks,
    /// An AEAD message failed authentication; none of its plaintext is released.
    AuthenticationFailed,
    /// The meta section is not the CBOR shape this format version defines.
    MalformedMeta {
        /// What the CBOR reader reported.
        detail: String,
    },
    /// The meta section could not be serialized.
    MetaEncodeFailed {
        /// What the CBOR writer reported.
        detail: String,
    },
    /// The meta section declares a schema this build cannot read.
    UnsupportedMetaSchema {
        /// The schema number found.
        schema: u64,
    },
    /// The meta section is larger than its 32-bit header field can record.
    MetaSectionTooLong,
    /// A Container must hold at least one Entry.
    EmptyEntryTable,
    /// The entry table does not tile the plaintext stream from offset zero
    /// without gaps or overlaps.
    EntryTableNotContiguous {
        /// Index of the first entry whose offset does not follow its predecessor.
        index: usize,
    },
    /// The combined size of the entries overflows the plaintext stream layout.
    StreamTooLong,
    /// The decrypted stream is not as long as the meta section says it is.
    PlaintextLengthMismatch {
        /// Length the entry table and `pad_len` imply.
        expected: u64,
        /// Length actually decrypted.
        actual: u64,
    },
    /// The padding tail holds something other than zeros.
    NonZeroPadding,
    /// A decrypted Entry does not hash to the value its metadata records.
    ContentHashMismatch {
        /// Index of the offending entry in the entry table.
        index: usize,
    },
    /// The operating system would not supply random bytes.
    EntropyUnavailable {
        /// What the entropy source reported.
        detail: String,
    },
    /// A value in the meta section is not a valid domain value.
    Model(coffret_model::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderTooShort { actual } => write!(
                f,
                "expected at least {} header bytes, found {actual}",
                Header::LEN
            ),
            Self::UnknownMagic { actual } => {
                write!(f, "unknown magic {actual:?}, not a Container")
            }
            Self::UnsupportedVersion { actual } => {
                write!(f, "unsupported Container format version {actual}")
            }
            Self::ReservedNotZero => f.write_str("reserved header bytes are not zero"),
            Self::InvalidChunkSize => f.write_str("chunk size must be greater than zero"),
            Self::Truncated => f.write_str("object ends before its header's declared lengths"),
            Self::MissingChunks => f.write_str("object carries no chunks"),
            Self::AuthenticationFailed => f.write_str("message failed authentication"),
            Self::MalformedMeta { detail } => write!(f, "malformed meta section: {detail}"),
            Self::MetaEncodeFailed { detail } => {
                write!(f, "could not encode meta section: {detail}")
            }
            Self::UnsupportedMetaSchema { schema } => {
                write!(f, "unsupported meta section schema {schema}")
            }
            Self::MetaSectionTooLong => {
                f.write_str("meta section exceeds the header's length field")
            }
            Self::EmptyEntryTable => f.write_str("a Container must hold at least one Entry"),
            Self::EntryTableNotContiguous { index } => {
                write!(
                    f,
                    "entry {index} does not follow its predecessor in the stream"
                )
            }
            Self::StreamTooLong => f.write_str("entry sizes overflow the plaintext stream"),
            Self::PlaintextLengthMismatch { expected, actual } => {
                write!(f, "expected {expected} plaintext bytes, decrypted {actual}")
            }
            Self::NonZeroPadding => f.write_str("padding tail is not zero-filled"),
            Self::ContentHashMismatch { index } => {
                write!(f, "entry {index} does not match its recorded content hash")
            }
            Self::EntropyUnavailable { detail } => {
                write!(f, "could not draw random bytes: {detail}")
            }
            Self::Model(error) => write!(f, "{error}"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            _ => None,
        }
    }
}

impl From<coffret_model::Error> for Error {
    fn from(error: coffret_model::Error) -> Self {
        Self::Model(error)
    }
}
