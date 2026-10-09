/// Whoever asked for a fetch saying it no longer wants the rest of it.
///
/// Asked only on a parcel boundary and never inside a parcel: a cancelled fetch
/// asks Storage for no further parcel, and never cuts one short to stop at an
/// Entry, because a read that stopped where an Entry did would tell the
/// provider where that Entry ends (spec: PK-20, PK-21). The parcels read before
/// the answer changed stay held.
///
/// A trait over a plain predicate because the caller who wants to stop a fetch
/// — a server whose reader moved to another folder — knows why, and the fetch
/// has no business knowing: it asks one question between parcels and nothing
/// else.
pub trait Cancellation: Send + Sync {
    /// Whether the fetch should ask for no further parcel.
    fn is_cancelled(&self) -> bool;
}

impl<F> Cancellation for F
where
    F: Fn() -> bool + Send + Sync,
{
    fn is_cancelled(&self) -> bool {
        self()
    }
}
