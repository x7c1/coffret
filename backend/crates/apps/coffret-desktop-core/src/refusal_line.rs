use coffret_device::{Error, Redacted};

/// What a refused open or unlock tells the person, as a Passphrase window
/// shows it (the Tauri app's own, or the page the door serves): one sentence
/// they can act on.
///
/// The window is small and the chain an error carries is written for a
/// terminal and a log — the wrong Passphrase alone comes with the Master Key
/// file's full path and the format layer's own words underneath — so the window
/// shows less than the chain and the caller logs what of it a log may carry
/// ([`refusal_evidence()`]). The refusal a person causes is said plainly; every
/// other keeps the outermost sentence of its chain, which still says what
/// happened (another server holds the Library, nothing could listen) without
/// the causes appended. That line is a lower-case, unstopped clause written to
/// be joined to its causes, so it is capitalised and stopped like the page's
/// own sentences.
pub fn refusal_line(refused: &anyhow::Error) -> String {
    let wrong_passphrase = refused.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<Error>(),
            Some(Error::MasterKeyNotUnlocked { .. })
        )
    });
    if wrong_passphrase {
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
/// (spec: EL-1). So the event takes the first link it has a vocabulary for —
/// a device failure through [`Redacted`], which renders its own causes under
/// it, or an `io::Error` by its kind — and stops at a fixed identity where the
/// chain has neither, rather than walking it (spec: EL-3, EL-4).
pub fn refusal_evidence(refused: &anyhow::Error) -> String {
    for cause in refused.chain() {
        if let Some(error) = cause.downcast_ref::<Error>() {
            return error.redacted();
        }
        if let Some(error) = cause.downcast_ref::<std::io::Error>() {
            return format!("Io(kind={:?})", error.kind());
        }
    }
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

    fn not_unlocked() -> Error {
        Error::MasterKeyNotUnlocked {
            path: PathBuf::from("/state/libraries/home/master-key.cfmk"),
            cause: FormatError::AuthenticationFailed,
        }
    }

    #[test]
    fn a_wrong_passphrase_is_said_plainly_and_without_the_path() {
        let line = refusal_line(&anyhow::Error::new(not_unlocked()));
        assert_eq!(line, "That is not this Library's Passphrase.");
        assert!(!line.contains("master-key.cfmk"));
    }

    #[test]
    fn a_wrong_passphrase_is_recognised_under_a_context() {
        let refused = Err::<(), _>(not_unlocked())
            .context("the Library could not be opened")
            .unwrap_err();
        assert_eq!(
            refusal_line(&refused),
            "That is not this Library's Passphrase."
        );
    }

    #[test]
    fn any_other_refusal_keeps_its_outermost_sentence_only() {
        let refused = Err::<(), _>(anyhow::anyhow!("address already in use"))
            .context("nothing could listen at 127.0.0.1:0")
            .unwrap_err();
        assert_eq!(
            refusal_line(&refused),
            "Nothing could listen at 127.0.0.1:0."
        );
    }

    #[test]
    fn the_log_is_told_the_device_failure_without_the_path() {
        let refused = Err::<(), _>(not_unlocked())
            .context("the Library could not be opened")
            .unwrap_err();
        let evidence = refusal_evidence(&refused);
        assert_eq!(evidence, not_unlocked().redacted());
        assert!(!evidence.contains("master-key.cfmk"));
        assert!(!evidence.contains("could not be opened"));
    }

    #[test]
    fn the_log_is_told_an_io_failure_by_its_kind_only() {
        let refused = Err::<(), _>(std::io::Error::from(std::io::ErrorKind::AddrInUse))
            .context("nothing could listen at 127.0.0.1:0")
            .unwrap_err();
        assert_eq!(refusal_evidence(&refused), "Io(kind=AddrInUse)");
    }

    #[test]
    fn the_log_is_told_nothing_of_a_chain_it_has_no_vocabulary_for() {
        let refused = anyhow::anyhow!("the Library called \"home\" is no longer this one");
        assert_eq!(refusal_evidence(&refused), "Untyped");
    }

    #[test]
    fn a_sentence_already_stopped_is_not_stopped_twice() {
        assert_eq!(as_sentence("nothing is open."), "Nothing is open.");
        assert_eq!(as_sentence(""), "");
    }
}
