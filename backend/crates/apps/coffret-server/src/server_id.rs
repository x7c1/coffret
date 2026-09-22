//! What this process calls itself, for as long as it runs.
//!
//! A browser keeps things across answers that are only true of the process that
//! said them — a line somebody read and put away is remembered by the run
//! number it carried, and run numbers start again at 1 with every process. A
//! locked Library is unlocked by typing the Passphrase and starting the server
//! again (spec: DK-1), so a restart is an ordinary step rather than an edge
//! case, and a tab left open across one would go on hiding runs 1..N of the new
//! process: a fill's line and its declined Entries, and the one sentence that
//! says a sync did not back a file up.
//!
//! So every answer says which process it came from, and a page that sees a new
//! one throws away what it was holding. Comparing run numbers instead cannot
//! work: a run number lower than a dismissed one is also what an answer issued
//! before the dismissal looks like, and the two cannot be told apart that way.

use tracing::warn;

/// How many random bytes name one run of this server.
const RANDOM_BYTES: usize = 16;

/// The name this process answers under.
///
/// Sixteen random bytes as lowercase hex, drawn once as the state is built, and
/// deliberately nothing else. Two requirements decide that, and between them
/// they rule out every value that was already lying around:
///
/// - It has to differ across process starts, which is the whole of its job.
///   Anything the device keeps — the Library ID, the directory it is in, the
///   port, the name it was started under — is the same after a restart, so a
///   tab would go on hiding the new process's runs.
/// - It must say nothing about this machine or this Library. It rides on an
///   ordinary answer and reaches a page, and a response coffret also writes
///   somewhere that outlives the process is a record there whatever it was
///   first written for (spec: EL-1), which admits nothing a person chose. As
///   EL-5 draws that same line for Storage diagnostics — an identifier coffret
///   itself generated, never the arrangement a person made — what is left here
///   is a value this server composes out of nothing but entropy: it tells a
///   reader that two answers came from one process and nothing further — not
///   the host, not the user, not the Library, not how long the process has
///   been up, which a clock reading or a process id would each give away.
///
/// The key this server admits its callers by is drawn per process too and is
/// emphatically not this: it is a bearer credential, which is the one thing a
/// record may never hold (spec: EL-1) and which must never reach a page.
///
/// Nothing rests on it being unpredictable — it separates processes, it does
/// not authorize anything — which is what makes an entropy source that refuses
/// something to report rather than something to refuse to serve over.
pub(crate) struct ServerId(String);

impl ServerId {
    /// A name for this process that no other run of this server holds.
    pub(crate) fn drawn() -> Self {
        let mut bytes = [0_u8; RANDOM_BYTES];
        if let Err(cause) = getrandom::fill(&mut bytes) {
            // Said out loud rather than swallowed: what is left is the same
            // name every other run of a machine in this state would draw, so a
            // browser across a restart is back to hiding lines it was told to
            // hide before it — which is the failure this whole value is here to
            // end. Not a reason to refuse to serve a Library: nothing else on
            // these routes depends on it.
            warn!(
                error = %cause,
                "this server could not draw a name for itself, so an explorer \
                 left open across a restart may go on hiding what it was told \
                 to hide before it",
            );
        }
        Self(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    /// The name, as an answer spells it.
    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::{ServerId, RANDOM_BYTES};

    // The whole of what a page reads it for: two processes are two names, so a
    // tab that comes back to a restarted server can tell that it did.
    #[test]
    fn two_runs_of_the_server_are_named_apart() {
        assert_ne!(ServerId::drawn().as_str(), ServerId::drawn().as_str());
    }

    // And within one process it is one name, because a page that saw it change
    // would throw away what it is holding for no reason at all.
    #[test]
    fn one_run_answers_under_one_name() {
        let server = ServerId::drawn();
        assert_eq!(server.as_str(), server.as_str());
        assert_eq!(server.as_str().len(), RANDOM_BYTES * 2);
        assert!(
            server
                .as_str()
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "the name is lowercase hex: {:?}",
            server.as_str(),
        );
    }
}
