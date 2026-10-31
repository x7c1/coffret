use coffret_device::{Phase, Step};

/// Where one run stands with a person's asking it to stop (spec: LA-35).
///
/// Three states, in the only order a run passes through them: nobody has
/// asked; somebody has asked and the run has not answered yet; or the run has
/// begun committing, from which point no stop is taken. A run that is asked
/// and then begins committing all the same — the step was reported before the
/// run heard the ask — is committing: what answers that stop is the run's real
/// result, never `cancelled`, because a commit that may have landed must not
/// be reported as cancelled (spec: PK-7, CP-1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stop {
    /// Nobody has asked, and the run has not begun committing.
    #[default]
    Open,
    /// Somebody asked, and the run has not answered yet.
    Asked,
    /// The run has begun committing, so a stop is no longer taken.
    Committing,
}

impl Stop {
    /// Asks the run to stop, and says whether the ask was taken: it is, unless
    /// the run has begun committing.
    pub(crate) fn ask(&mut self) -> bool {
        match self {
            Self::Open | Self::Asked => {
                *self = Self::Asked;
                true
            }
            Self::Committing => false,
        }
    }

    /// Hears one step of the run: a step of [`Phase::Committing`], or of
    /// anything after it, is the run beginning its commit (spec: LA-34).
    pub(crate) fn hear(&mut self, step: &Step) {
        if matches!(step.phase, Phase::Committing | Phase::RemovingCopies) {
            *self = Self::Committing;
        }
    }

    /// Whether somebody asked and the run has not answered.
    pub(crate) fn asked(self) -> bool {
        self == Self::Asked
    }
}

#[cfg(test)]
mod tests {
    use coffret_device::{Phase, Step};

    use super::Stop;

    // LA-35: a run asked before it begins committing is asked, and asking it
    // again is still one ask.
    #[test]
    fn a_stop_is_taken_until_the_run_begins_committing() {
        let mut stop = Stop::default();
        stop.hear(&Step::new(Phase::Uploading, 1, 2));
        assert!(stop.ask());
        assert!(stop.ask(), "asking twice is one ask");
        assert!(stop.asked());
    }

    // LA-35: once the run says it is committing, no stop is taken — and one
    // asked before the run heard it gives way to the commit, so what answers
    // it is the run's real result.
    #[test]
    fn a_run_that_begins_committing_takes_no_stop() {
        let mut stop = Stop::default();
        stop.hear(&Step::new(Phase::Committing, 0, 4));
        assert!(!stop.ask());
        assert_eq!(stop, Stop::Committing);

        let mut raced = Stop::default();
        assert!(raced.ask());
        raced.hear(&Step::new(Phase::Committing, 0, 4));
        assert!(!raced.asked(), "the commit is the answer, not the stop");
    }
}
