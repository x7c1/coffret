//! The whole object a run prints under `--json`.

use std::borrow::Cow;
use std::path::Path;

use serde::Serialize;

use coffret_device::Findings;

use crate::report::Report;

use super::{said_path, Answer, Failure, Found, Ran, VERSION};

/// The whole object a run prints under `--json`.
#[derive(Serialize)]
pub struct Document<'a> {
    version: u32,
    command: Option<&'static str>,
    exit_status: u8,
    log: Option<String>,
    answer: Option<&'a Answer>,
    error: Option<Failure>,
    findings: Cow<'a, [Found]>,
}

impl<'a> Document<'a> {
    /// What a run of `command` that succeeded answers with.
    pub fn succeeded(command: &'static str, log: Option<&Path>, ran: &'a Ran) -> Self {
        Self {
            version: VERSION,
            command: Some(command),
            exit_status: ran.report.exit_status(),
            log: log.map(said_path),
            answer: Some(&ran.answer),
            error: None,
            findings: Cow::Borrowed(&ran.findings),
        }
    }

    /// What a run that failed with `error` answers with.
    ///
    /// `command` is `None` where the run failed before a subcommand was read
    /// at all. The findings are the Keyring repairs the run performed before it
    /// failed, which stand whatever became of the run (spec: KL-15).
    pub fn failed(
        command: Option<&'static str>,
        log: Option<&Path>,
        error: Failure,
        repaired: &Findings,
    ) -> Self {
        Self {
            version: VERSION,
            command,
            exit_status: Report::FAILED,
            log: log.map(said_path),
            answer: None,
            error: Some(error),
            findings: Cow::Owned(repaired.iter().map(Found::from).collect()),
        }
    }

    /// Puts the object on standard output, on one line.
    pub fn print(&self) {
        println!("{}", self.rendered());
    }

    /// The object as it is printed.
    pub(super) fn rendered(&self) -> String {
        // Nothing here is a map with a key that is not a string, and nothing
        // implements `Serialize` by hand, which are the two ways serializing
        // to a string can fail.
        serde_json::to_string(self).expect("every answer type serializes to JSON")
    }
}
