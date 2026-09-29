//! Why a run failed, as the JSON answer carries it.

use std::error;

use serde::Serialize;

use coffret_device::{CommitError, CommitFailure, Error, IndexError, ModelError, StorageError};

use crate::fetch::UnmappedEntry;
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
/// at a layout this build does not read, a Drive Library with no grant,
/// credentials Storage rejected, and an epoch this device holds no key for.
/// Each is answered by one gesture — `coffret map`, deleting the Index file,
/// `coffret authorize`, the credentials, re-enrolment — whatever flow met it.
///
/// Then the outermost error this crate knows the type of, named by its variant
/// — except where the variant is a flow's wrapper around Storage not coming
/// through, which is `storage`, the kind the explorer's server gives it.
fn classified<'a>(
    chain: impl Iterator<Item = &'a (dyn error::Error + 'static)> + Clone,
) -> Classified {
    for link in chain.clone() {
        if link.is::<UnmappedEntry>() {
            return Classified::kind("unmapped");
        }
        if let Some(IndexError::UnsupportedSchema { found, supported }) = link.downcast_ref() {
            return Classified {
                kind: "unsupported_schema",
                found: Some(*found),
                supported: Some(*supported),
            };
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
        if let Some(CommitError::EpochActivated { .. }) = commit {
            return Classified::kind("epoch");
        }
    }

    let storage_below = chain.clone().any(|link| link.is::<StorageError>());
    for link in chain {
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
    }
    Classified::kind("other")
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
        Error::Fetch { .. } if storage_below => "storage",
        Error::CatchUp { .. } if storage_below => "storage",
        Error::Sync { .. } => "sync",
        Error::Freeze { .. } => "freeze",
        Error::Fetch { .. } => "fetch",
        Error::CatchUp { .. } => "catch_up",
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
