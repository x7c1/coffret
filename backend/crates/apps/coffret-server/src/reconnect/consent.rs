use std::future::Future;
use std::pin::Pin;

use coffret_device::{OpenLibrary, Unrenewed};

/// One consent flow under way: done when the grant is cached, or when the
/// flow ends without one.
pub type ConsentFlow = Pin<Box<dyn Future<Output = Result<(), Unrenewed>> + Send>>;

/// How this server asks a person for a grant again.
///
/// A port rather than a call, for one reason: the flow ends at a person's
/// browser and Google's token endpoint, and a case that drives the reconnect
/// route has neither. [`DriveConsent`](super::DriveConsent) is what ships,
/// and a case gives the state its own through
/// [`ServerState::consenting_through`](crate::ServerState::consenting_through)
/// and completes it when it says so.
pub trait Consent: Send + Sync {
    /// The flow for the account `library`'s grant belongs to, which hands
    /// `show` the consent page's URL before it waits, or `None` where the
    /// Library reaches its Storage through no grant at all.
    ///
    /// The flow owns what it needs, so it can outlive the borrow of `library`
    /// and run on a task of its own.
    fn ask(&self, library: &OpenLibrary, show: Box<dyn FnOnce(&str) + Send>)
        -> Option<ConsentFlow>;
}
