use coffret_device::Redacted;

use crate::Error;

/// What a refused open or unlock tells the person, as the Passphrase window
/// shows it (the page the door serves): one sentence they can act on.
///
/// The window is small and the chain an error carries is written for a
/// terminal and a log — the wrong Passphrase alone comes with the Master Key
/// file's full path and the format layer's own words underneath — so the window
/// shows less than the chain and the caller logs what of it a log may carry
/// ([`refusal_evidence()`]). The refusal a person causes is said plainly, and
/// so is a stored Master Key refused before its Passphrase was checked, lest it
/// be taken for a wrong Passphrase; every other keeps the outermost sentence of
/// its chain, which still says what happened (another server holds the
/// Library, nothing could listen) without the causes appended. That line is a
/// lower-case, unstopped clause written to be joined to its causes, so it is
/// capitalised and stopped like the page's own sentences.
pub fn refusal_line(refused: &Error) -> String {
    let device_refused = |is: fn(&coffret_device::Error) -> bool| match refused {
        Error::Device(error) => is(error),
        // The server says what it refused in a chain of its own, with the
        // device's failure somewhere under its sentences.
        Error::Server(chain) => chain
            .chain()
            .filter_map(|cause| cause.downcast_ref::<coffret_device::Error>())
            .any(is),
        _ => false,
    };
    // Checked first, and never read as a wrong Passphrase: the stored form was
    // refused before the Passphrase was checked at all (spec: KD-9).
    if device_refused(is_too_costly) {
        "This Library's stored Master Key asks for more work than coffret allows; \
         the Passphrase was not checked."
            .to_owned()
    } else if device_refused(is_wrong_passphrase) {
        "That is not this Library's Passphrase.".to_owned()
    } else {
        as_sentence(&refused.to_string())
    }
}

/// What the log is told of a refusal [`refusal_line()`] answered: the
/// device's own failure as its log-safe rendering, never the chain's words.
///
/// The chain is the person's answer, written with the Library's name and the
/// Master Key file's path in it, and a diagnostic event may carry neither
/// (spec: EL-1). So the event takes the failure it has a vocabulary for — a
/// device failure through [`Redacted`], which renders its own causes under it,
/// or an `io::Error` by its kind — and a fixed identity where there is neither
/// (spec: EL-3, EL-4).
pub fn refusal_evidence(refused: &Error) -> String {
    match refused {
        Error::Device(error) | Error::LibrariesUnlisted { cause: error } => error.redacted(),
        Error::Io { cause, .. } => io_evidence(cause),
        Error::Host(coffret_explorer_host::Error::Unbound(cause)) => io_evidence(cause),
        // The server's chain is the one cause kept untyped, so it is searched
        // for the first link the event has a vocabulary for, and not walked
        // further.
        Error::Server(chain) => chain
            .chain()
            .find_map(|cause| {
                if let Some(error) = cause.downcast_ref::<coffret_device::Error>() {
                    return Some(error.redacted());
                }
                cause.downcast_ref::<std::io::Error>().map(io_evidence)
            })
            .unwrap_or_else(untyped),
        Error::AlreadyStarted
        | Error::IdleMinutesNotText
        | Error::IdleMinutesNotWhole { .. }
        | Error::IdleMinutesBelowOne { .. }
        | Error::LogNotStarted { .. }
        | Error::NothingOpen
        | Error::NothingToUnlock
        | Error::NotDrawn { .. }
        | Error::DropHandleNotDrawn { .. }
        | Error::Host(_) => untyped(),
    }
}

/// Whether the device refused because the Passphrase does not open the
/// Library's Master Key.
fn is_wrong_passphrase(error: &coffret_device::Error) -> bool {
    matches!(error, coffret_device::Error::MasterKeyNotUnlocked { .. })
}

/// Whether the device refused the Library's stored Master Key for recording an
/// Argon2id cost above what coffret accepts.
fn is_too_costly(error: &coffret_device::Error) -> bool {
    matches!(
        error,
        coffret_device::Error::MasterKeyCostAboveCeiling { .. }
    )
}

/// An `io::Error` as the log may carry it: its kind alone.
fn io_evidence(error: &std::io::Error) -> String {
    format!("Io(kind={:?})", error.kind())
}

/// The fixed identity of a refusal the log has no vocabulary for.
fn untyped() -> String {
    "Untyped".to_owned()
}

