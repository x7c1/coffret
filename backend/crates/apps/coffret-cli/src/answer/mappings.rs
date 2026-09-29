//! What `mappings` answers with.

use serde::Serialize;

use coffret_device::Mapping;

use super::{prefix_said, said_path, Failure};

/// What `mappings` listed.
#[derive(Serialize)]
pub struct Mappings {
    mappings: Vec<MappingSaid>,
    refused: Option<Failure>,
}

/// One mapping: a prefix, `null` for the Library root, and the folder it is at.
#[derive(Serialize)]
struct MappingSaid {
    prefix: Option<String>,
    local_root: String,
}

impl Mappings {
    /// `mappings` as listed, and why the Index was not opened, where it was not.
    pub fn new(mappings: &[Mapping], refused: Option<Failure>) -> Self {
        Self {
            mappings: mappings
                .iter()
                .map(|mapping| MappingSaid {
                    prefix: prefix_said(mapping.prefix.as_ref()),
                    local_root: said_path(&mapping.local_root),
                })
                .collect(),
            refused,
        }
    }
}
