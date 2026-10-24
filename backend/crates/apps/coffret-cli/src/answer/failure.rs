//! Why a run failed, as the JSON answer carries it.

use std::error;

use serde::Serialize;

use coffret_device::{
    CommitError, CommitFailure, Error, FormatError, IndexError, ModelError, RenameError,
    RenameRefusal, StorageError,
};

use crate::fetch::UnmappedEntry;
use crate::interrupted::Interrupted;
use crate::refusal::Refusal;
use crate::report;

/// A failure: a stable kind to branch on, and the sentences the text form
/// prints for it.
#[derive(Clone, Debug, Serialize)]
pub struct Failure {
    kind: &'static str,
    message: String,
    advice: Vec<String>,
    /// The layout the Index file is at, for `unsupported_schema`.
    #[serde(skip_serializing_if = "Option::is_none")]
    found: Option<i64>,
    /// The layout this build reads, for `unsupported_schema`.
    #[serde(skip_serializing_if = "Option::is_none")]
    supported: Option<i64>,
}

impl Failure {
    /// What a run that failed with `error` answers with.
    ///
    /// The message and the advice are the lines [`report::failed`] prints on
    /// standard error, so the two forms cannot say different things.
    pub fn of(error: &anyhow::Error) -> Self {
        let mut said = report::failed(error).said.into_iter();
        let message = said.next().unwrap_or_default();
        let classified = classified(error.chain());
        Self {
            kind: classified.kind,
            message,
            advice: said.collect(),
            found: classified.found,
            supported: classified.supported,
        }
    }

    /// A refusal a run met before it knew which command it was running, or
    /// that the argument parser met: `kind` and the one sentence it said.
    pub fn plain(kind: &'static str, message: String) -> Self {
        Self {
            kind,
            message,
            advice: Vec::new(),
            found: None,
            supported: None,
        }
    }

    /// The status a run that failed with this exits with: [`Report::INTERRUPTED`]
    /// for a run the person stopped with Ctrl-C, and [`Report::FAILED`] for
    /// every other failure.
    ///
    /// [`Report::INTERRUPTED`]: report::Report::INTERRUPTED
    /// [`Report::FAILED`]: report::Report::FAILED
    pub fn exit_status(&self) -> u8 {
        match self.kind {
            INTERRUPTED => report::Report::INTERRUPTED,
            _ => report::Report::FAILED,
        }
    }

