//! Opening the fixture set the TypeScript implementation wrote.
//!
//! Every check compares a decoded value against what the manifest states, never
//! against a value this program derived from the same bytes: an expectation
//! recomputed from the object under test would agree with any bug the object
//! carries. The one thing recomputed here is each Entry's place in the
//! plaintext stream, which the manifest deliberately does not state — the
//! offsets follow from the Entry contents it does state.
//!
//! This module walks the manifest; each fixture kind is checked by a `check_*`
//! submodule of its own, mirroring the `write_*` modules that produced it.

use std::fmt::Debug;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::fixture_set::FixtureReader;

mod check_container;
use check_container::check_container;

mod check_control_object;
use check_control_object::check_control_object;

mod check_key_envelope;
use check_key_envelope::check_key_envelope;

mod check_recovery_code;
use check_recovery_code::check_recovery_code;

mod check_stored_master_key;
use check_stored_master_key::check_stored_master_key;

/// Opens every fixture in the set at `input` and checks it against the manifest.
///
/// The first mismatch stops the run and names the producer, the fixture and the
/// field, so a failing exchange points at the disagreement rather than at the
/// exchange. The producer is part of the message because both directions of the
/// exchange land here: a set this build wrote and a set the other implementation
/// wrote fail the same way otherwise.
pub fn verify(input: &Path) -> Result<()> {
    let reader = FixtureReader::open(input)?;
    let producer = reader.manifest().producer.clone();
    check_fixtures(&reader).with_context(|| format!("in the fixture set {producer:?} wrote"))
}

fn check_fixtures(reader: &FixtureReader) -> Result<()> {
    let manifest = reader.manifest();
    let master_key = manifest.master_key()?;

    for fixture in &manifest.containers {
        check_container(reader, fixture)
            .with_context(|| format!("Container fixture {:?}", fixture.fixture))?;
    }
    for fixture in &manifest.control_objects {
        check_control_object(reader, &master_key, fixture)
            .with_context(|| format!("control-object fixture {:?}", fixture.fixture))?;
    }
    for fixture in &manifest.key_envelopes {
        check_key_envelope(reader, &master_key, fixture)
            .with_context(|| format!("Key Envelope fixture {:?}", fixture.fixture))?;
    }
    for fixture in &manifest.stored_master_keys {
        check_stored_master_key(reader, &manifest.passphrase(), fixture)
            .with_context(|| format!("stored Master Key fixture {:?}", fixture.fixture))?;
    }
    for fixture in &manifest.recovery_codes {
        check_recovery_code(reader, fixture)
            .with_context(|| format!("Recovery Code fixture {:?}", fixture.fixture))?;
    }
    Ok(())
}

/// Reports a decoded value that is not the one the manifest states.
fn same<T: PartialEq + Debug>(field: &str, decoded: &T, stated: &T) -> Result<()> {
    if decoded != stated {
        bail!("{field}: decoded {decoded:?}, the manifest states {stated:?}");
    }
    Ok(())
}
