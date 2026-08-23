use crate::error::{Error, Result};
use crate::generation::Generation;
use crate::lowercase_hex::is_nonempty_lowercase_hex;

/// The exact Keyring replica set a commit selected.
///
/// A replica set becomes committed only when a Journal commit or a Master Key
/// epoch activation names its whole tuple, and a candidate carrying any other
/// commitment is not selected even at the same generation (spec: KL-3, CP-10).
/// The tuple therefore travels as one value rather than as fields that could be
/// carried apart: the Master Key epoch belongs to it too, and is held once by
/// the [checkpoint](crate::IndexCheckpoint) that this is part of, because a
/// checkpoint belongs to exactly one epoch (spec: CK-3).
///
/// After covered Journal records are pruned, the Index Snapshot preserving this
/// tuple is the only evidence of that selection, which is what the completeness
/// gate before `prune` rests on (spec: CK-4, CK-5, KL-4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyringCommitment {
    generation: Generation,
    replica_count: u16,
    set_digest: String,
}

impl KeyringCommitment {
    /// Takes the generation, replica count, and digest a commit named.
    ///
    /// The digest is a non-empty lowercase hex token, the same spelling a
    /// replica's object name carries it in (spec: FM-12); a count of zero
    /// declares no replica and so can never be complete (spec: KL-2).
    pub fn new(generation: Generation, replica_count: u16, set_digest: &str) -> Result<Self> {
        if replica_count == 0 {
            return Err(Error::InvalidReplicaCount);
        }
        if !is_nonempty_lowercase_hex(set_digest) {
            return Err(Error::InvalidSetDigest {
                digest: set_digest.to_owned(),
            });
        }
        Ok(Self {
            generation,
            replica_count,
            set_digest: set_digest.to_owned(),
        })
    }

    /// Which generation of the Keyring the committed set belongs to.
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    /// How many replicas that generation declares (spec: KL-2).
    pub const fn replica_count(&self) -> u16 {
        self.replica_count
    }

    /// The digest binding the canonical complete mapping from Container IDs to
    /// Key Envelopes and key-lost markers (spec: KL-1, CP-10).
    pub fn set_digest(&self) -> &str {
        &self.set_digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lowercase_hex_digest_is_accepted() {
        let commitment = KeyringCommitment::new(Generation::new(3), 2, "a1b2")
            .expect("a lowercase hex digest is a valid one");
        assert_eq!(commitment.set_digest(), "a1b2");
        assert_eq!(commitment.replica_count(), 2);
    }

    // FM-12, KL-3: one digest has one spelling, so the uppercase form of a
    // committed digest is not the same commitment written differently.
    #[test]
    fn an_uppercase_digest_is_rejected() {
        let result = KeyringCommitment::new(Generation::new(3), 2, "A1B2");
        assert!(
            matches!(result, Err(Error::InvalidSetDigest { ref digest }) if digest == "A1B2"),
            "expected an uppercase digest to be rejected, got {result:?}"
        );
    }

    // KL-2: a set is complete when every replica index the count declares is
    // present, which no set of zero replicas can be.
    #[test]
    fn a_set_of_no_replicas_is_not_a_commitment() {
        let result = KeyringCommitment::new(Generation::new(3), 0, "a1b2");
        assert!(
            matches!(result, Err(Error::InvalidReplicaCount)),
            "expected a count of zero to be rejected, got {result:?}"
        );
    }
}
