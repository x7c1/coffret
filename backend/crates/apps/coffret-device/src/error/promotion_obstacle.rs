use super::ClientMismatch;

/// Why a Library's previous per-Library grant could not go into the account
/// the device already holds under the name it would take (spec: SA-8).
///
/// Each asks the person for the same thing — another name — but they are
/// different facts about the held account, and a diagnostic event keeps which
/// one it was.
#[derive(Debug)]
pub enum PromotionObstacle {
    /// The held account was consented to through another OAuth client than the
    /// one the Library names.
    ClientDiffers(Box<ClientMismatch>),
    /// The held account's key could not be reached with the Passphrase given:
    /// no Library that references the account opens with it, or none whose
    /// envelope then opens.
    NotOpened,
    /// The held account's grant does not reach the Library's app folder, or
    /// holds no grant to ask with.
    FolderNotReached,
}
