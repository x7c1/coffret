use crate::error::{Error, Result};

/// Which replica of a replicated control object this is, out of how many.
///
/// Only Keyrings are replicated; a Journal record and an Index Snapshot are
/// each written once and therefore carry [`ReplicaPosition::SINGLE`]. The count
/// provides redundancy against individual object loss and carries no quorum
/// semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplicaPosition {
    index: u16,
    count: u16,
}

impl ReplicaPosition {
    /// The position of an object that is written exactly once: replica 0 of 1.
    pub const SINGLE: Self = Self { index: 0, count: 1 };

    /// Takes a 0-based replica index and the replica count it belongs to.
    ///
    /// A set has at least one replica, and every index names a replica the
    /// count declares.
    pub fn new(index: u16, count: u16) -> Result<Self> {
        if count == 0 || index >= count {
            return Err(Error::InvalidReplicaPosition { index, count });
        }
        Ok(Self { index, count })
    }

    /// The 0-based index of this replica.
    pub const fn index(self) -> u16 {
        self.index
    }

    /// How many replicas the set declares.
    pub const fn count(self) -> u16 {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_is_replica_zero_of_one() {
        assert_eq!(ReplicaPosition::SINGLE.index(), 0);
        assert_eq!(ReplicaPosition::SINGLE.count(), 1);
        assert_eq!(
            ReplicaPosition::new(0, 1).expect("replica 0 of 1 is valid"),
            ReplicaPosition::SINGLE
        );
    }

    #[test]
    fn an_index_outside_the_count_is_rejected() {
        let result = ReplicaPosition::new(3, 3);
        assert!(
            matches!(
                result,
                Err(Error::InvalidReplicaPosition { index: 3, count: 3 })
            ),
            "expected replica 3 of 3 to be rejected, got {result:?}"
        );
    }

    #[test]
    fn an_empty_set_is_rejected() {
        let result = ReplicaPosition::new(0, 0);
        assert!(
            matches!(
                result,
                Err(Error::InvalidReplicaPosition { index: 0, count: 0 })
            ),
            "expected replica 0 of 0 to be rejected, got {result:?}"
        );
    }
}
