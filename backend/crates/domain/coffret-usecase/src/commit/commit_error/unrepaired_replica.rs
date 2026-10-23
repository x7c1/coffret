use std::error;
use std::fmt;

use coffret_model::Redacted;

use super::{CommitError, UnusableReplica};

/// Why one position of a committed Keyring set found short is still not one a
/// valid replica stands at (spec: KL-13, KL-16).
///
/// [`UnusableReplica`] says why a replica could not be *read*; this says why the
/// repair that answer called for did not finish. The two are kept apart because
/// a reader steps over a bad replica and a repair is obliged to replace it, so
/// the vocabulary a repair reports in has a state the reader's has not: a
/// position left deliberately alone.
///
/// That state is [`Unfetchable`](Self::Unfetchable), and it is the reason this
/// is three variants rather than one. A replica Storage would not hand over is
/// not known to be lost — the object at that name may be exactly what it
/// promises — so KL-13's "rewrites the missing replicas" does not reach it and
/// the repair writes nothing there. The set cannot be called complete on that
/// evidence either, so the position still holds the gate closed (spec: KL-16).
/// The other two are ordinary failures of the write KL-14 defines: Storage
/// refused it, or it was acknowledged and the read-back that confirms it found
/// something that is still not a valid replica.
#[derive(Debug, Clone)]
pub enum UnrepairedReplica {
    /// Storage would not hand over the object this position holds.
    ///
    /// Nothing about its content is known, so it was not rewritten. What
    /// Storage reported travels inside, in this flow's own vocabulary.
    ///
    /// It carries [`UnusableReplica::Unfetchable`]'s name because it is that
    /// same verdict: the read of the position answered nothing about the
    /// object, and here that answer is also the verdict on the repair.
    Unfetchable(Box<CommitError>),
    /// Storage refused the rewrite.
    ///
    /// Quota, permissions, or a provider having a bad minute — the retry policy
    /// has already spent what it was willing to (spec: KL-16). What Storage
    /// reported travels inside.
    Unwritten(Box<CommitError>),
    /// The rewrite was acknowledged and the replica did not read back valid.
    ///
    /// A repair confirms itself by reading the replica back, and what that
    /// read-back establishes is the replica's validity (spec: KL-14). This is
    /// that read-back's own verdict.
    Unconfirmed(UnusableReplica),
}

impl fmt::Display for UnrepairedReplica {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // "was not rewritten" first, because a person reading this beside
            // the two below has to be told that this one position was left
            // exactly as it stands rather than written at and refused. What
            // Storage or the reading answered is the value `source` hands on
            // and is read under this line, never inside it as well.
            //
            // The position is named rather than left to a pronoun, because of
            // where this line lands: the wrapper that carries it says which
            // replica the repair stopped at and then spends a clause and a
            // half on what the run did and what it left standing, so by the
            // time a chain reaches this the nearest thing an "it" could point
            // at is the batch or the set. What to do next is not among them:
            // that is the wrapper's advice, which a shell prints after the
            // whole chain, under this line rather than above it.
            Self::Unfetchable(_) => f.write_str(
                "that replica was not rewritten, because Storage would not hand over what it \
                 holds",
            ),
            Self::Unwritten(_) => f.write_str("that replica could not be written"),
            Self::Unconfirmed(_) => {
                f.write_str("that replica was written and did not read back valid")
            }
        }
    }
}

impl error::Error for UnrepairedReplica {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Unfetchable(error) | Self::Unwritten(error) => Some(error.as_ref()),
            Self::Unconfirmed(cause) => Some(cause),
        }
    }
}

impl Redacted for UnrepairedReplica {
    /// Which way the position was left short, with whatever refused it
    /// underneath.
    fn redacted(&self) -> String {
        match self {
            Self::Unfetchable(error) => format!("Repair::Unfetchable: {}", error.redacted()),
            Self::Unwritten(error) => format!("Repair::Unwritten: {}", error.redacted()),
            Self::Unconfirmed(cause) => format!("Repair::Unconfirmed: {}", cause.redacted()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commit::commit_error::fixtures::provider_fault;

    // KL-13: a replica Storage would not hand over is not one a repair may
    // write over, and the three ways a position stays short have to be legible
    // as three rather than as one refusal worded differently.
    #[test]
    fn the_three_ways_a_repair_stops_do_not_read_alike() {
        let unfetchable = UnrepairedReplica::Unfetchable(Box::new(provider_fault()));
        let unwritten = UnrepairedReplica::Unwritten(Box::new(provider_fault()));
        let unconfirmed = UnrepairedReplica::Unconfirmed(UnusableReplica::Absent);

        assert!(
            unfetchable.to_string().contains("was not rewritten"),
            "a position left alone says so: {unfetchable}",
        );
        for (left, right) in [
            (&unfetchable, &unwritten),
            (&unwritten, &unconfirmed),
            (&unfetchable, &unconfirmed),
        ] {
            assert_ne!(left.to_string(), right.to_string());
            assert_ne!(left.redacted(), right.redacted());
        }
    }
}
