use crate::error::Error;

/// How a renewal that did not end in a grant ended, as far as a screen tells
/// the endings apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unrenewed {
    /// The person declined on the consent page, and the provider said so
    /// (spec: SA-2).
    Refused,
    /// Nobody came back to the loopback listener before the flow stopped
    /// waiting.
    TimedOut,
    /// Anything else: a grant wider than asked for (spec: SA-4), one without a
    /// refresh token, a listener that would not bind, a token endpoint that
    /// refused. The log says which.
    Failed,
}

impl Unrenewed {
    /// Which of the three `error` is.
    pub fn of(error: &Error) -> Self {
        match error {
            Error::Drive { cause } => match cause.as_ref() {
                google_drive_store::Error::ProviderRefusedAuthorization { .. } => Self::Refused,
                google_drive_store::Error::RedirectTimedOut { .. } => Self::TimedOut,
                _ => Self::Failed,
            },
            _ => Self::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Unrenewed;
    use crate::error::Error;

    fn drive(cause: google_drive_store::Error) -> Error {
        Error::Drive {
            cause: Box::new(cause),
        }
    }

    // The two endings a person caused, told apart from everything else, because
    // they are what a screen offers the button again with a reason for.
    #[test]
    fn a_declined_and_an_unanswered_consent_are_told_apart_from_a_failure() {
        assert_eq!(
            Unrenewed::of(&drive(
                google_drive_store::Error::ProviderRefusedAuthorization {
                    refusal: "access_denied".to_owned(),
                }
            )),
            Unrenewed::Refused,
        );
        assert_eq!(
            Unrenewed::of(&drive(google_drive_store::Error::RedirectTimedOut {
                after: Duration::from_secs(300),
            })),
            Unrenewed::TimedOut,
        );
        assert_eq!(
            Unrenewed::of(&drive(google_drive_store::Error::GrantWithoutRefreshToken)),
            Unrenewed::Failed,
        );
        assert_eq!(
            Unrenewed::of(&Error::NotADriveLibrary {
                name: "albums".to_owned(),
            }),
            Unrenewed::Failed,
        );
    }
}
