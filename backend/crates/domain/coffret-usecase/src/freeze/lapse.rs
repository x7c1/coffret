/// Why a reserved file was not packed (spec: PK-17).
///
/// Three answers and the whole set is named here, because whoever shows them
/// writes a sentence per answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lapse {
    /// No file stands at the path any more: it was deleted or moved since the
    /// drop, or the drop stopped before it landed.
    Gone,
    /// A Pack already holds its Entry, which no freeze rewrites (spec: PK-1,
    /// PK-2) — the drop's Pack committed and this device's catalog had not yet
    /// heard, or another run packed it first.
    InPack,
    /// The file is there and is not eligible for some other reason PK-1 or
    /// EP-10 gives: an Entry this device never placed stands at the path, the
    /// Entry it was a copy of has left the Library or moved, or no mapping of
    /// this device reaches it any more.
    Ineligible,
}
