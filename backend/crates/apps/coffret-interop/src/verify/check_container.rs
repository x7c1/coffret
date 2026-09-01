use anyhow::{bail, Context, Result};
use coffret_format::{decode, DecodedEntry};
use coffret_model::ContainerKind;

use crate::fixture_set::FixtureReader;
use crate::hex;
use crate::manifest::{ContainerFixture, EntryFixture};

use super::same;

pub(super) fn check_container(reader: &FixtureReader, fixture: &ContainerFixture) -> Result<()> {
    let container_id = fixture.container_id()?;
    same(
        "object_name",
        &fixture.object_name,
        &container_id.object_name(),
    )?;

    let bytes = reader.read(&fixture.file)?;
    let opened = decode(&bytes, &fixture.container_key()?).context("opening the Container")?;

    same("container_id", &opened.container_id, &container_id)?;
    same("kind", &opened.kind, &ContainerKind::from(fixture.kind))?;
    same("chunk_size", &opened.chunk_size.get(), &fixture.chunk_size)?;
    same("entry count", &opened.entries.len(), &fixture.entries.len())?;

    let mut offset = 0u64;
    for (index, (opened, expected)) in opened.entries.iter().zip(&fixture.entries).enumerate() {
        check_entry(opened, expected, &mut offset).with_context(|| format!("entry {index}"))?;
    }
    Ok(())
}

fn check_entry(opened: &DecodedEntry, expected: &EntryFixture, offset: &mut u64) -> Result<()> {
    let metadata = &opened.metadata;
    same("path", &metadata.path.as_str(), &expected.path.as_str())?;
    same("mtime", &metadata.mtime.as_unix_seconds(), &expected.mtime)?;
    same(
        "btime",
        &metadata.btime.map(|btime| btime.as_unix_seconds()),
        &expected.btime,
    )?;
    same(
        "derived_from",
        &metadata.derived_from,
        &expected.derived_from()?,
    )?;
    same("mime", &metadata.mime, &expected.mime)?;

    let content = expected.content()?;
    if opened.content != content {
        bail!(
            "content: decoded {}, the manifest states {}",
            hex::encode(&opened.content),
            hex::encode(&content)
        );
    }
    // The stream layout is the one expectation the manifest does not state, so
    // it is derived here from the contents the manifest does state.
    same("offset", &metadata.offset, offset)?;
    same("size", &metadata.size, &(content.len() as u64))?;
    *offset += content.len() as u64;
    Ok(())
}