/// `clause` with its first letter in capitals and a full stop after it, unless
/// it already ends in one.
fn as_sentence(clause: &str) -> String {
    let mut chars = clause.chars();
    let mut sentence: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => return String::new(),
    };
    if !sentence.ends_with(['.', '!', '?']) {
        sentence.push('.');
    }
    sentence
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::Context;
    use coffret_device::FormatError;

    use super::*;
    use crate::IoStep;

    fn not_unlocked() -> coffret_device::Error {
        coffret_device::Error::MasterKeyNotUnlocked {
            path: PathBuf::from("/state/libraries/home/master-key.cfmk"),
            cause: FormatError::AuthenticationFailed,
        }
    }

    /// A refusal the server makes, under a sentence of its own.
    fn from_server<E>(cause: E, context: &'static str) -> Error
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Error::Server(Err::<(), _>(cause).context(context).unwrap_err())
    }

    #[test]
    fn a_wrong_passphrase_is_said_plainly_and_without_the_path() {
        let line = refusal_line(&Error::Device(not_unlocked()));
        assert_eq!(line, "That is not this Library's Passphrase.");
        assert!(!line.contains("master-key.cfmk"));
    }

    #[test]
    fn a_wrong_passphrase_is_recognised_under_a_context() {
        let refused = from_server(not_unlocked(), "the Library could not be opened");
        assert_eq!(
            refusal_line(&refused),
            "That is not this Library's Passphrase."
        );
    }

    fn too_costly() -> coffret_device::Error {
        coffret_device::Error::MasterKeyCostAboveCeiling {
            path: PathBuf::from("/state/libraries/home/master-key.cfmk"),
            cause: FormatError::Argon2CostAboveCeiling {
                parameter: "iterations",
                value: u32::MAX,
                ceiling: 8,
            },
        }
    }

    // KD-9: a stored form refused for its cost was refused before the
    // Passphrase was checked, so it is never told as a wrong Passphrase.
    #[test]
    fn a_too_costly_stored_key_is_not_told_as_a_wrong_passphrase() {
        let expected = "This Library's stored Master Key asks for more work than coffret \
                        allows; the Passphrase was not checked.";
        let line = refusal_line(&Error::Device(too_costly()));
        assert_eq!(line, expected);
        assert!(!line.contains("master-key.cfmk"));
        let refused = from_server(too_costly(), "the Library could not be opened");
        assert_eq!(refusal_line(&refused), expected);
        assert_eq!(
            refusal_evidence(&Error::Device(too_costly())),
            too_costly().redacted()
        );
    }

    #[test]
    fn any_other_refusal_keeps_its_outermost_sentence_only() {
        let refused = Error::Server(
            Err::<(), _>(anyhow::anyhow!("address already in use"))
                .context("nothing could listen at 127.0.0.1:0")
                .unwrap_err(),
        );
        assert_eq!(
            refusal_line(&refused),
            "Nothing could listen at 127.0.0.1:0."
        );
        let refused = Error::Io {
            step: IoStep::ExplorerBind,
            cause: std::io::Error::from(std::io::ErrorKind::AddrInUse),
        };
        assert_eq!(
            refusal_line(&refused),
            "Nothing could listen for the explorer on 127.0.0.1."
        );
    }

    #[test]
    fn the_log_is_told_the_device_failure_without_the_path() {
        let refused = from_server(not_unlocked(), "the Library could not be opened");
        let evidence = refusal_evidence(&refused);
        assert_eq!(evidence, not_unlocked().redacted());
        assert!(!evidence.contains("master-key.cfmk"));
        assert!(!evidence.contains("could not be opened"));
        assert_eq!(
            refusal_evidence(&Error::Device(not_unlocked())),
            not_unlocked().redacted()
        );
    }

    #[test]
    fn the_log_is_told_an_io_failure_by_its_kind_only() {
        let refused = from_server(
            std::io::Error::from(std::io::ErrorKind::AddrInUse),
            "nothing could listen at 127.0.0.1:0",
        );
        assert_eq!(refusal_evidence(&refused), "Io(kind=AddrInUse)");
        let refused = Error::Io {
            step: IoStep::ExplorerBind,
            cause: std::io::Error::from(std::io::ErrorKind::AddrInUse),
        };
        assert_eq!(refusal_evidence(&refused), "Io(kind=AddrInUse)");
    }

    #[test]
    fn the_log_is_told_nothing_of_a_chain_it_has_no_vocabulary_for() {
        let refused = Error::Server(anyhow::anyhow!(
            "the Library called \"home\" is no longer this one"
        ));
        assert_eq!(refusal_evidence(&refused), "Untyped");
        assert_eq!(refusal_evidence(&Error::NothingOpen), "Untyped");
    }

    #[test]
    fn the_whole_chain_reads_as_anyhow_renders_it() {
        let refused = from_server(
            std::io::Error::other("address already in use"),
            "nothing could listen at 127.0.0.1:0",
        );
        let Error::Server(chain) = &refused else {
            unreachable!("built as the server's");
        };
        assert_eq!(refused.chain_line(), format!("{chain:#}"));
    }

    #[test]
    fn a_sentence_already_stopped_is_not_stopped_twice() {
        assert_eq!(as_sentence("nothing is open."), "Nothing is open.");
        assert_eq!(as_sentence(""), "");
    }
}
