//! What a run answers a script with, under `--json`.
//!
//! The text a run prints is written for a person, and a script that read its
//! facts out of those sentences broke whenever one of them was reworded. So a
//! run asked for `--json` puts one JSON object on standard output when it
//! finishes, whether it succeeded or failed, and nothing else there: progress,
//! advice, and the line naming the log file stay on standard error, where they
//! are for a person watching.
//!
//! The object is built from the types in this module and never from the device
//! layer's own. That is what makes it a contract: a field renamed or a variant
//! reshaped in a flow's outcome changes nothing here until somebody changes
//! this module on purpose, and the golden cases beside it are what say
//! that such a change is one. Its shape is [`SHAPE`], which is what `--help`
//! prints for the flag.
//!
//! What it may carry is what the text form already puts in front of whoever
//! ran the command, and no more: the Recovery Code only from `init` and
//! `recovery-code`, which print it on standard output as text too; no
//! Passphrase, token or client secret ever; and no local path the text form
//! does not print. The answer is a response to whoever asked for the run, as
//! the text is, rather than a record written down somewhere (spec: EL-1).

use std::path::Path;

use serde::Serialize;

use coffret_device::{CommitOutcome, EntryPath};

mod authorized;
pub use authorized::Authorized;

mod created;
pub use created::Created;

mod document;
pub use document::Document;

mod failure;
pub use failure::Failure;

mod fetched;
pub use fetched::Fetched;

mod fetched_entry;
pub use fetched_entry::FetchedEntry;

mod form;
pub use form::Form;

mod found;
use found::Found;

mod frozen;
pub use frozen::Frozen;

mod joined;
pub use joined::Joined;

mod mapped;
pub use mapped::Mapped;

mod mappings;
pub use mappings::Mappings;

mod ran;
pub use ran::Ran;

mod recovery_code;
pub use recovery_code::RecoveryCode;

mod storage;
use storage::Storage;

mod synced;
pub use synced::Synced;

#[cfg(test)]
mod tests;

/// The version of the shape below, which a script can check before it reads
/// anything else. It changes only when a field is removed or changes meaning;
/// a field added beside the rest does not change it.
pub const VERSION: u32 = 1;

/// What `--help` says about `--json`: the whole shape, in one place.
///
/// Kept beside the types it describes so that the two are changed together;
/// the golden cases hold the types to it.
pub const SHAPE: &str = "\
Answer in one JSON object on standard output, printed when the command
finishes, whether it succeeded or failed; nothing else is printed there.
Progress, advice and the log file's name stay on standard error. Without
this flag, the output is the text a person reads.

Every answer carries:

  version      1, the version of this shape
  command      the subcommand, e.g. \"sync\"
  exit_status  0, 1 or 2, the status the run exits with
  log          the log file this run wrote to, or null
  answer       what the command answered, or null where it failed
  error        why it failed, or null where it succeeded
  findings     one object per finding: kind, needs_attention, said (the
               line the text form prints) and the facts of that kind,
               e.g. path and reason for \"surfaced\"

A finding's kind is \"surfaced\", \"unavailable_root\", \"refused_root\",
\"key_lost_container\", \"degraded_keyring\", \"keyring_repaired\",
\"settled\", \"untrashed_removal\" or \"checkpoint_failed\". Its reason,
where it has one, is spelled as the explorer's server spells it, e.g.
\"DeletedLocally\" or \"ForeignFile\", not in snake case.

error is kind, message (the sentence the text form prints), advice (the
lines printed after it), and found and supported for \"unsupported_schema\".
A failure the explorer's server already has a name for keeps that name
(\"storage\", \"epoch\", \"refused_root\", \"unmapped\", \"bad_path\"); every
other is named after the error's variant in snake case, e.g.
\"not_authorized\", \"unauthenticated\" or \"no_such_local_root\". What was
typed wrongly and refused before the command ran is \"usage\", with
command and log null. Refused once it has started: flags that leave a
provider short are \"flags_missing\", and a client secret variable that
is empty or not Unicode is \"empty_client_secret\" or
\"client_secret_not_unicode\". \"other\" is a failure raised by nothing
this command knows the kinds of.

answer, per command:

  init           library, library_id, storage, account, consent_asked,
                 recovery_code
  join           library, library_id, storage, account, consent_asked,
                 found_on_storage (\"the_library\" or \"nothing_yet\")
  recovery-code  library, recovery_code
  authorize      consent_asked
  map            prefix, local_root, replaced, marker (\"written\",
                 \"adopted\" or \"reset\")
  mappings       mappings (prefix and local_root each; the root's prefix
                 is null), refused (an error, where the Index could not be
                 opened and the mappings were read out of the file)
  sync           added, replaced, unchanged, committed_head, mappings
  freeze         packs, entries, absorbed, packed_already, committed_head,
                 mappings
  fetch          fetched, containers, skipped, mappings; with --entry,
                 entry (\"placed\", \"already_present\" or \"surfaced\"),
                 fetched, skipped and alongside (the other files the
                 parcels read placed)

storage is {\"provider\": \"s3\", \"bucket\", \"prefix\"} or {\"provider\":
\"drive\", \"folder_id\"}. committed_head is the generation committed, or
null where nothing was.";

/// A local path, in the one spelling the text form prints it in.
fn said_path(path: &Path) -> String {
    path.display().to_string()
}

/// A mapping's prefix, `None` for the Library root.
fn prefix_said(prefix: Option<&EntryPath>) -> Option<String> {
    prefix.map(|prefix| prefix.as_str().to_owned())
}

/// What a command answered, one variant per command.
///
/// Untagged, because the command that answered is beside it in
/// [`Document`]: each variant is the object `answer` holds.
#[derive(Serialize)]
#[serde(untagged)]
pub enum Answer {
    /// `init`.
    Created(Created),
    /// `join`.
    Joined(Joined),
    /// `recovery-code`.
    RecoveryCode(RecoveryCode),
    /// `authorize`.
    Authorized(Authorized),
    /// `map`.
    Mapped(Mapped),
    /// `mappings`.
    Mappings(Mappings),
    /// `sync`.
    Synced(Synced),
    /// `freeze`.
    Frozen(Frozen),
    /// `fetch`.
    Fetched(Fetched),
    /// `fetch --entry`.
    FetchedEntry(FetchedEntry),
}

/// The generation a run committed, or `None` where it committed nothing.
fn committed_head(commit: Option<&CommitOutcome>) -> Option<u64> {
    commit.map(|commit| commit.record.generation().get())
}
