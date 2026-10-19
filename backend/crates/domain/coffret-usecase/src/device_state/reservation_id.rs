use std::fmt;

/// Which reservation one is, in the order this device made them
/// (spec: PK-17).
///
/// Drawn by the Index as a reservation is made and never drawn twice, so a
/// later reservation always has a greater one: the order a server resumes them
/// in is this order, which is the order the drops they record arrived in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReservationId(u64);

impl ReservationId {
    /// The reservation numbered `id`.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The number itself.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ReservationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
