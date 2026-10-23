use std::error;
use std::fmt;

use coffret_model::{ControlObjectKind, Generation, Redacted};

/// What about a control object did not hold (spec: FM-11, FM-12).
///
/// It is what [`CommitError::CorruptControlObject`](super::CommitError::CorruptControlObject) carries alongside the name
/// the object was read under, so the reason is a value a caller can act on and
/// not a sentence it would have to read.
#[derive(Debug, Clone)]
pub enum ControlObjectFault {
    /// The format layer would not hand a value back for it.
    ///
    /// Two verdicts under one name, and the refusal inside says which: bytes
    /// this build cannot open at all — a header it does not read, a tag that
    /// does not verify — and a payload that opened and was then refused for
    /// what it says. A Snapshot checkpointing a head other than the one its
    /// name is for is the second kind: the decoder is told the name's own
    /// generation and holds the rule there (spec: CK-10), so the refusal
    /// arrives here already saying which head was named and which was
    /// checkpointed.
    Unopenable(coffret_format::Error),
    /// Its header declares a kind the position it was read at does not admit.
    KindNotAdmitted {
        /// The kind its authenticated header declares.
        found: ControlObjectKind,
    },
    /// Its authenticated header stands at another generation than its name
    /// carries.
    GenerationMismatch {
        /// The generation the header states.
        found: Generation,
    },
    /// It is the head a commit would succeed, and it carries no commit slot
    /// where this Storage mints the identifier a successor is created under.
    ///
    /// The head is what carries the one slot every writer starting from it
    /// aims at (spec: CP-2), and on a Storage that mints identifiers that slot
    /// is the pre-minted identifier in its `next_commit_slot`. Minting a fresh
    /// one in its place would give each writer a slot of its own, so two of
    /// them could both succeed the head (spec: CP-3); the commit is refused
    /// instead.
    NoCommitSlot,
    /// It is the head a commit would succeed, and it carries a minted
    /// identifier as its commit slot where this Storage identifies objects by
    /// name.
    ///
    /// Where names identify objects the head persists nothing in
    /// `next_commit_slot` and the slot is the successor's name, re-derived when
    /// it is consumed (spec: CP-2, CP-15). An identifier there was minted by
    /// some other Storage, and this one cannot create under it.
    MintedCommitSlot,
}

impl fmt::Display for ControlObjectFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // What the format layer refused it for is that layer's own answer,
            // which `source` hands on.
            Self::Unopenable(_) => f.write_str("the format layer refused it"),
            Self::KindNotAdmitted { found } => write!(
                f,
                "it carries a {found:?}, which the position it was read at does not admit"
            ),
            Self::GenerationMismatch { found } => {
                write!(f, "its header stands at generation {found}")
            }
            Self::NoCommitSlot => f.write_str(
                "it carries no commit slot, and this Storage mints the identifier its \
                 successor is created under",
            ),
            Self::MintedCommitSlot => f.write_str(
                "it carries a minted identifier as its commit slot, and this Storage \
                 identifies objects by name",
            ),
        }
    }
}

impl error::Error for ControlObjectFault {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Unopenable(error) => Some(error),
            _ => None,
        }
    }
}

impl Redacted for ControlObjectFault {
    /// Which way a control object was not the one it promised to be.
    fn redacted(&self) -> String {
        match self {
            Self::Unopenable(error) => format!("Control::Unopenable: {}", error.redacted()),
            Self::KindNotAdmitted { found } => {
                format!("Control::KindNotAdmitted(found={found:?})")
            }
            Self::GenerationMismatch { found } => {
                format!("Control::GenerationMismatch(found={found})")
            }
            Self::NoCommitSlot => "Control::NoCommitSlot".to_owned(),
            Self::MintedCommitSlot => "Control::MintedCommitSlot".to_owned(),
        }
    }
}
