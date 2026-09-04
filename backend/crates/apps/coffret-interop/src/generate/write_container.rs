use anyhow::{Context, Result};
use coffret_format::{
    encode, generate_container_id, generate_container_key, ChunkSize, EncodeRequest, EntrySource,
};
use coffret_model::{Btime, ContainerKind, Mtime};

use crate::fixture_set::{FixtureWriter, OBJECTS_DIR};
use crate::hex;
use crate::manifest::{ContainerFixture, DerivedFromFixture, EntryFixture, WireContainerKind};

use super::{entry_path, EntryPlan};

pub(super) fn write_container(
    writer: &FixtureWriter,
    fixture: &str,
    kind: ContainerKind,
    chunk_size: ChunkSize,
    plans: &[EntryPlan],
) -> Result<ContainerFixture> {
    let container_id = generate_container_id().context("drawing a Container ID")?;
    let container_key = generate_container_key().context("drawing a Container Key")?;
    let sources: Vec<EntrySource<'_>> = plans
        .iter()
        .map(|plan| EntrySource {
            path: entry_path(plan.path.to_owned()),
            mtime: Mtime::from_unix_seconds(plan.mtime),
            btime: plan.btime.map(Btime::from_unix_seconds),
            content: &plan.content,
            derived_from: plan.derived_from.clone(),
            mime: plan.mime.map(str::to_owned),
        })
        .collect();

    let encoded = encode(&EncodeRequest {
        container_id,
        kind,
        key: &container_key,
        chunk_size,
        entries: &sources,
    })
    .with_context(|| format!("encoding the {fixture:?} Container"))?;

    let file = writer.write(OBJECTS_DIR, encoded.object_name(), encoded.bytes())?;
    Ok(ContainerFixture {
        fixture: fixture.to_owned(),
        file,
        object_name: encoded.object_name().to_owned(),
        container_id: hex::encode(container_id.as_bytes()),
        container_key: hex::encode(container_key.as_bytes()),
        kind: WireContainerKind::from(kind),
        chunk_size: chunk_size.get(),
        entries: plans
            .iter()
            .map(|plan| EntryFixture {
                path: plan.path.to_owned(),
                mtime: plan.mtime,
                btime: plan.btime,
                content: hex::encode(&plan.content),
                derived_from: plan
                    .derived_from
                    .as_ref()
                    .map(DerivedFromFixture::from_model),
                mime: plan.mime.map(str::to_owned),
            })
            .collect(),
    })
}