    /// Why the Index file was not opened, where a listing read the mappings
    /// out of it anyway: the refusal is not the run's failure, and is said
    /// beside what the run answered.
    pub fn of_index(refusal: &IndexError) -> Self {
        let classified = classified(std::iter::once(refusal as &(dyn error::Error + 'static)));
        Self {
            kind: classified.kind,
            message: refusal.to_string(),
            advice: Vec::new(),
            found: classified.found,
            supported: classified.supported,
        }
    }
}

/// The kind of a run the person stopped with Ctrl-C, which is the one failure
/// that exits with a status of its own.
const INTERRUPTED: &str = "interrupted";

/// A failure's kind, and the two numbers the one kind that carries any has.
struct Classified {
    kind: &'static str,
    found: Option<i64>,
    supported: Option<i64>,
}

impl Classified {
    fn kind(kind: &'static str) -> Self {
        Self {
            kind,
            found: None,
            supported: None,
        }
    }
}

/// What a chain of errors is, for a caller to branch on.
///
/// First the kinds a caller acts on whichever layer carried them, found
/// anywhere in the chain: a one-Entry fetch no mapping reaches, an Index file
/// at a layout this build does not read, a Journal record of a schema this
/// build is too old or too new for, a Drive Library with no grant,
/// credentials Storage rejected, and an epoch this device holds no key for.
/// Each is answered by one gesture — `coffret map`, deleting the Index file,
/// `coffret authorize`, the credentials, re-enrolment — whatever flow met it.
/// A Library listing two control objects under one name is found the same way:
/// no flow reads or writes such a Library, so what a caller does about it is
/// the same whichever flow stopped (spec: FM-12).
///
/// Then the outermost error this crate knows the type of, named by its variant
/// — except where the variant is a flow's wrapper around Storage not coming
/// through, which is `storage`, the kind the explorer's server gives it. What
/// is left is `other`: an error no layer this crate knows the types of raised.
fn classified<'a>(
    chain: impl Iterator<Item = &'a (dyn error::Error + 'static)> + Clone,
) -> Classified {
    for link in chain.clone() {
        if link.is::<UnmappedEntry>() {
            return Classified::kind("unmapped");
        }
        // Stopped by the person at the terminal, whatever it was doing: what
        // it left is the next sync's to settle.
        if link.is::<Interrupted>() {
            return Classified::kind(INTERRUPTED);
        }
        if let Some(IndexError::UnsupportedSchema { found, supported }) = link.downcast_ref() {
            return Classified {
                kind: "unsupported_schema",
                found: Some(*found),
                supported: Some(*supported),
            };
        }
        // A record this build is too old to apply, whichever flow's catch-up
        // met it: what a person does about it is install a newer build, and
        // nothing else (spec: FM-9, FM-15).
        if let Some(FormatError::JournalRecordNeedsNewerBuild { .. }) = link.downcast_ref() {
            return Classified::kind("newer_build_needed");
        }
        // The other side of the same line: a record of a schema older than
        // this build reads. The Library is not broken, it predates this
        // build's format, and what a person does about it is make it again
        // (spec: FM-15).
        if let Some(FormatError::UnsupportedJournalRecordSchema { .. }) = link.downcast_ref() {
            return Classified::kind("library_predates_format");
        }
        // A rename refused before anything was written is the request's own,
        // and each refusal is put right differently.
        if let Some(RenameError::Refused(refusal)) = link.downcast_ref() {
            return Classified::kind(rename_refusal_kind(refusal));
        }
        if let Some(Error::NotAuthorized { .. }) = link.downcast_ref() {
            return Classified::kind("not_authorized");
        }
        if let Some(StorageError::Unauthenticated { .. }) = link.downcast_ref() {
            return Classified::kind("unauthenticated");
        }
        // A commit's failure stands in the chain for the error it carries,
        // saying it and handing on what is under it, so the epoch is looked
        // for on both.
        let commit = link
            .downcast_ref::<CommitFailure>()
            .map(|failure| failure.error.as_ref())
            .or_else(|| link.downcast_ref::<CommitError>());
        match commit {
            Some(CommitError::EpochActivated { .. }) => return Classified::kind("epoch"),
            Some(CommitError::DuplicateControlObject { .. }) => {
                return Classified::kind("duplicate_control_object")
            }
            _ => {}
        }
    }

    let storage_below = chain.clone().any(|link| link.is::<StorageError>());
    for link in chain {
        // A secret the shell did not give is the device layer's one line over
        // the shell's own refusal, and the refusal is what a script acts on:
        // an empty Passphrase and two that differ, or an empty Recovery Code
        // and one too long, are put right differently, and the wrapper tells
        // none of them apart. Only the refusal the wrapper carries is looked
        // at, not one met elsewhere in the chain.
        if let Some(refusal) = refusal_not_given(link) {
            return Classified::kind(shell_kind(refusal));
        }
        if let Some(error) = link.downcast_ref::<Error>() {
            return Classified::kind(device_kind(error, storage_below));
        }
        if link.is::<StorageError>() {
            return Classified::kind("storage");
        }
        if let Some(error) = link.downcast_ref::<coffret_shell::Error>() {
            return Classified::kind(shell_kind(error));
        }
        if link.is::<ModelError>() {
            return Classified::kind("bad_path");
        }
        if let Some(refusal) = link.downcast_ref::<Refusal>() {
            return Classified::kind(refusal_kind(refusal));
        }
    }
    Classified::kind("other")
}

/// The shell's refusal `link` carries, where `link` is the device layer saying
/// a Passphrase or a Recovery Code was not given.
fn refusal_not_given<'a>(
    link: &'a (dyn error::Error + 'static),
) -> Option<&'a coffret_shell::Error> {
    match link.downcast_ref::<Error>()? {
        Error::PassphraseNotGiven { cause } | Error::RecoveryCodeNotGiven { cause } => {
            cause.downcast_ref::<coffret_shell::Error>()
        }
        _ => None,
    }
}

