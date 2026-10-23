use std::error;
use std::fmt;

use coffret_model::{ControlObjectKind, Redacted};

use super::CommitError;

/// Why no key table was read from one replica of a Keyring generation.
///
/// This is the reader's verdict on a position, not a finding about content: a
/// replica is *valid* when it decrypts, authenticates, and is consistent
/// (spec: KL-1), and only the variants from [`Unreadable`](Self::Unreadable)
/// on establish that the object at a position is not a valid replica.
/// [`Absent`](Self::Absent) and [`Unfetchable`](Self::Unfetchable) know nothing
/// about any content — there was none to look at, or Storage did not hand it
/// over.
///
/// A replica is read back the same way wherever a Keyring is read, and what
/// differs is what the reader does with a failure. A read of a committed set
/// steps over the replica and tries the next, because one valid replica carries
/// the whole key table (spec: KL-6); a candidate set stops the commit, because a
/// set that is not complete is not one a commit may select (spec: CP-8, KL-2);
/// and a commit examining the committed set before it writes rewrites the
/// position instead, because that set is one it owes a repair (spec: KL-11,
/// KL-13). So the reason travels as a value and the reader wraps it in whichever
/// of [`CommitError::KeyringUnreadable`], [`CommitError::IncompleteKeyring`],
/// and [`UnrepairedReplica::Unconfirmed`](super::UnrepairedReplica::Unconfirmed) says which of those it made.
///
/// A fetch that failed and an object that arrived and was rejected are kept
/// apart, because they are different verdicts about the Library rather than two
/// spellings of one. An object that did arrive and could not be opened is a
/// replica that is definitively not one a key table may be read from, so the set
/// it belongs to is a valid replica short: a committed set with one left is the
/// degraded state KL-5 names, with a repair owed to it (spec: KL-13), and a
/// candidate is one no commit may select (spec: KL-2). A caller
/// reading [`CommitError::KeyringUnreadable`] tells a Keyring that has lost
/// replicas from a provider that was merely having a bad minute by which of the
/// two it finds.
#[derive(Debug, Clone)]
pub enum UnusableReplica {
    /// The commitment declares the replica and Storage does not hold it.
    Absent,
    /// Storage did not hand the object over.
    ///
    /// Nothing about the replica's content is known: what failed is the fetch,
    /// and the object it was for may be exactly what its name promises. What
    /// Storage reported travels inside, in this flow's own vocabulary.
    Unfetchable(Box<CommitError>),
    /// The object arrived and could not be opened.
    ///
    /// Decrypting, authenticating, or decoding it failed, so this replica is
    /// definitively not one a key table may be read from. What the format layer
    /// reported travels inside, in this flow's own vocabulary.
    Unreadable(Box<CommitError>),
    /// It opened as another kind of control object.
    ///
    /// Its authenticated header says it is not a Keyring, so this replica is
    /// definitively not one a key table may be read from.
    KindNotAdmitted {
        /// The kind its authenticated header declares.
        found: ControlObjectKind,
    },
    /// The key table it holds is not the one its name promises (spec: CP-10,
    /// KL-14).
    ///
    /// It is not the generation its commitment names, so this replica is
    /// definitively not one a key table may be read from.
    DigestMismatch {
        /// The digest the replica's name carries.
        expected: String,
        /// The digest of the key table the object holds.
        actual: String,
    },
}

impl fmt::Display for UnusableReplica {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => f.write_str("it is not in Storage"),
            // Which way the replica was no good, and not what Storage or the
            // format layer answered: `source` hands that value on, and a
            // caller printing the chain would otherwise read it twice.
            Self::Unfetchable(_) => f.write_str("Storage would not hand it over"),
            Self::Unreadable(_) => f.write_str("it could not be opened"),
            Self::KindNotAdmitted { found } => write!(f, "it carries a {found:?}, not a Keyring"),
            Self::DigestMismatch { expected, actual } => write!(
                f,
                "its key table digests to {actual}, and its name promises {expected}"
            ),
        }
    }
}

impl error::Error for UnusableReplica {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Unfetchable(error) | Self::Unreadable(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl Redacted for UnusableReplica {
    /// Which way a replica was no good, with whatever refused it underneath.
    fn redacted(&self) -> String {
        match self {
            Self::Absent => "Replica::Absent".to_owned(),
            Self::Unfetchable(error) => {
                format!("Replica::Unfetchable: {}", error.redacted())
            }
            Self::Unreadable(error) => format!("Replica::Unreadable: {}", error.redacted()),
            Self::KindNotAdmitted { found } => {
                format!("Replica::KindNotAdmitted(found={found:?})")
            }
            Self::DigestMismatch { expected, actual } => {
                format!("Replica::DigestMismatch(expected={expected}, actual={actual})")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;
    use crate::commit::commit_error::fixtures::{provider_fault, unopenable};
    use crate::error::Error;

    #[test]
    fn a_fetch_that_failed_says_nothing_about_the_replica() {
        // The reading a caller has to be able to make: Storage was asked and
        // did not answer, so the Keyring's own health is unknown and repairing
        // the set is not what this reports.
        let cause = UnusableReplica::Unfetchable(Box::new(provider_fault()));
        let UnusableReplica::Unfetchable(inner) = &cause else {
            panic!("expected an unfetchable replica, got {cause:?}");
        };
        assert!(
            matches!(
                inner.as_ref(),
                CommitError::Storage(Error::ServiceUnavailable { .. })
            ),
            "what Storage reported travels inside, got {inner:?}",
        );
        assert!(
            cause.source().is_some(),
            "the fetch failure is the reason's source",
        );
    }

    #[test]
    fn an_object_that_arrived_and_would_not_open_is_a_bad_replica() {
        // The opposite reading: the object is on Storage and is not a replica
        // this Library can read, so the set it belongs to is a valid replica
        // short — which of the states KL-5 separates that leaves it in is the
        // reader's to say, and both readers construct this same value.
        let cause = UnusableReplica::Unreadable(Box::new(unopenable()));
        let UnusableReplica::Unreadable(inner) = &cause else {
            panic!("expected an unreadable replica, got {cause:?}");
        };
        assert!(
            matches!(inner.as_ref(), CommitError::Format(_)),
            "what the format layer reported travels inside, got {inner:?}",
        );
        assert!(
            cause.source().is_some(),
            "the refusal to open is the reason's source",
        );
    }

    #[test]
    fn the_two_verdicts_do_not_read_alike() {
        let unfetchable = UnusableReplica::Unfetchable(Box::new(provider_fault())).to_string();
        let unreadable = UnusableReplica::Unreadable(Box::new(unopenable())).to_string();
        assert_ne!(
            unfetchable, unreadable,
            "a person reading either one is told which of the two happened",
        );
    }
}
