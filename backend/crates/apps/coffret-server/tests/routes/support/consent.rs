use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use coffret_device::{OpenLibrary, Unrenewed};
use coffret_server::{Consent, ConsentFlow};
use tokio::sync::oneshot;

/// The consent page every flow here shows.
pub const CONSENT_PAGE: &str = "https://consent.example/?state=the-flows-own";

/// A consent flow a case completes when it says so.
///
/// What it stands in for is the person and Google: the page shown, the browser
/// coming back to the loopback listener, the token endpoint's answer and the
/// grant cached. What it keeps is the shape the server relies on — the page is
/// shown before the flow waits, and the flow ends exactly once, however it ends.
#[derive(Default)]
pub struct ScriptedConsent {
    asked: AtomicUsize,
    ending: Mutex<Option<oneshot::Sender<Result<(), Unrenewed>>>>,
}

impl ScriptedConsent {
    /// How many flows the server started.
    pub fn asked(&self) -> usize {
        self.asked.load(Ordering::SeqCst)
    }

    /// Ends the flow that is waiting, the way `outcome` says.
    pub fn end(&self, outcome: Result<(), Unrenewed>) {
        let ending = self
            .ending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
            .expect("a flow is waiting to be ended");
        ending
            .send(outcome)
            .expect("the server is still following the flow");
    }
}

impl Consent for ScriptedConsent {
    fn ask(
        &self,
        _library: &OpenLibrary,
        show: Box<dyn FnOnce(&str) + Send>,
    ) -> Option<ConsentFlow> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        let (ending, ended) = oneshot::channel();
        *self
            .ending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(ending);
        Some(Box::pin(async move {
            show(CONSENT_PAGE);
            ended.await.unwrap_or(Err(Unrenewed::Failed))
        }))
    }
}
