use crate::{Error, FixtureKind, Result};

use super::{
    Manifest, REQUIRED_CONTAINERS, REQUIRED_CONTROL_OBJECTS, REQUIRED_KEY_ENVELOPES,
    REQUIRED_RECOVERY_CODES, REQUIRED_STORED_MASTER_KEYS, SCHEMA,
};

impl Manifest {
    /// Checks that the set is this schema and covers every fixture the exchange
    /// requires.
    ///
    /// A set that quietly stopped carrying a kind would let that kind's
    /// interoperability lapse without anything going red, so the reader insists
    /// on the whole list rather than checking whatever it happens to find.
    pub fn check_coverage(&self) -> Result<()> {
        check_schema(self.schema)?;
        require(
            FixtureKind::Container,
            &REQUIRED_CONTAINERS,
            self.containers
                .iter()
                .map(|fixture| fixture.fixture.as_str()),
        )?;
        require(
            FixtureKind::ControlObject,
            &REQUIRED_CONTROL_OBJECTS,
            self.control_objects
                .iter()
                .map(|fixture| fixture.fixture.as_str()),
        )?;
        require(
            FixtureKind::KeyEnvelope,
            &REQUIRED_KEY_ENVELOPES,
            self.key_envelopes
                .iter()
                .map(|fixture| fixture.fixture.as_str()),
        )?;
        require(
            FixtureKind::StoredMasterKey,
            &REQUIRED_STORED_MASTER_KEYS,
            self.stored_master_keys
                .iter()
                .map(|fixture| fixture.fixture.as_str()),
        )?;
        require(
            FixtureKind::RecoveryCode,
            &REQUIRED_RECOVERY_CODES,
            self.recovery_codes
                .iter()
                .map(|fixture| fixture.fixture.as_str()),
        )
    }
}

/// A reader accepts the schema it knows and nothing else.
fn check_schema(schema: u64) -> Result<()> {
    if schema != SCHEMA {
        return Err(Error::UnknownSchema { found: schema });
    }
    Ok(())
}

fn require<'a>(
    kind: FixtureKind,
    required: &[&'static str],
    present: impl Iterator<Item = &'a str>,
) -> Result<()> {
    let present: Vec<&str> = present.collect();
    for &name in required {
        if !present.contains(&name) {
            return Err(Error::FixtureUnlisted { kind, name });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;
    use crate::manifest::{
        Argon2ParamsFixture, ContainerFixture, ControlObjectFixture, KeyEnvelopeFixture,
        RecoveryCodeFixture, StoredMasterKeyFixture, WireContainerKind, WireControlObjectKind,
    };

    fn manifest() -> Manifest {
        Manifest {
            schema: SCHEMA,
            producer: "test".to_owned(),
            master_key: hex::encode(&[0u8; 32]),
            passphrase: "correct horse battery staple".to_owned(),
            containers: REQUIRED_CONTAINERS.iter().map(container).collect(),
            control_objects: REQUIRED_CONTROL_OBJECTS
                .iter()
                .map(control_object)
                .collect(),
            key_envelopes: REQUIRED_KEY_ENVELOPES.iter().map(key_envelope).collect(),
            stored_master_keys: REQUIRED_STORED_MASTER_KEYS
                .iter()
                .map(stored_master_key)
                .collect(),
            recovery_codes: REQUIRED_RECOVERY_CODES.iter().map(recovery_code).collect(),
        }
    }

    fn container(fixture: &&str) -> ContainerFixture {
        ContainerFixture {
            fixture: (*fixture).to_owned(),
            file: String::new(),
            object_name: String::new(),
            container_id: hex::encode(&[0u8; 16]),
            container_key: hex::encode(&[0u8; 32]),
            kind: WireContainerKind::Pack,
            chunk_size: 1024,
            entries: Vec::new(),
        }
    }

    fn control_object(fixture: &&str) -> ControlObjectFixture {
        ControlObjectFixture {
            fixture: (*fixture).to_owned(),
            file: String::new(),
            object_name: String::new(),
            kind: WireControlObjectKind::Journal,
            generation: 0,
            replica_index: 0,
            replica_count: 1,
            master_key_epoch: 1,
            body: Vec::new(),
        }
    }

    fn key_envelope(fixture: &&str) -> KeyEnvelopeFixture {
        KeyEnvelopeFixture {
            fixture: (*fixture).to_owned(),
            file: String::new(),
            container_id: hex::encode(&[0u8; 16]),
            container_key: hex::encode(&[0u8; 32]),
        }
    }

    fn stored_master_key(fixture: &&str) -> StoredMasterKeyFixture {
        StoredMasterKeyFixture {
            fixture: (*fixture).to_owned(),
            file: String::new(),
            master_key: hex::encode(&[0u8; 32]),
            epoch: 1,
            argon2: Argon2ParamsFixture {
                memory_kib: 8,
                iterations: 1,
                parallelism: 1,
            },
        }
    }

    fn recovery_code(fixture: &&str) -> RecoveryCodeFixture {
        RecoveryCodeFixture {
            fixture: (*fixture).to_owned(),
            file: String::new(),
            master_key: hex::encode(&[0u8; 32]),
            epoch: 1,
        }
    }

    #[test]
    fn a_complete_set_passes_the_coverage_check() {
        manifest()
            .check_coverage()
            .expect("every fixture is listed");
    }

    #[test]
    fn a_dropped_fixture_is_reported_by_name() {
        let mut manifest = manifest();
        manifest
            .control_objects
            .retain(|fixture| fixture.fixture != "keyring-replica");
        let error = manifest
            .check_coverage()
            .expect_err("the Keyring replica is missing");
        assert!(
            matches!(
                error,
                Error::FixtureUnlisted {
                    kind: FixtureKind::ControlObject,
                    name: "keyring-replica",
                }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn a_manifest_of_another_schema_is_rejected() {
        let mut manifest = manifest();
        manifest.schema = SCHEMA + 1;
        let error = manifest
            .check_coverage()
            .expect_err("another schema is refused");
        assert!(
            matches!(error, Error::UnknownSchema { found } if found == SCHEMA + 1),
            "{error:?}"
        );
    }
}
