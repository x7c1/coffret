/// A Library and the account it would use naming two different OAuth clients
/// (spec: SA-8).
///
/// Its own type, carried boxed, because it is four names wide: which Library,
/// which account, and the client each one names — the whole of what a person
/// has to look at to see the two differ.
#[derive(Debug)]
pub struct ClientMismatch {
    /// The Library that was being put here, or opened.
    pub library: String,
    /// The OAuth client that Library names.
    pub library_client: String,
    /// The account it would reference.
    pub account: String,
    /// The OAuth client the account's grant was issued to.
    pub account_client: String,
}
