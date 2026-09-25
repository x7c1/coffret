//! How a flow asks for the Passphrase of a Library that already references an
//! account, when the Passphrase it was given opens none of them.

use std::fmt;
use std::sync::Arc;

use coffret_model::Passphrase;

use crate::error::Result;

/// What a flow putting a Library on an account this device holds asks for the
/// Passphrase of one Library that already references the account, named by its
/// device-local name (spec: SA-9).
///
/// An account's key is kept only in the envelopes of the Libraries that
/// reference it, each under its own Master Key. The new Library's own
/// Passphrase is tried against them first, which is all it takes where one
/// Passphrase keeps every Library on the device; where it opens none of them,
/// this is asked — once, for the one Library it names — rather than consenting
/// again and keeping a second cache for the same account, the one shape SA-8
/// forbids.
///
/// [`unasked`](Self::unasked) is a caller that cannot ask anybody, a script
/// reading its one Passphrase from standard input: the flow refuses instead,
/// naming the Library whose Passphrase would open the account.
#[derive(Clone, Default)]
pub struct ReferencingPassphrase(Option<Arc<Ask>>);

/// The question itself: a Library's device-local name in, its Passphrase out.
type Ask = dyn Fn(&str) -> Result<Passphrase> + Send + Sync;

impl ReferencingPassphrase {
    /// A caller with nobody to ask.
    pub fn unasked() -> Self {
        Self(None)
    }

    /// A caller that asks `ask`, handing it the Library's device-local name.
    pub fn asking(ask: impl Fn(&str) -> Result<Passphrase> + Send + Sync + 'static) -> Self {
        Self(Some(Arc::new(ask)))
    }

    /// The Passphrase of the Library called `library`, or `None` where there is
    /// nobody to ask.
    pub(crate) fn ask(&self, library: &str) -> Option<Result<Passphrase>> {
        self.0.as_ref().map(|ask| ask(library))
    }
}

impl fmt::Debug for ReferencingPassphrase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.0 {
            Some(_) => "ReferencingPassphrase(asking)",
            None => "ReferencingPassphrase(unasked)",
        })
    }
}
