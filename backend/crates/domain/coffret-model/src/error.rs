use std::error;
use std::fmt;

use crate::{ContainerId, Generation, PathDefect, Redacted};

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong when building a domain value from raw input.
#[derive(Debug, Clone)]
pub enum Error {
    /// A hex string did not have the number of characters the target type needs.
    InvalidHexLength {
        /// Characters the target type requires.
        expected: usize,
        /// Characters actually supplied.
        actual: usize,
    },
    /// A hex string held a character outside `0-9a-f`.
    ///
    /// Uppercase is rejected as well: identifiers are canonically lowercase, so
    /// accepting both cases would let two spellings name the same value.
    InvalidHexDigit {
        /// The offending character.
        found: char,
    },
    /// A byte slice did not have the length the target type needs.
    InvalidByteLength {
        /// Bytes the target type requires.
        expected: usize,
        /// Bytes actually supplied.
        actual: usize,
    },
    /// A Master Key epoch number falls outside the range epochs are numbered in.
    ///
    /// Numbering starts at 1, so 0 names no epoch, and the last representable
    /// epoch has no successor to rotate into.
    EpochOutOfRange,
    /// The last representable generation has no successor to write next.
    GenerationOutOfRange,
    /// A replica index does not name a replica the count declares.
    InvalidReplicaPosition {
        /// The 0-based index supplied.
        index: u16,
        /// The replica count supplied.
        count: u16,
    },
    /// A control object's name is not one of the forms FM-12 defines.
    MalformedObjectName {
        /// The name as it was presented.
        name: String,
    },
    /// A Keyring `set_digest` is not the non-empty lowercase hex token FM-12
    /// spells it as.
    ///
    /// Uppercase is rejected for the same reason a hex identifier's is: two
    /// spellings of one digest would name one replica set twice, while a commit
    /// selects a set by its exact tuple (KL-3, CP-10).
    InvalidSetDigest {
        /// The digest as it was presented.
        digest: String,
    },
    /// A Storage key prefix a Library's app folder was to be placed under is
    /// neither empty nor terminated by `/` (FM-18).
    ///
    /// Appending to such a base would run it into the folder's own name and
    /// place the Library where the caller did not ask for it, so it is refused
    /// rather than corrected.
    MalformedPrefixBase {
        /// The base as it was presented.
        base: String,
    },
    /// A Keyring replica count declares no replica.
    ///
    /// A set of zero replicas can never be complete, so no commit can ever
    /// select it (KL-2, KL-3).
    InvalidReplicaCount,
    /// An Entry's extent ends past what a Container's plaintext stream can
    /// address (FM-9).
    ///
    /// `offset + size` overflows `u64`, so the pair names no range of a stream
    /// whose positions are 64-bit. A conforming writer never produces one — its
    /// own layout refuses the table before the Container is written — so one
    /// arriving out of a meta section, a control payload, or a catalog row is
    /// malformed data.
    ///
    /// Both numbers travel, and unlike the two variants below both may be
    /// written down: an offset and a length are values the format itself
    /// defines rather than names a person gave anything. Either one alone would
    /// leave a reader unable to say which pair was refused.
    ExtentPastTheAddressSpace {
        /// Where the extent starts.
        offset: u64,
        /// How many bytes it claims from there.
        size: u64,
    },
    /// A path the Library already holds is not the NFC spelling every Entry
    /// Path is in (EP-1).
    ///
    /// Text from outside the Library is composed on the way in, so a stored
    /// path that is not NFC was never written by anything holding to that rule:
    /// it is malformed data. Why it is refused rather than composed is on
    /// [`EntryPath`](crate::EntryPath).
    ///
    /// The offending path travels in the value, which is where a caller
    /// reporting the record it could not read has to get it from. It is Library
    /// content all the same, so it belongs in no log field.
    UnnormalizedEntryPath {
        /// The path as it was stored.
        path: String,
    },
    /// A piece of text that had to be an Entry Path is not in the shape EP-2
    /// spells.
    ///
    /// Which part of the shape it fails is carried rather than left to the
    /// reader to work out: text from outside the Library is refused here and
    /// somebody typed it, so "that is not an Entry Path" on its own tells them
    /// nothing to change. A path the Library already holds that is outside the
    /// shape is malformed data in the way an unnormalized one is — nothing
    /// holding to EP-2 could have written it — and the same refusal serves both.
    ///
    /// The offending path travels in the value, for the reason
    /// [`UnnormalizedEntryPath`](Self::UnnormalizedEntryPath)'s does, and it is
    /// Library content all the same: it belongs in no log field.
    MalformedEntryPath {
        /// The path as it was presented.
        path: String,
        /// The part of the shape it fails.
        defect: PathDefect,
    },
    /// A collection one of the control aggregates carries is not in the
    /// strictly increasing canonical order its rule gives it (FM-15, FM-16,
    /// FM-17, EP-3).
    ///
    /// Strictly increasing, so a repeat fails it too: the keys these
    /// collections are ordered by identify their elements — one Container ID
    /// names one Container, one Entry Path holds at most one current Entry at a
    /// committed state (EP-5) — so an element that does not follow its
    /// predecessor is either an order the encoding does not admit or a
    /// collection naming one thing twice, and both are refused here.
    ///
    /// The order is what makes one Library state have exactly one encoding, so
    /// a caller holding a collection in scan or spool order sorts it first
    /// rather than handing it over unsorted: that is what `canonical` is for.
    CollectionOutOfCanonicalOrder {
        /// The collection, as the rule that orders it names it: `additions`,
        /// `removals`, `containers`, `entries`, or `mapping`.
        collection: &'static str,
        /// Position of the first element that does not follow its predecessor.
        index: usize,
    },
    /// A Journal record does not name the head it succeeds (FM-15).
    ///
    /// A record at generation *g* succeeds head *g − 1*, so its own statement of
    /// what it was built on has exactly one right value; the record at
    /// generation 0 succeeds nothing and states none. A record whose two fields
    /// disagree would make a replay follow a chain no commit ever wrote.
    JournalRecordPredecessorMismatch {
        /// The generation the record takes.
        generation: Generation,
        /// The head it claims to succeed, absent where it claims none.
        prev: Option<Generation>,
    },
    /// A checkpoint's last applied Journal generation is ahead of the head it
    /// stands at (CK-1).
    ///
    /// The two coincide after an ordinary commit and diverge only downwards, at
    /// an epoch activation whose Snapshot takes a head position without being a
    /// Journal record (CP-6, FM-12). A Journal generation past the head names
    /// records applied to reach a state the head does not cover, which no
    /// commit can produce.
    CheckpointJournalAheadOfHead {
        /// The control-head generation the checkpoint represents.
        head_generation: Generation,
        /// The last Journal generation it claims to have applied.
        journal_generation: Generation,
    },
    /// A Container addition carries no Entry (FM-10).
    ///
    /// A Container is built out of Entries, so one holding none is a Container
    /// no writer produces and an addition that adds nothing to the Index.
    AdditionWithoutEntries,
    /// A Container addition's entry table does not tile its Container's
    /// plaintext stream from offset 0 (FM-9).
    ///
    /// Every Entry begins where its predecessor ended and the first begins at
    /// zero, so a gap, an overlap, and a table that starts anywhere else are
    /// one refusal: the offset an Entry claims is not the one the walk had
    /// reached.
    AdditionEntriesDoNotTile {
        /// Position of the offending Entry in the table.
        entry: usize,
        /// Where the walk stood: the end of everything before it.
        expected: u64,
        /// Where the Entry claims to start instead.
        found: u64,
    },
    /// A Container addition's entry table names one Entry Path twice (EP-5).
    ///
    /// One Entry Path identifies at most one current Entry at a committed
    /// state, so a table naming one twice puts two current Entries at one
    /// position the moment the record is applied.
    ///
    /// Only the position travels. Which path it was is Library content, and an
    /// index says which element to look at without naming any of it.
    AdditionNamesOnePathTwice {
        /// Position of the second Entry naming a path the table already holds.
        entry: usize,
    },
    /// An Index Snapshot holds an Entry in a Container the Snapshot does not
    /// list (FM-16).
    ///
    /// A Snapshot's Entries name their Containers among the ones it carries, so
    /// an Entry naming another leaves a restored Index pointing at a Container
    /// it does not hold.
    SnapshotEntryWithoutContainer {
        /// Position of the offending Entry in `entries`.
        entry: usize,
        /// The Container it named.
        container_id: ContainerId,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHexLength { expected, actual } => {
                write!(f, "expected {expected} hex characters, found {actual}")
            }
            Self::InvalidHexDigit { found } => {
                write!(f, "expected a lowercase hex character, found {found:?}")
            }
            Self::InvalidByteLength { expected, actual } => {
                write!(f, "expected {expected} bytes, found {actual}")
            }
            Self::EpochOutOfRange => f.write_str("Master Key epochs are numbered from 1 upward"),
            Self::GenerationOutOfRange => {
                f.write_str("the last representable generation has no successor")
            }
            Self::InvalidReplicaPosition { index, count } => {
                write!(f, "replica {index} is not one of {count} replicas")
            }
            Self::MalformedObjectName { name } => {
                write!(f, "{name:?} is not a control-object name")
            }
            Self::InvalidSetDigest { digest } => {
                write!(f, "{digest:?} is not a lowercase hex Keyring digest")
            }
            Self::MalformedPrefixBase { base } => {
                write!(f, "the prefix {base:?} does not end in a \"/\"")
            }
            Self::InvalidReplicaCount => {
                f.write_str("a Keyring replica set declares at least one replica")
            }
            Self::ExtentPastTheAddressSpace { offset, size } => write!(
                f,
                "an Entry of {size} bytes at offset {offset} ends past the plaintext stream's address space",
            ),
            Self::UnnormalizedEntryPath { path } => {
                write!(f, "the stored path {path:?} is not normalized to NFC")
            }
            Self::MalformedEntryPath { path, defect } => {
                write!(f, "{path:?} is not an Entry Path: {defect}")
            }
            Self::CollectionOutOfCanonicalOrder { collection, index } => write!(
                f,
                "element {index} of {collection} does not follow its predecessor in the canonical order",
            ),
            Self::JournalRecordPredecessorMismatch { generation, prev } => match prev {
                Some(prev) => write!(
                    f,
                    "the Journal record at generation {generation} states {prev} as the head it succeeds",
                ),
                None => write!(
                    f,
                    "the Journal record at generation {generation} states no head it succeeds",
                ),
            },
            Self::CheckpointJournalAheadOfHead {
                head_generation,
                journal_generation,
            } => write!(
                f,
                "a checkpoint at head {head_generation} claims to have applied Journal generation {journal_generation}",
            ),
            Self::AdditionWithoutEntries => {
                f.write_str("a Container addition carries no Entry")
            }
            Self::AdditionEntriesDoNotTile {
                entry,
                expected,
                found,
            } => write!(
                f,
                "entry {entry} starts at {found} where the plaintext stream had reached {expected}",
            ),
            Self::AdditionNamesOnePathTwice { entry } => write!(
                f,
                "entry {entry} names an Entry Path the same Container already holds",
            ),
            Self::SnapshotEntryWithoutContainer { entry, container_id } => write!(
                f,
                "entry {entry} is held by {container_id}, which this Snapshot does not list",
            ),
        }
    }
}

