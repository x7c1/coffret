use coffret_model::{ContainerId, DerivedFrom, EntryPath};
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Serialize, Deserialize)]
pub(super) struct WireDerivedFrom {
    #[serde(with = "serde_bytes")]
    container_id: Vec<u8>,
    path: String,
}

impl From<&DerivedFrom> for WireDerivedFrom {
    fn from(derived_from: &DerivedFrom) -> Self {
        Self {
            container_id: derived_from.container_id.as_bytes().to_vec(),
            path: derived_from.path.as_str().to_owned(),
        }
    }
}

impl WireDerivedFrom {
    pub(super) fn to_domain(&self) -> Result<DerivedFrom> {
        let bytes: [u8; ContainerId::BYTE_LEN] =
            self.container_id.as_slice().try_into().map_err(|_| {
                coffret_model::Error::InvalidByteLength {
                    expected: ContainerId::BYTE_LEN,
                    actual: self.container_id.len(),
                }
            })?;
        Ok(DerivedFrom {
            container_id: ContainerId::from_bytes(bytes),
            path: EntryPath::new(self.path.clone()),
        })
    }
}
