use std::error;
use std::fmt;

use coffret_model::{ContainerId, ControlObjectKind, ControlObjectName, Generation};

use crate::control::ControlHeader;
use crate::header::Header;
use crate::purpose::Purpose;
use crate::recovery_code::RecoveryCode;
use crate::stored_master_key::StoredMasterKey;
use crate::token_cache::MAGIC_LEN as TOKEN_CACHE_MAGIC_LEN;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong writing or reading a coffret object.
///
/// The header-shape variants are raised before any key is touched, so an object
/// that is not a Container v1 — or not a control object v1 — at all is
/// distinguishable from one that is but fails to open.
///
/// What a variant carries is bounded by where the value came from. A value the
/// format itself defines is named outright — a magic, a version or kind byte, a
/// field name, an index, a length, a Container `kind` spelling, and the names
/// and digests FM-12 already spells in the open. A value a payload carried is
/// not: a payload is Library content, Entry Paths among it, and an error travels
/// further than the payload does, so a reader names the field and the shape
/// found in its place (`control::cbor::describe`). The one value that still
/// reaches a message is whatever ciborium quotes in its own text, which a
/// `detail` passes through as it stands.
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
    /// The header records a chunk size this build cannot cut a stream with:
    /// zero, or one past what this platform can address.
    ///
    /// The second is not a claim about the object — a 64-bit reader opens what a
    /// 32-bit one refuses — which is why it is this variant and not one of the
    /// ones that accuse the bytes.
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
    MetaSectionTooLong {
        /// The padded length FM-9's rule calls for.
        padded: u64,
        /// The longest padded meta section the header can describe.
        limit: u64,
    },
    /// A Container must hold at least one Entry.
    EmptyEntryTable,
    /// The entry table does not tile the plaintext stream from offset zero
    /// without gaps or overlaps.
    EntryTableNotContiguous {
        /// Index of the first entry whose offset does not follow its predecessor.
        index: usize,
    },
    /// An Entry Path in a decoded entry table is not the NFC spelling every
    /// Entry Path is in (EP-1).
    ///
    /// One entry table is read out of a meta section, out of a Journal record's
    /// additions, and out of an Index Snapshot (FM-9, FM-15, FM-16), so one
    /// refusal serves all three. A path that is not NFC was written by something
    /// that did not hold to EP-1, and it is refused rather than composed — see
    /// [`coffret_model::EntryPath`] for why a stored path is never rewritten.
    ///
    /// Which field carried it is named and the path itself is not, on the rule
    /// this enum states above.
    UnnormalizedEntryPath {
        /// The field the offending path stood in, as the map carrying it
        /// names it: `original_path` in a meta section, `path` in a record or a
        /// Snapshot, and `derived_from.original_path` in either
        /// (FM-9, FM-15, FM-16).
        field: &'static str,
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
    /// A meta section plaintext is not the length FM-9's padding rule gives
    /// the map it carries.
    MetaPaddingLengthMismatch {
        /// The Padmé bucket the map that was read belongs in.
        expected: u64,
        /// The length the plaintext actually is.
        actual: u64,
    },
    /// A decrypted Entry does not hash to the value its metadata records.
    ContentHashMismatch {
        /// Index of the offending entry in the entry table.
        index: usize,
    },
    /// A streaming encode was closed with an Entry still short of its declared
    /// length.
    ///
    /// The entry table was written before the content arrived, so an Entry that
    /// never arrived whole would leave an object whose table describes bytes it
    /// does not hold. The usual cause is a local file that changed between being
    /// surveyed and being read.
    EntryLengthMismatch {
        /// Index of the offending entry in the entry table.
        index: usize,
        /// The length the plan declared.
        expected: u64,
        /// How much of it arrived.
        actual: u64,
    },
    /// The bytes fed for an Entry do not hash to the value its plan declares.
    ///
    /// The other half of the same guarantee: a declared size an Entry happens to
    /// keep says nothing about its content having stayed the same.
    EntryHashMismatch {
        /// Index of the offending entry in the entry table.
        index: usize,
    },
    /// More bytes were fed to a streaming encode than its entry table plans for.
    StreamOverrun {
        /// How many content bytes the whole entry table plans for.
        planned: u64,
    },
    /// A chunk run was asked for over plaintext the stream does not reach.
    ///
    /// The range came from somewhere other than this object's own entry table —
    /// a catalog describing a different state of the Library, or a caller's
    /// arithmetic — and there is no chunk to read for it.
    PlaintextRangeOutOfBounds {
        /// Where the requested range starts.
        start: u64,
        /// Where it ends.
        end: u64,
        /// How long the padded plaintext stream actually is (FM-4).
        plaintext_len: u64,
    },
    /// A chunk run ended before every chunk of it had arrived whole.
    ///
    /// The run's ciphertext extent follows from the header and the meta section
    /// (FM-2, FM-5), so a short delivery is the provider answering with fewer
    /// bytes than were asked for rather than anything about the object. No
    /// plaintext from the unfinished chunk is released.
    ChunkRunTruncated {
        /// How many ciphertext bytes the run covers.
        expected: u64,
        /// How many arrived.
        actual: u64,
    },
    /// More ciphertext arrived than the chunk run covers.
    ///
    /// The other half of [`ChunkRunTruncated`](Self::ChunkRunTruncated), and it
    /// carries the same two numbers for the same reason: a provider that
    /// answered with one byte too many and one that ignored the range and sent
    /// the whole object are the same variant, and only the count tells them
    /// apart afterwards.
    ChunkRunOverrun {
        /// How many ciphertext bytes the run covers.
        expected: u64,
        /// How many have been offered to the run, this piece included.
        actual: u64,
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
    /// A Journal record payload is not the CBOR shape FM-15 defines.
    MalformedJournalRecord {
        /// Which field, and what was found there instead.
        detail: String,
    },
    /// A Journal record payload declares a schema this build cannot read.
    UnsupportedJournalRecordSchema {
        /// The schema number found.
        schema: u64,
    },
    /// A Journal record's `prev` does not name the head it was built on
    /// (FM-15).
    ///
    /// A record at generation *g* succeeds head *g − 1*, so its own statement
    /// of what it was built on has exactly one right value; the record at
    /// generation 0 succeeds nothing and states none.
    JournalRecordPrevMismatch {
        /// The generation the object's header declared.
        generation: Generation,
        /// The head the payload claimed, absent where it carried no `prev`.
        prev: Option<Generation>,
    },
    /// An Index Snapshot payload is not the CBOR shape FM-16 defines.
    MalformedIndexSnapshot {
        /// Which field, and what was found there instead.
        detail: String,
    },
    /// An Index Snapshot payload declares a schema this build cannot read.
    UnsupportedIndexSnapshotSchema {
        /// The schema number found.
        schema: u64,
    },
    /// A Keyring payload is not the CBOR shape FM-17 defines.
    MalformedKeyringPayload {
        /// Which field, and what was found there instead.
        detail: String,
    },
    /// A Keyring payload declares a schema this build cannot read.
    UnsupportedKeyringSchema {
        /// The schema number found.
        schema: u64,
    },
    /// An element of a Keyring's `mapping` spells its key-lost marker `false`
    /// (FM-17).
    ///
    /// The marker's presence is what records the loss, and FM-17 spells it
    /// `true`, so a `false` there is not a way of saying there is no marker —
    /// it is a writer stating the field in a form the rule does not define.
    KeyringEntryMarkerNotTrue {
        /// Position of the offending element in `mapping`.
        index: usize,
    },
    /// An element of a Keyring's `mapping` carries neither a Key Envelope nor
    /// a key-lost marker (FM-17).
    ///
    /// Every Container the Keyring maps is mapped to exactly one of the two, so
    /// an element carrying neither maps its Container to no determinate state —
    /// which is what KL-7's completeness rules out.
    KeyringEntryWithoutEnvelopeOrMarker {
        /// Position of the offending element in `mapping`.
        index: usize,
    },
    /// An element of a Keyring's `mapping` carries both a Key Envelope and a
    /// key-lost marker (FM-17).
    ///
    /// The marker records that no envelope is reachable, so an element carrying
    /// one beside an envelope contradicts itself.
    KeyringEntryWithEnvelopeAndMarker {
        /// Position of the offending element in `mapping`.
        index: usize,
    },
    /// An array in a control-object payload is not in the canonical order its
    /// rule gives it (FM-15, FM-16, FM-17).
    ///
    /// The order is what makes one Library state have one encoding, so a
    /// payload that is not in it is refused rather than sorted into shape.
    ControlPayloadOutOfOrder {
        /// The field holding the array, as its rule names it.
        array: &'static str,
        /// Position of the first element that does not follow its predecessor.
        index: usize,
    },
    /// An Index Snapshot being written holds an Entry in a Container the
    /// Snapshot does not list.
    SnapshotEntryWithoutContainer {
        /// Position of the offending Entry in the content handed in.
        entry: usize,
        /// The Container it named.
        container_id: ContainerId,
    },
    /// An Index Snapshot payload holds an Entry whose `container` index names
    /// no element of `containers` (FM-16).
    DanglingContainerIndex {
        /// Position of the offending Entry in `entries`.
        entry: usize,
        /// The index it carried.
        container: u64,
        /// How many Containers the payload lists.
        containers: usize,
    },
    /// An ordinary Index Snapshot payload carries a field only an activation
    /// Snapshot may (FM-16, MR-2).
    ActivationFieldOnOrdinarySnapshot {
        /// The field, as FM-16 names it.
        field: &'static str,
    },
    /// An activation Index Snapshot payload lacks a field it must carry
    /// (FM-16, MR-2).
    ActivationSnapshotFieldMissing {
        /// The field, as FM-16 names it.
        field: &'static str,
    },
    /// A payload was presented as an Index Snapshot under a control-object kind
    /// that is not one (FM-11).
    NotAnIndexSnapshotKind {
        /// The kind the object's header declared.
        kind: ControlObjectKind,
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
    /// A Recovery Code is not a Bech32 string at all: no separator to divide it
    /// at, nothing before the one it has, or a prefix built from characters a
    /// human-readable part cannot hold.
    ///
    /// A string that does have a prefix and a separator but too few characters
    /// after them is not this: it is a code whose checksum cannot verify, and
    /// [`RecoveryCodeChecksumFailed`](Self::RecoveryCodeChecksumFailed) is what
    /// ends that read.
    MalformedRecoveryCode,
    /// A Recovery Code holds a character outside the Bech32 alphabet.
    ///
    /// The alphabet leaves out `1`, `b`, `i` and `o` precisely because they are
    /// the ones a hand copy substitutes, so naming the character is naming the
    /// transcription mistake.
    RecoveryCodeInvalidCharacter {
        /// The character found.
        actual: char,
    },
    /// A Recovery Code mixes upper and lower case.
    ///
    /// Bech32 admits a code written entirely in either case, and the checksum
    /// covers one case at a time — a mixture is not a third spelling of the
    /// same code but a string no checksum can be verified over.
    RecoveryCodeMixedCase,
    /// A Recovery Code's Bech32m checksum does not verify.
    RecoveryCodeChecksumFailed,
    /// A Recovery Code's human-readable part is not the one coffret writes.
    UnknownRecoveryCodePrefix {
        /// The prefix found, lowercased.
        actual: String,
    },
    /// A Recovery Code's data part is not the 66 characters KD-11's 41-byte
    /// payload takes.
    RecoveryCodeLengthMismatch {
        /// How many data characters were found, the checksum excluded.
        actual: usize,
    },
    /// A Recovery Code's two trailing padding bits are not zero.
    NonZeroRecoveryCodePadding,
    /// The version byte in a Recovery Code names a form this build cannot read.
    UnsupportedRecoveryCodeVersion {
        /// The version byte found.
        actual: u8,
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
            Self::InvalidChunkSize => {
                f.write_str("chunk size is zero, or past what this build can address")
            }
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
            Self::MetaSectionTooLong { padded, limit } => write!(
                f,
                "a meta section padded to {padded} bytes exceeds the {limit} the header's length field records"
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
            Self::StreamTooLong => f.write_str("entry sizes overflow the plaintext stream"),
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
