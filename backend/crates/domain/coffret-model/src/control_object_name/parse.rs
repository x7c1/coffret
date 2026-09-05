use super::{ControlObjectName, HEAD_PREFIX, INDEX_SNAPSHOT_PREFIX, KEYRING_PREFIX};
use crate::container_id::ContainerId;
use crate::error::{Error, Result};
use crate::generation::Generation;
use crate::replica_position::ReplicaPosition;

impl ControlObjectName {
    /// Reads a name back into the values it encodes.
    ///
    /// A name shaped like a Keyring replica's whose `set_digest` field is not
    /// the lowercase hex token FM-12 spells it as is reported as
    /// [`Error::InvalidSetDigest`] rather than as a malformed name: a reader
    /// scanning Storage can then tell a replica whose digest field is corrupt
    /// from an object that is no control object at all, which are two
    /// different findings about the Library.
    pub fn parse(name: &str) -> Result<Self> {
        let malformed = || Error::MalformedObjectName {
            name: name.to_owned(),
        };
        let body = name
            .strip_suffix(ContainerId::STORAGE_EXTENSION)
            .ok_or_else(malformed)?;

        if let Some(rest) = body.strip_prefix(HEAD_PREFIX) {
            return Ok(Self::head(parse_generation(rest).ok_or_else(malformed)?));
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
        // The shape is a Keyring replica's by now, so a digest this refuses is
        // a corrupt field of a replica name and not a name of another form:
        // the refusal travels as it stands, naming the digest.
        Self::keyring_replica(generation, set_digest, replica)
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

/// The generation a name spells, and `None` where those digits spell none.
///
/// A number the format does not admit (FM-19) is one of the ways: FM-12 spells
/// a generation in decimal, so digits naming a generation this format cannot
/// carry name no object at all — the same verdict a leading zero gets, since
/// both are shapes no conforming writer ever named an object with.
fn parse_generation(digits: &str) -> Option<Generation> {
    Generation::new(parse_digits(digits)?.parse().ok()?).ok()
}

fn parse_u16(digits: &str) -> Option<u16> {
    parse_digits(digits)?.parse().ok()
}