impl error::Error for Error {}

impl Redacted for Error {
    /// The counts and the positions, and none of the text that was presented.
    ///
    /// Every value this vocabulary refuses arrived from somewhere outside the
    /// process — a control-object name a listing answered with, a digest, a
    /// prefix somebody typed, a path read back out of a record — so none of
    /// them is a fact this Library minted and none of them is written down.
    /// What is left is the shape of the refusal, which is what a reader
    /// grouping a log file by it is after.
    fn redacted(&self) -> String {
        match self {
            Self::InvalidHexLength { expected, actual } => {
                format!("Model::InvalidHexLength(expected={expected}, actual={actual})")
            }
            Self::InvalidHexDigit { .. } => "Model::InvalidHexDigit".to_owned(),
            Self::InvalidByteLength { expected, actual } => {
                format!("Model::InvalidByteLength(expected={expected}, actual={actual})")
            }
            Self::EpochOutOfRange => "Model::EpochOutOfRange".to_owned(),
            Self::GenerationOutOfRange => "Model::GenerationOutOfRange".to_owned(),
            Self::InvalidReplicaPosition { index, count } => {
                format!("Model::InvalidReplicaPosition(index={index}, count={count})")
            }
            Self::MalformedObjectName { .. } => "Model::MalformedObjectName".to_owned(),
            Self::InvalidSetDigest { .. } => "Model::InvalidSetDigest".to_owned(),
            Self::MalformedPrefixBase { .. } => "Model::MalformedPrefixBase".to_owned(),
            Self::InvalidReplicaCount => "Model::InvalidReplicaCount".to_owned(),
            // Both values, because both are the format's own arithmetic: an
            // offset and a length say where bytes sit in a stream and nothing
            // about whose bytes they are.
            Self::ExtentPastTheAddressSpace { offset, size } => {
                format!("Model::ExtentPastTheAddressSpace(offset={offset}, size={size})")
            }
            // The two variants carrying a path, and the reason this whole
            // vocabulary needs a rendering of its own: it is Library content
            // whatever it turns out to be a path to, so only its length
            // survives — beside the defect, which says what is wrong with the
            // path without saying any of it.
            Self::UnnormalizedEntryPath { path } => {
                format!("Model::UnnormalizedEntryPath(path_len={})", path.len())
            }
            Self::MalformedEntryPath { path, defect } => format!(
                "Model::MalformedEntryPath(defect={defect}, path_len={})",
                path.len()
            ),
            // The refusals the control aggregates raise, every field of them.
            // What they name is the format's own bookkeeping — a collection the
            // rules name, a position in it, a generation, a stream offset, a
            // Container ID — and not one of them is anything a person chose, so
            // a reader grouping a log file by refusal gets to see which
            // aggregate was refused and where.
            Self::CollectionOutOfCanonicalOrder { collection, index } => {
                format!("Model::CollectionOutOfCanonicalOrder(collection={collection}, index={index})")
            }
            Self::JournalRecordPredecessorMismatch { generation, prev } => match prev {
                Some(prev) => format!(
                    "Model::JournalRecordPredecessorMismatch(generation={generation}, prev={prev})"
                ),
                None => format!(
                    "Model::JournalRecordPredecessorMismatch(generation={generation}, prev=none)"
                ),
            },
            Self::CheckpointJournalAheadOfHead {
                head_generation,
                journal_generation,
            } => format!(
                "Model::CheckpointJournalAheadOfHead(head_generation={head_generation}, journal_generation={journal_generation})"
            ),
            Self::AdditionWithoutEntries => "Model::AdditionWithoutEntries".to_owned(),
            Self::AdditionEntriesDoNotTile {
                entry,
                expected,
                found,
            } => format!(
                "Model::AdditionEntriesDoNotTile(entry={entry}, expected={expected}, found={found})"
            ),
            Self::AdditionNamesOnePathTwice { entry } => {
                format!("Model::AdditionNamesOnePathTwice(entry={entry})")
            }
            Self::SnapshotEntryWithoutContainer { entry, container_id } => format!(
                "Model::SnapshotEntryWithoutContainer(entry={entry}, container_id={container_id})"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // EP-1: a path read back out of a record is the user's own name for their
    // file whether or not it is spelled the way the Library spells one, so the
    // message that names it is not what a log line renders.
    #[test]
    fn a_path_a_record_carried_never_reaches_a_log_line() {
        let error = Error::UnnormalizedEntryPath {
            path: "albums/spring.jpg".to_owned(),
        };

        assert!(error.to_string().contains("albums/spring.jpg"));
        assert_eq!(
            error.redacted(),
            "Model::UnnormalizedEntryPath(path_len=17)",
        );
    }

    // EP-2: the defect is what a reader grouping a log file by refusal is
    // after, and it says which part of the shape went without saying any of the
    // path it went in.
    #[test]
    fn a_malformed_path_reaches_a_log_line_as_its_defect_alone() {
        let error = Error::MalformedEntryPath {
            path: "albums/../etc".to_owned(),
            defect: PathDefect::RelativeComponent,
        };

        assert!(error.to_string().contains("albums/../etc"));
        assert_eq!(
            error.redacted(),
            "Model::MalformedEntryPath(defect=it holds a `.` or `..` component, path_len=13)",
        );
    }

    // FM-9: an offset and a length are the format's own arithmetic rather than
    // anything a person named, so both of them survive into a log line — which
    // is what lets a reader tell one refused table from another.
    #[test]
    fn an_extent_that_ran_off_the_end_is_logged_with_both_its_numbers() {
        let error = Error::ExtentPastTheAddressSpace {
            offset: u64::MAX,
            size: 1,
        };

        assert_eq!(
            error.redacted(),
            "Model::ExtentPastTheAddressSpace(offset=18446744073709551615, size=1)",
        );
    }

    #[test]
    fn a_refusal_about_a_name_keeps_its_identity_and_drops_the_name() {
        let error = Error::MalformedObjectName {
            name: "not-a-control-object".to_owned(),
        };

        assert_eq!(error.redacted(), "Model::MalformedObjectName");
    }
}
