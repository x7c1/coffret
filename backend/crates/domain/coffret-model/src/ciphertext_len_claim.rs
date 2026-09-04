/// How long a Journal record said a Container's ciphertext is (spec: CP-11,
/// FM-15, FM-16).
///
/// A claim, and the name says so because nothing checks it. What proves a
/// Container's ciphertext is the ciphertext its Library committed is the
/// BLAKE3-256 the same record carries and the authenticated chunks the object
/// is made of (spec: FM-5, FM-8, FM-15) — a length is neither, and a device
/// that trusted this number over those would be taking Storage's word for the
/// object. So it is carried for what it is worth: a figure to log, and a figure
/// the device that wrote the Container can compare its own measurement against.
///
/// Every `u64` is a possible claim, so there is nothing here to refuse. What
/// this type adds is that a caller reaching for the number has to say it is
/// reaching for a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CiphertextLenClaim(u64);

impl CiphertextLenClaim {
    /// The claim a record makes, or a writer makes about the object it just
    /// wrote.
    pub const fn new(len: u64) -> Self {
        Self(len)
    }

    /// The number claimed, for a caller that has decided what it is worth.
    pub const fn get(self) -> u64 {
        self.0
    }
}
