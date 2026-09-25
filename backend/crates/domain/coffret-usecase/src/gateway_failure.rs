use std::error;
use std::fmt;
use std::sync::Arc;

/// What a gateway had in hand when it classified a failure into the port's
/// vocabulary, handed over whole.
///
/// The port lives in the domain and cannot name a gateway type, so a failure
/// crossing it used to cross as a sentence: the gateway rendered its own error,
/// kept the string, and dropped the value. Everything the value knew that the
/// sentence did not say — an errno, a provider's structured answer, the links
/// under a client library's own error — was gone before anyone above the port
/// could ask for it. This carries the value instead, as the one shape a domain
/// type can hold without knowing what it holds: an [`error::Error`] that is
/// [`Send`] and [`Sync`], so that [`Error::source`](crate::Error) can walk on
/// into the gateway's own chain and whoever renders the chain can do so link by
/// link.
///
/// One type the variants share rather than the trait object spelled out in each
/// of them, for two reasons. The port's error is [`Clone`] — a report and a
/// retry may each hold one — and a boxed trait object is not, so the value is
/// shared behind an [`Arc`], which is a detail every variant would otherwise
/// repeat and every gateway would have to know. And it is deliberately *not*
/// itself an error: it is the place the value is kept, not a link of its own,
/// so a chain reads the port's line and then the gateway's without a wrapper's
/// sentence in between.
///
/// A field on each variant rather than a wrapper the variant holds in place of
/// its `detail`, because what sits beside it differs by variant — a status, a
/// limit's name, how long to wait — and those are what a caller matches on and
/// a diagnostic event keeps. Folding them into one shared wrapper would put
/// them one level further from the match that reads them.
#[derive(Clone)]
pub struct GatewayFailure(Arc<dyn error::Error + Send + Sync + 'static>);

impl GatewayFailure {
    /// Keeps `error` as the value behind a port failure.
    pub fn new(error: impl error::Error + Send + Sync + 'static) -> Self {
        Self(Arc::new(error))
    }

    /// The value the gateway handed over, as a link in a chain.
    pub fn as_error(&self) -> &(dyn error::Error + 'static) {
        self.0.as_ref()
    }
}

impl fmt::Debug for GatewayFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The value's own `Debug`, with nothing of this holder around it: the
        // holder says nothing a reader of a `{:?}` is after.
        fmt::Debug::fmt(&self.0, f)
    }
}
