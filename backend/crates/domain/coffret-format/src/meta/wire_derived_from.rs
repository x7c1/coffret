use coffret_model::{ContainerId, DerivedFrom};
use serde::{Deserialize, Serialize};

use super::stored_path::stored_path;
use crate::error::Result;

/// The parent an Entry's content was derived from (FM-9).
///
/// The parent's Entry Path carries the `original_` prefix for the reason the
/// entry's own does: it is the name the parent stood under when this Container
/// was written, and no later rename reaches into an object already stored. The
/// map is shared verbatim by both spellings of the entry map, so a reference
/// reads the same wherever it travels.
#[derive(Serialize, Deserialize)]
pub(crate) struct WireDerivedFrom {
    #[serde(with = "serde_bytes")]
    container_id: Vec<u8>,
    original_path: String,
}

impl From<&DerivedFrom> for WireDerivedFrom {
    fn from(derived_from: &DerivedFrom) -> Self {
        Self {
            container_id: derived_from.container_id.as_bytes().to_vec(),
            original_path: derived_from.path.as_str().to_owned(),
        }
    }
}

impl WireDerivedFrom {
    pub(crate) fn to_domain(&self) -> Result<DerivedFrom> {
        let bytes: [u8; ContainerId::BYTE_LEN] =
            self.container_id.as_slice().try_into().map_err(|_| {
                coffret_model::Error::InvalidByteLength {
                    expected: ContainerId::BYTE_LEN,
                    actual: self.container_id.len(),
                }
            })?;
        Ok(DerivedFrom {
            container_id: ContainerId::from_bytes(bytes),
            path: stored_path(&self.original_path, "derived_from.original_path")?,
        })
    }
}
