use coffret_device::Redacted;

/// A failure this server observed with nothing below it to keep, named by the
/// identity it is filed under.
///
/// For the states that are not a value from a lower layer at all — no home
/// directory to start a listing at — and whose identity is therefore the whole
/// of what an event is told.
pub(crate) struct Named(pub(crate) &'static str);

impl Redacted for Named {
    fn redacted(&self) -> String {
        self.0.to_owned()
    }
}
