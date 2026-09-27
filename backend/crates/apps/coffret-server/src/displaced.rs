use crate::reported::Reported;

/// A run that stopped, kept after a later run took the record from it.
///
/// Its own type rather than the run alone, because a run that is displaced is
/// a run that stopped and nothing else is: a finished run has nothing owing and
/// goes when the next one starts. So what stopped it is held here as a value
/// rather than as one of the run's states that a reader would have to hope was
/// the stopped one — which is what lets the work answer say, in its own type,
/// that every displaced run carries its refusal.
#[derive(Clone, Debug)]
pub struct Displaced<A> {
    /// The run as it stood when it stopped.
    pub run: A,
    /// What stopped it.
    pub stopped: Reported,
}
