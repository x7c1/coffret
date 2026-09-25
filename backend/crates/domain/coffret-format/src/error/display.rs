//! The line each [`Error`] variant is read out as.

use std::fmt;

use crate::control::ControlHeader;
use crate::header::Header;
use crate::recovery_code::RecoveryCode;

use super::Error;

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
            Self::InvalidChunkSize => f.write_str("chunk size is zero"),
            Self::UnaddressableOnThisBuild { what, declared } => write!(
                f,
                "a {what} of {declared} bytes is past what this build can address"
            ),
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
            Self::MetaSectionTooLong { declared, ceiling } => write!(
                f,
                "a meta section of {declared} bytes is past the {ceiling} a Container may carry"
            ),
            Self::EmptyEntryTable => f.write_str("a Container must hold at least one Entry"),
            Self::EntryTableNotContiguous { index } => {
                write!(
                    f,
                    "entry {index} does not follow its predecessor in the stream"
                )
            }
            Self::UnnormalizedEntryPath { field } => {
                write!(f, "the {field} of an entry is not normalized to NFC")
            }
            Self::MalformedEntryPath { field } => {
                write!(f, "the {field} of an entry is not an Entry Path")
            }
            Self::StreamTooLong => f.write_str(
                "the entry table runs past the last plaintext stream position the format admits",
            ),
            Self::PlaintextLengthMismatch { expected, actual } => {
                write!(f, "expected {expected} plaintext bytes, decrypted {actual}")
            }
            Self::NonZeroPadding => f.write_str("padding tail is not zero-filled"),
            Self::NonZeroMetaPadding => f.write_str("meta section padding is not zero-filled"),
            Self::MetaPaddingLengthMismatch { expected, actual } => write!(
                f,
                "expected a meta section padded to {expected} bytes, found {actual}"
            ),
            Self::ContentHashMismatch { index } => {
                write!(f, "entry {index} does not match its recorded content hash")
            }
            Self::EntryLengthMismatch {
                index,
                expected,
                actual,
            } => write!(
                f,
                "entry {index} plans for {expected} bytes and {actual} were written"
            ),
            Self::EntryHashMismatch { index } => {
                write!(f, "entry {index} does not hash to the value it plans for")
            }
            Self::StreamOverrun { planned } => write!(
                f,
                "more bytes were written than the {planned} the entry table plans for"
            ),
            Self::PlaintextRangeOutOfBounds {
                start,
                end,
                plaintext_len,
            } => write!(
                f,
                "the plaintext range {start}..{end} reaches past the {plaintext_len} \
                 bytes this Container's stream holds"
            ),
            Self::ChunkRunTruncated { expected, actual } => write!(
                f,
                "a chunk run of {expected} ciphertext bytes ended after {actual}"
            ),
            Self::ChunkRunOverrun { expected, actual } => write!(
                f,
                "a chunk run of {expected} ciphertext bytes was offered {actual}"
            ),
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
            Self::ControlHeaderGenerationOutOfRange { generation } => write!(
                f,
                "the header's generation {generation} is past the largest the format admits"
            ),
            Self::UnknownControlObjectKind { actual } => {
                write!(f, "unknown control-object kind {actual:#04x}")
            }
            Self::MissingControlPayload => f.write_str("control object carries no payload"),
            Self::ControlObjectTooLong { kind, len, ceiling } => write!(
                f,
                "a control object of {len} bytes is past the {ceiling} a {kind:?} may be"
            ),
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
            Self::MalformedJournalRecord { detail } => {
                write!(f, "malformed Journal record payload: {detail}")
            }
            Self::UnsupportedJournalRecordSchema { schema } => {
                write!(f, "unsupported Journal record payload schema {schema}")
            }
            Self::JournalRecordPrevMismatch { generation, prev } => match prev {
                Some(prev) => write!(
                    f,
                    "the Journal record at generation {generation} states {prev} as the head it succeeds"
                ),
                None => write!(
                    f,
                    "the Journal record at generation {generation} states no head it succeeds"
                ),
            },
            Self::MalformedIndexSnapshot { detail } => {
                write!(f, "malformed Index Snapshot payload: {detail}")
            }
            Self::UnsupportedIndexSnapshotSchema { schema } => {
                write!(f, "unsupported Index Snapshot payload schema {schema}")
            }
            Self::MalformedKeyringPayload { detail } => {
                write!(f, "malformed Keyring payload: {detail}")
            }
            Self::UnsupportedKeyringSchema { schema } => {
                write!(f, "unsupported Keyring payload schema {schema}")
            }
            Self::KeyringEntryMarkerNotTrue { index } => write!(
                f,
                "element {index} of mapping spells its key-lost marker false rather than true"
            ),
            Self::KeyringEntryWithoutEnvelopeOrMarker { index } => write!(
                f,
                "element {index} of mapping carries neither a Key Envelope nor a key-lost marker"
            ),
            Self::KeyringEntryWithEnvelopeAndMarker { index } => write!(
                f,
                "element {index} of mapping carries a Key Envelope and a key-lost marker at once"
            ),
            Self::ControlPayloadOutOfOrder { array, index } => write!(
                f,
                "element {index} of {array} does not follow its predecessor in the canonical order"
            ),
            Self::SnapshotEntryWithoutContainer { entry, container_id } => write!(
                f,
                "entry {entry} is held by {container_id}, which this Snapshot does not list"
            ),
            Self::AdditionWithoutEntries { addition } => write!(
                f,
                "addition {addition} carries no Entry"
            ),
            Self::AdditionEntriesDoNotTile {
                addition,
                entry,
                expected,
                found,
            } => write!(
                f,
                "entry {entry} of addition {addition} starts at {found} where the plaintext stream had reached {expected}"
            ),
            Self::AdditionNamesOnePathTwice { addition, entry } => write!(
                f,
                "entry {entry} of addition {addition} names an Entry Path the same Container already holds"
            ),
            Self::CheckpointJournalAheadOfHead {
                head_generation,
                journal_generation,
            } => write!(
                f,
                "a checkpoint at head {head_generation} claims to have applied Journal generation {journal_generation}"
            ),
            Self::SnapshotCheckpointsAnotherHead {
                generation,
                head_generation,
            } => write!(
                f,
                "the Index Snapshot named for generation {generation} checkpoints head {head_generation}"
            ),
            Self::ActivationBaseHeadNotEarlier {
                head_generation,
                base_head_generation,
            } => write!(
                f,
                "an activation Snapshot at head {head_generation} names {base_head_generation} as the head whose commit slot it consumed"
            ),
            Self::DanglingContainerIndex {
                entry,
                container,
                containers,
            } => write!(
                f,
                "entry {entry} names container {container}, not one of the {containers} this Snapshot lists"
            ),
            Self::ActivationFieldOnOrdinarySnapshot { field } => write!(
                f,
                "an ordinary Index Snapshot carries no {field}"
            ),
            Self::ActivationSnapshotFieldMissing { field } => {
                write!(f, "an activation Index Snapshot carries {field}")
            }
            Self::NotAnIndexSnapshotKind { kind } => {
                write!(f, "a control object of kind {kind:?} is no Index Snapshot")
            }
            Self::UnknownStoredMasterKeyMagic { actual } => {
                write!(f, "unknown magic {actual:?}, not a stored Master Key")
            }
            Self::UnsupportedStoredMasterKeyVersion { actual } => {
                write!(f, "unsupported stored Master Key version {actual}")
            }
            Self::StoredMasterKeyLengthMismatch => {
                f.write_str("stored Master Key is not the length its own header declares")
            }
            Self::StoredMasterKeyEpochOutOfRange { epoch } => write!(
                f,
                "the epoch {epoch} sealed beside a stored Master Key numbers no epoch"
            ),
            Self::UnknownTokenCacheMagic { actual } => {
                write!(f, "unknown magic {actual:?}, not a token cache")
            }
            Self::UnsupportedTokenCacheVersion { actual } => {
                write!(f, "unsupported token cache version {actual}")
            }
            Self::TokenCacheTooShort { actual } => {
                write!(f, "a token cache cannot be {actual} bytes long")
            }
            Self::MalformedRecoveryCode => f.write_str("this is not a Recovery Code"),
            Self::RecoveryCodeInvalidCharacter { actual } => {
                write!(f, "a Recovery Code holds no character {actual:?}")
            }
            Self::RecoveryCodeMixedCase => {
                f.write_str("a Recovery Code is written in one case, not a mixture of two")
            }
            Self::RecoveryCodeChecksumFailed => {
                f.write_str("a Recovery Code's checksum does not verify")
            }
            Self::UnknownRecoveryCodePrefix { actual } => write!(
                f,
                "unknown prefix {actual:?}, not the {:?} a Recovery Code starts with",
                RecoveryCode::HUMAN_READABLE_PART
            ),
            Self::RecoveryCodeLengthMismatch { actual } => write!(
                f,
                "expected {} data characters in a Recovery Code, found {actual}",
                RecoveryCode::DATA_LEN
            ),
            Self::NonZeroRecoveryCodePadding => {
                f.write_str("a Recovery Code's padding bits are not zero")
            }
            Self::UnsupportedRecoveryCodeVersion { actual } => {
                write!(f, "unsupported Recovery Code version {actual}")
            }
            Self::RecoveryCodeEpochOutOfRange { epoch } => {
                write!(f, "the epoch {epoch} in a Recovery Code numbers no epoch")
            }
            // These four say which of this layer's steps refused and nothing
            // more, because `source` hands the value the layer below reported
            // on and a caller walking the chain prints both. Rendering that
            // value here as well would spell one refusal twice over.
            Self::InvalidArgon2Params { .. } => f.write_str("invalid Argon2id parameters"),
            Self::PassphraseDerivationFailed { .. } => {
                f.write_str("could not derive the protection key")
            }
            Self::EntropyUnavailable { .. } => f.write_str("could not draw random bytes"),
            Self::Model(_) => f.write_str(
                "a value in a meta section or a control object is not one the domain admits",
            ),
        }
    }
}
