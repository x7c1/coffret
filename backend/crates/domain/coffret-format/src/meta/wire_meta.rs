use serde::{Deserialize, Serialize};

use super::wire_entry::WireEntry;
use super::wire_kind::WireKind;

#[derive(Serialize, Deserialize)]
pub(super) struct WireMeta {
    pub(super) schema: u64,
    pub(super) kind: WireKind,
    pub(super) pad_len: u64,
    pub(super) entries: Vec<WireEntry>,
}
