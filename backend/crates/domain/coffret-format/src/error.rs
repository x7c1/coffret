use std::error;
use std::fmt;

use coffret_model::{ControlObjectKind, ControlObjectName};

use crate::control::ControlHeader;
use crate::header::Header;
use crate::purpose::Purpose;
use crate::stored_master_key::StoredMasterKey;
use crate::token_cache::MAGIC_LEN as TOKEN_CACHE_MAGIC_LEN;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong writing or reading a coffret object.
///
/// The header-shape variants are raised before any key is touched, so an object
/// that is not a Container v1 — or not a control object v1 — at all is
/// distinguishable from one that is but fails to open.
#[derive(Debug, Clone)]
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
    /// The meta section holds something other than zeros after its CBOR map.
    NonZeroMetaPadding,
    /// A decrypted Entry does not hash to the value its metadata records.
    ContentHashMismatch {
        /// Index of the offending entry in the entry table.
        index: usize,
    },
    /// Fewer bytes than a control-object header occupies.
    ControlHeaderTooShort {
        /// Bytes available.
        actual: usize,
    },
    /// The leading bytes are not the control-object magic.
    UnknownControlMagic {
        /// The bytes found where the magic should be.
        actual: [u8; ControlHeader::MAGIC_LEN],
    },
    /// The format version byte names a control-object version this build cannot
    /// read.
    UnsupportedControlVersion {
        /// The version byte found.
        actual: u8,
    },
    /// The kind byte names no control-object kind this build knows.
    UnknownControlObjectKind {
        /// The kind byte found.
        actual: u8,
    },
    /// The object carries a control-object header but no payload.
    MissingControlPayload,
    /// A key derived for one purpose was handed to another purpose's message.
    WrongPurposeKey {
        /// The purpose the operation needs.
        expected: Purpose,
        /// The purpose the key was derived for.
        actual: Purpose,
    },
    /// A control object's name does not admit the kind its header declares.
    ///
    /// The name says what an object is stored *for* and the header says what it
    /// *is*; FM-12's admission table is the whole of the relation between them,
    /// and a pairing outside it means the name did not lead to the object it
    /// promised.
    ControlObjectKindNotAdmitted {
        /// The name the object was presented under, as the value it stands for
        /// rather than as text, so that a caller can ask which role was claimed
        /// without parsing the name again.
        name: ControlObjectName,
        /// The kind its header declares.
        kind: ControlObjectKind,
    },
    /// A control object's name disagrees with the header inside it.
    ObjectNameMismatch {
        /// Which of generation or replica position disagrees.
        field: &'static str,
    },
    /// A control-object payload is not the CBOR shape this format version
    /// defines.
    MalformedControlPayload {
        /// What the CBOR reader reported.
        detail: String,
    },
    /// A control-object payload is CBOR but not the map the format calls for.
    ControlPayloadNotAMap,
    /// A control-object payload plaintext holds something other than zeros
    /// after its CBOR map.
    NonZeroControlPadding,
    /// A control-object payload plaintext is not the length FM-11's padding
    /// rule gives the map it carries.
    ControlPaddingLengthMismatch {
        /// The Padmé bucket the map that was read belongs in.
        expected: u64,
        /// The length the plaintext actually is.
        actual: u64,
    },
    /// A control-object payload does not say which Master Key epoch wrote it.
    MissingMasterKeyEpoch,
    /// A control-object payload could not be serialized.
    ControlPayloadEncodeFailed {
        /// What the CBOR writer reported.
        detail: String,
    },
    /// A control-object payload carried to its Padmé bucket is longer than this
    /// platform can address.
    ControlPayloadTooLong {
        /// The padded length FM-11's rule calls for.
        padded: u64,
    },
    /// The leading bytes are not the stored Master Key magic.
    UnknownStoredMasterKeyMagic {
        /// The bytes found where the magic should be.
        actual: [u8; StoredMasterKey::MAGIC_LEN],
    },
    /// The version byte names a stored Master Key form this build cannot read.
    UnsupportedStoredMasterKeyVersion {
        /// The version byte found.
        actual: u8,
    },
    /// The stored Master Key is not the length its own header declares: it ends
    /// early, or bytes follow the wrapped key.
    StoredMasterKeyLengthMismatch,
    /// The leading bytes are not the sealed token cache magic.
    UnknownTokenCacheMagic {
        /// The bytes found where the magic should be.
        actual: [u8; TOKEN_CACHE_MAGIC_LEN],
    },
    /// The version byte names a token cache form this build cannot read.
    UnsupportedTokenCacheVersion {
        /// The version byte found.
        actual: u8,
    },
    /// The token cache ends before the form's fixed part and one tag.
    TokenCacheTooShort {
        /// Bytes available.
        actual: usize,
    },
    /// The recorded Argon2id parameters are not ones Argon2id accepts.
    InvalidArgon2Params {
        /// What the Argon2id implementation reported.
        detail: String,
    },
    /// Deriving the Passphrase-based protection key failed.
    PassphraseDerivationFailed {
        /// What the Argon2id implementation reported.
        detail: String,
    },
    /// The operating system would not supply random bytes.
    EntropyUnavailable {
        /// What the entropy source reported.
        detail: String,
    },
    /// A value in the meta section or a control object is not a valid domain
    /// value.
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
            Self::NonZeroMetaPadding => f.write_str("meta section padding is not zero-filled"),
            Self::ContentHashMismatch { index } => {
                write!(f, "entry {index} does not match its recorded content hash")
            }
            Self::ControlHeaderTooShort { actual } => write!(
                f,
                "expected at least {} control-object header bytes, found {actual}",
                ControlHeader::LEN
            ),
            Self::UnknownControlMagic { actual } => {
                write!(f, "unknown magic {actual:?}, not a control object")
            }
            Self::UnsupportedControlVersion { actual } => {
                write!(f, "unsupported control-object format version {actual}")
            }
            Self::UnknownControlObjectKind { actual } => {
                write!(f, "unknown control-object kind {actual:#04x}")
            }
            Self::MissingControlPayload => f.write_str("control object carries no payload"),
            Self::WrongPurposeKey { expected, actual } => write!(
                f,
                "this message needs the {expected} key, not the {actual} key"
            ),
            Self::ControlObjectKindNotAdmitted { name, kind } => {
                write!(f, "{name} admits no control object of kind {kind:?}")
            }
            Self::ObjectNameMismatch { field } => {
                write!(f, "the object name and its header disagree on {field}")
            }
            Self::MalformedControlPayload { detail } => {
                write!(f, "malformed control-object payload: {detail}")
            }
            Self::ControlPayloadNotAMap => f.write_str("a control-object payload is a CBOR map"),
            Self::NonZeroControlPadding => {
                f.write_str("control-object payload padding is not zero-filled")
            }
            Self::ControlPaddingLengthMismatch { expected, actual } => write!(
                f,
                "expected a control-object payload padded to {expected} bytes, found {actual}"
            ),
            Self::MissingMasterKeyEpoch => {
                f.write_str("control-object payload carries no master_key_epoch")
            }
            Self::ControlPayloadEncodeFailed { detail } => {
                write!(f, "could not encode control-object payload: {detail}")
            }
            Self::ControlPayloadTooLong { padded } => write!(
                f,
                "a control-object payload padded to {padded} bytes is longer than this platform addresses"
            ),
            Self::UnknownStoredMasterKeyMagic { actual } => {
                write!(f, "unknown magic {actual:?}, not a stored Master Key")
            }
            Self::UnsupportedStoredMasterKeyVersion { actual } => {
                write!(f, "unsupported stored Master Key version {actual}")
            }
            Self::StoredMasterKeyLengthMismatch => {
                f.write_str("stored Master Key is not the length its own header declares")
            }
            Self::UnknownTokenCacheMagic { actual } => {
                write!(f, "unknown magic {actual:?}, not a token cache")
            }
            Self::UnsupportedTokenCacheVersion { actual } => {
                write!(f, "unsupported token cache version {actual}")
            }
            Self::TokenCacheTooShort { actual } => {
                write!(f, "a token cache cannot be {actual} bytes long")
            }
            Self::InvalidArgon2Params { detail } => {
                write!(f, "invalid Argon2id parameters: {detail}")
            }
            Self::PassphraseDerivationFailed { detail } => {
                write!(f, "could not derive the protection key: {detail}")
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
