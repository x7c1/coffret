use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::error::Result;
use crate::google_drive::GoogleDrive;
use crate::http::{StubAnswer, StubTransport};
use crate::oauth::AccessTokens;
use crate::settings::DriveSettings;

/// A source of tokens that mints them out of thin air and counts refreshes.
///
/// What the gateway does around a token is worth testing; obtaining a real one
/// is not, and could not be done in a test run anyway.
pub struct CountingTokens {
    refreshes: AtomicUsize,
}

impl CountingTokens {
    /// Starts a token source that has never been refreshed.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            refreshes: AtomicUsize::new(0),
        })
    }

    /// How many times a token has been minted again.
    pub fn refresh_count(&self) -> usize {
        self.refreshes.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl AccessTokens for CountingTokens {
    async fn access_token(&self) -> Result<String> {
        Ok("token-0".to_owned())
    }

    async fn refresh(&self) -> Result<String> {
        let previous = self.refreshes.fetch_add(1, Ordering::SeqCst);
        Ok(format!("token-{}", previous + 1))
    }
}

/// A store whose every call is answered from a script.
pub fn scripted_drive(
    answers: impl IntoIterator<Item = StubAnswer>,
) -> (GoogleDrive, Arc<StubTransport>, Arc<CountingTokens>) {
    let transport = StubTransport::new(answers);
    let tokens = CountingTokens::new();
    let store = GoogleDrive::new(
        transport.clone(),
        tokens.clone(),
        DriveSettings::new("folder-1"),
    );

    (store, transport, tokens)
}
