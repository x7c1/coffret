use serde::{Deserialize, Serialize};

use super::wire_kind::WireKind;
use super::wire_meta_entry::WireMetaEntry;
use crate::wire_uint::WireUint;

#[derive(Serialize, Deserialize)]
pub(super) struct WireMeta {
    pub(super) schema: WireUint,
    pub(super) kind: WireKind,
    pub(super) pad_len: WireUint,
    pub(super) entries: Vec<WireMetaEntry>,
}
