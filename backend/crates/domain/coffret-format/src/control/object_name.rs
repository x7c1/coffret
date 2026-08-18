use std::fmt;

use coffret_model::{ContainerId, ControlObjectKind, Generation, ReplicaPosition};

use crate::error::{Error, Result};

/// The name a control object is stored under.
///
/// Control objects carry recognizable names because recovery discovers them by
/// name before any index exists:
///
/// ```text
/// jrn-<generation>.cfrt                                  Journal record
/// idx-<generation>.cfrt                                  Index Snapshot
/// key-<generation>-<set_digest>-r<index>-of-<count>.cfrt  Keyring replica
/// ```
///
/// A Journal record and an Index Snapshot are written once each, so their names
/// carry no replica position and they report [`ReplicaPosition::SINGLE`].
///
/// Numbers are spelled in decimal without leading zeros, so one object has
/// exactly one name: a reader that accepted `jrn-007.cfrt` as generation 7 would
/// let two names claim the same object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ControlObjectName {
    /// A Journal record.
    Journal {
        /// Which generation of the Journal this record is.
        generation: Generation,
    },
    /// An Index Snapshot.
    IndexSnapshot {
        /// Which generation of the Index Snapshot this is.
        generation: Generation,
    },
    /// One replica of a Keyring.
    KeyringReplica {
        /// Which generation of the Keyring this replica belongs to.
        generation: Generation,
        /// The digest of the mapping the replica set carries.
        ///
        /// Its contents are the Keyring's business; a name only needs it to be
        /// a lowercase hex token, so that it cannot swallow the separators the
        /// rest of the name is parsed on.
        set_digest: String,
        /// Which replica this is, out of how many.
        replica: ReplicaPosition,
    },
}

/// The name prefix of a Journal record.
const JOURNAL_PREFIX: &str = "jrn-";
/// The name prefix of an Index Snapshot.
const INDEX_SNAPSHOT_PREFIX: &str = "idx-";
/// The name prefix of a Keyring replica.
const KEYRING_PREFIX: &str = "key-";

impl ControlObjectName {
    /// The name of one generation of the Journal.
    pub const fn journal(generation: Generation) -> Self {
        Self::Journal { generation }
    }

    /// The name of one generation of the Index Snapshot.
    pub const fn index_snapshot(generation: Generation) -> Self {
        Self::IndexSnapshot { generation }
    }

    /// The name of one replica of one generation of the Keyring.
    pub fn keyring_replica(
        generation: Generation,
        set_digest: &str,
        replica: ReplicaPosition,
    ) -> Result<Self> {
        // Lowercase only, as every hex spelling in coffret is: two spellings of
        // one digest would be two names for one object.
        let is_lowercase_hex = |byte: u8| matches!(byte, b'0'..=b'9' | b'a'..=b'f');
        if set_digest.is_empty() || !set_digest.bytes().all(is_lowercase_hex) {
            return Err(Error::MalformedObjectName {
                name: set_digest.to_owned(),
            });
        }
        Ok(Self::KeyringReplica {
            generation,
            set_digest: set_digest.to_owned(),
            replica,
        })
    }

    /// Which kind of control object this name belongs to.
    pub const fn kind(&self) -> ControlObjectKind {
        match self {
            Self::Journal { .. } => ControlObjectKind::Journal,
            Self::IndexSnapshot { .. } => ControlObjectKind::IndexSnapshot,
            Self::KeyringReplica { .. } => ControlObjectKind::Keyring,
        }
    }

    /// The generation the name encodes.
    pub const fn generation(&self) -> Generation {
        match self {
            Self::Journal { generation }
            | Self::IndexSnapshot { generation }
            | Self::KeyringReplica { generation, .. } => *generation,
        }
    }

    /// The replica position the name encodes.
    pub const fn replica(&self) -> ReplicaPosition {
        match self {
            Self::Journal { .. } | Self::IndexSnapshot { .. } => ReplicaPosition::SINGLE,
            Self::KeyringReplica { replica, .. } => *replica,
        }
    }

    /// The digest a Keyring replica's name carries, if this is one.
    pub fn set_digest(&self) -> Option<&str> {
        match self {
            Self::KeyringReplica { set_digest, .. } => Some(set_digest),
            _ => None,
        }
    }

    /// Reads a name back into the values it encodes.
    pub fn parse(name: &str) -> Result<Self> {
        let malformed = || Error::MalformedObjectName {
            name: name.to_owned(),
        };
        let body = name
            .strip_suffix(ContainerId::STORAGE_EXTENSION)
            .ok_or_else(malformed)?;

        if let Some(rest) = body.strip_prefix(JOURNAL_PREFIX) {
            return Ok(Self::journal(parse_generation(rest).ok_or_else(malformed)?));
        }
        if let Some(rest) = body.strip_prefix(INDEX_SNAPSHOT_PREFIX) {
            return Ok(Self::index_snapshot(
                parse_generation(rest).ok_or_else(malformed)?,
            ));
        }
        let rest = body.strip_prefix(KEYRING_PREFIX).ok_or_else(malformed)?;

        // The digest is hex, so it holds none of the `-` this splits on and the
        // five fields always land in the same places.
        let fields: Vec<&str> = rest.split('-').collect();
        let [generation, set_digest, index, of, count] = fields[..] else {
            return Err(malformed());
        };
        if of != "of" {
            return Err(malformed());
        }
        let generation = parse_generation(generation).ok_or_else(malformed)?;
        let index = index.strip_prefix('r').ok_or_else(malformed)?;
        let replica = ReplicaPosition::new(
            parse_u16(index).ok_or_else(malformed)?,
            parse_u16(count).ok_or_else(malformed)?,
        )?;
        Self::keyring_replica(generation, set_digest, replica).map_err(|_| malformed())
    }
}