/// A device error by its variant, snake-cased.
///
/// Every variant is listed rather than left to a wildcard, so that a variant
/// added to the device layer has to be given its name here on purpose — the
/// name is part of the contract, and one made up by a fallback would be a
/// name nobody chose.
fn device_kind(error: &Error, storage_below: bool) -> &'static str {
    match error {
        // The flows, and the steps that wrap a flow's failure: where Storage
        // did not come through underneath, that is what a caller acts on, and
        // the flow that met it says nothing more a script could use.
        Error::Sync { .. } if storage_below => "storage",
        Error::Freeze { .. } if storage_below => "storage",
        Error::Delete { .. } if storage_below => "storage",
        Error::Rename { .. } if storage_below => "storage",
        Error::Fetch { .. } if storage_below => "storage",
        Error::CatchUp { .. } if storage_below => "storage",
        Error::RemoteHeadNotRead { .. } if storage_below => "storage",
        Error::FirstHead { .. } if storage_below => "storage",
        Error::Sync { .. } => "sync",
        Error::Freeze { .. } => "freeze",
        Error::Delete { .. } => "delete",
        Error::Rename { .. } => "rename",
        Error::Fetch { .. } => "fetch",
        Error::CatchUp { .. } => "catch_up",
        Error::RemoteHeadNotRead { .. } => "remote_head_not_read",
        Error::FirstHead { .. } => "first_head",
        // The name the explorer's server gives the same refusal of a mapped
        // folder (spec: EP-13).
        Error::RootRefused(_) => "refused_root",
        Error::InvalidLibraryName { .. } => "invalid_library_name",
        Error::NoStateDirectory => "no_state_directory",
        Error::LibraryExists { .. } => "library_exists",
        Error::NoSuchLibrary { .. } => "no_such_library",
        Error::Local(_) => "local",
        Error::MalformedSettings { .. } => "malformed_settings",
        Error::UnsupportedSettingsVersion { .. } => "unsupported_settings_version",
        Error::UnencodableSettings { .. } => "unencodable_settings",
        Error::MasterKeyNotUnlocked { .. } => "master_key_not_unlocked",
        Error::KeyMaterial { .. } => "key_material",
        Error::ServerKeyNotDrawn { .. } => "server_key_not_drawn",
        Error::LibraryAlreadyServed { .. } => "library_already_served",
        Error::MalformedStoragePrefix { .. } => "malformed_storage_prefix",
        Error::Index { .. } => "index",
        Error::Drive { .. } => "drive",
        Error::NotADriveLibrary { .. } => "not_a_drive_library",
        Error::NotAuthorized { .. } => "not_authorized",
        Error::InvalidAccountName { .. } => "invalid_account_name",
        Error::AccountNameRequired { .. } => "account_name_required",
        Error::NoAccountReachesFolder => "no_account_reaches_folder",
        Error::ClientMismatch(_) => "client_mismatch",
        Error::UnreadableAccountEnvelope { .. } => "unreadable_account_envelope",
        Error::AccountNotOpened { .. } => "account_not_opened",
        Error::PromotionNeedsName { .. } => "promotion_needs_name",
        Error::NoSuchAccount { .. } => "no_such_account",
        Error::AccountFixed { .. } => "account_fixed",
        Error::MalformedMappingPrefix { .. } => "malformed_mapping_prefix",
        Error::NoSuchLocalRoot { .. } => "no_such_local_root",
        Error::ManagementAreaNotADirectory { .. } => "management_area_not_a_directory",
        Error::ManagementAreaIncomplete { .. } => "management_area_incomplete",
        Error::ManagementAreaFolded { .. } => "management_area_folded",
        Error::MarkerNotARegularFile { .. } => "marker_not_a_regular_file",
        Error::MarkerMalformed { .. } => "marker_malformed",
        Error::RootUnvouched { .. } => "root_unvouched",
        Error::RootMarkerNotDrawn { .. } => "root_marker_not_drawn",
        Error::PassphraseNotGiven { .. } => "passphrase_not_given",
        Error::RecoveryCodeNotGiven { .. } => "recovery_code_not_given",
        Error::BucketUnreachable { .. } => "bucket_unreachable",
        Error::MalformedRecoveryCode { .. } => "malformed_recovery_code",
        Error::NotALibraryFolder { .. } => "not_a_library_folder",
        Error::LocalPathNotResolved { .. } => "local_path_not_resolved",
        Error::FileNotAdded { .. } => "file_not_added",
        Error::LocalFilesNotRead { .. } => "local_files_not_read",
        Error::LocalFileNotOpened { .. } => "local_file_not_opened",
        Error::LibraryNotCreated { .. } => "library_not_created",
        Error::LibraryNotJoined { .. } => "library_not_joined",
    }
}

/// A rename's refusal by its variant, snake-cased, for the reason
/// [`device_kind`] lists every variant.
fn rename_refusal_kind(refusal: &RenameRefusal) -> &'static str {
    match refusal {
        RenameRefusal::NothingAt { .. } => "nothing_to_rename",
        RenameRefusal::SamePath => "same_path",
        RenameRefusal::ReservedNamespace { .. } => "reserved_namespace",
        RenameRefusal::IntoItself => "into_itself",
        RenameRefusal::DestinationTaken { .. } => "destination_taken",
        RenameRefusal::FileAndFolder { .. } => "file_and_folder",
        RenameRefusal::Rule(_) => "rename_rule",
    }
}

/// A refusal of the shell's own — reading a secret, starting the log — by its
/// variant, snake-cased, for the reason [`device_kind`] lists every variant.
fn shell_kind(error: &coffret_shell::Error) -> &'static str {
    match error {
        coffret_shell::Error::Unread { .. } => "unread",
        coffret_shell::Error::InputEnded { .. } => "input_ended",
        coffret_shell::Error::PassphrasesDiffer => "passphrases_differ",
        coffret_shell::Error::EmptyPassphrase => "empty_passphrase",
        coffret_shell::Error::EmptyRecoveryCode { .. } => "empty_recovery_code",
        coffret_shell::Error::RecoveryCodeTooLong { .. } => "recovery_code_too_long",
        coffret_shell::Error::RecoveryCodeNotUtf8 { .. } => "recovery_code_not_utf8",
        coffret_shell::Error::LogSettingsUnread { .. } => "log_settings_unread",
        coffret_shell::Error::LogNotStarted { .. } => "log_not_started",
    }
}

/// A refusal this binary makes itself — flags that leave a provider short, a
/// client secret variable set wrongly — by its variant, snake-cased, for the
/// reason [`device_kind`] lists every variant.
fn refusal_kind(refusal: &Refusal) -> &'static str {
    match refusal {
        Refusal::FlagsMissing { .. } => "flags_missing",
        Refusal::EmptyClientSecret => "empty_client_secret",
        Refusal::ClientSecretNotUnicode => "client_secret_not_unicode",
    }
}
