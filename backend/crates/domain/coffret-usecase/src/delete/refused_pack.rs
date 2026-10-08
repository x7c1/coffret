use coffret_model::{ContainerId, EntryPath};

use crate::delete::pack_refusal::PackRefusal;

/// A Container a deletion left exactly as it was, and why.
///
/// Only a Container that also keeps Entries the request did not name can be
/// refused: one whose Entries are all deleted is simply removed, readable or
/// not (spec: PK-9, KL-17). Keeping the others means read-modify-replace, and
/// when that cannot happen nothing is committed for the Container — neither its
/// removal nor a replacement — so every Entry it held, the named ones included,
/// stays in the Library (spec: PK-10).
///
/// There is deliberately no `PartialEq`: a refusal carries a failure, which a
/// caller reads from the variant and the fields it names, never by comparing
/// two refusals — and the format layer's error it can carry has none either.
#[derive(Debug)]
pub struct RefusedPack {
    /// The Container left in place.
    pub container_id: ContainerId,
    /// The Entries the request did not name, which a replacement would have
    /// had to carry forward, in the order the Container holds them.
    pub kept: Vec<EntryPath>,
    /// The Entries the request did name, which stay in the Library because
    /// their Container does, in the same order.
    pub spared: Vec<EntryPath>,
    /// Why no replacement was written.
    pub reason: PackRefusal,
}