impl fmt::Display for ControlObjectName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let extension = ContainerId::STORAGE_EXTENSION;
        match self {
            Self::Journal { generation } => {
                write!(f, "{JOURNAL_PREFIX}{generation}{extension}")
            }
            Self::IndexSnapshot { generation } => {
                write!(f, "{INDEX_SNAPSHOT_PREFIX}{generation}{extension}")
            }
            Self::KeyringReplica {
                generation,
                set_digest,
                replica,
            } => write!(
                f,
                "{KEYRING_PREFIX}{generation}-{set_digest}-r{}-of-{}{extension}",
                replica.index(),
                replica.count()
            ),
        }
    }
}

/// Reads a decimal number that carries no leading zeros and no sign.
fn parse_digits(digits: &str) -> Option<&str> {
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    Some(digits)
}

fn parse_generation(digits: &str) -> Option<Generation> {
    parse_digits(digits)?.parse().ok().map(Generation::new)
}

fn parse_u16(digits: &str) -> Option<u16> {
    parse_digits(digits)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyring(index: u16, count: u16) -> ControlObjectName {
        ControlObjectName::keyring_replica(
            Generation::new(12),
            "a1b2",
            ReplicaPosition::new(index, count).expect("the position is valid"),
        )
        .expect("a lowercase hex digest is a valid one")
    }

    // FM-12: control objects are named `jrn-<generation>.cfrt`,
    // `idx-<generation>.cfrt`, and
    // `key-<generation>-<set_digest>-r<index>-of-<count>.cfrt`.
    #[test]
    fn names_match_the_forms_the_rule_defines() {
        assert_eq!(
            ControlObjectName::journal(Generation::new(4)).to_string(),
            "jrn-4.cfrt"
        );
        assert_eq!(
            ControlObjectName::index_snapshot(Generation::new(4)).to_string(),
            "idx-4.cfrt"
        );
        assert_eq!(keyring(1, 3).to_string(), "key-12-a1b2-r1-of-3.cfrt");
    }

    // FM-12: every form round-trips, so a name written by one device is read
    // back to the same values by another.
    #[test]
    fn every_form_round_trips() {
        let names = [
            ControlObjectName::journal(Generation::FIRST),
            ControlObjectName::journal(Generation::new(u64::MAX)),
            ControlObjectName::index_snapshot(Generation::new(9)),
            keyring(0, 1),
            keyring(2, 3),
        ];
        for name in names {
            assert_eq!(ControlObjectName::parse(&name.to_string()), Ok(name));
        }
    }

    // FM-12: Journal records and Index Snapshots use replica index 0, count 1.
    #[test]
    fn single_written_kinds_report_replica_zero_of_one() {
        for name in [
            ControlObjectName::journal(Generation::new(1)),
            ControlObjectName::index_snapshot(Generation::new(1)),
        ] {
            assert_eq!(name.replica(), ReplicaPosition::SINGLE);
            assert_eq!(name.replica().index(), 0);
            assert_eq!(name.replica().count(), 1);
            assert_eq!(name.set_digest(), None);
        }
    }

    #[test]
    fn a_name_reports_the_kind_it_belongs_to() {
        assert_eq!(
            ControlObjectName::journal(Generation::FIRST).kind(),
            ControlObjectKind::Journal
        );
        assert_eq!(
            ControlObjectName::index_snapshot(Generation::FIRST).kind(),
            ControlObjectKind::IndexSnapshot
        );
        assert_eq!(keyring(0, 1).kind(), ControlObjectKind::Keyring);
        assert_eq!(keyring(0, 1).set_digest(), Some("a1b2"));
    }

    #[test]
    fn names_outside_the_forms_are_rejected() {
        let names = [
            "jrn-4",                    // no extension
            "jrn-.cfrt",                // no generation
            "jrn-04.cfrt",              // a second spelling of generation 4
            "jrn-4x.cfrt",              // not a number
            "jrn--4.cfrt",              // signed, in effect
            "log-4.cfrt",               // not a kind coffret writes
            "key-12-a1b2-r1-of.cfrt",   // no replica count
            "key-12-a1b2-r1-to-3.cfrt", // not the `of` separator
            "key-12-a1b2-1-of-3.cfrt",  // no `r` on the index
            "key-12-zz-r1-of-3.cfrt",   // digest is not hex
            "key-12-A1B2-r1-of-3.cfrt", // digest is not lowercase
            "key-12--r1-of-3.cfrt",     // no digest
            "0011.cfrt",                // a Container, not a control object
        ];
        for name in names {
            assert_eq!(
                ControlObjectName::parse(name),
                Err(Error::MalformedObjectName {
                    name: name.to_owned()
                }),
                "{name} should not parse"
            );
        }
    }

    // FM-12: a replica index outside its count names no replica, whatever the
    // rest of the name says.
    #[test]
    fn a_name_with_an_inconsistent_replica_position_is_rejected() {
        assert_eq!(
            ControlObjectName::parse("key-12-a1b2-r3-of-3.cfrt"),
            Err(Error::Model(coffret_model::Error::InvalidReplicaPosition {
                index: 3,
                count: 3
            }))
        );
    }
}
