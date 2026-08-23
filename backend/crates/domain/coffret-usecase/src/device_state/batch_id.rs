use std::fmt;

/// Names one upload batch on this device.
///
/// Automatic cleanup of a suspected orphan needs positive local provenance
/// identifying the batch that created it, plus proof that the batch did not
/// commit (spec: OC-2, OC-3). This is the first half of that: it is what ties a
/// Container spooled or uploaded before a commit back to the attempt it belongs
/// to, so that abandoning the attempt names exactly what may be removed.
///
/// It never leaves the device — no Journal record or Snapshot carries it — so
/// it is opaque, and any spelling a device can keep unique among its own
/// unfinished batches will do.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BatchId(String);

impl BatchId {
    /// Takes the device's own name for a batch.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The name as this device spells it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BatchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
