//! One finding, as the JSON answer carries it.

use serde::Serialize;

use coffret_device::{Disposal, Finding, FindingReason, RootRefused, RootUnavailable, Settled};

use super::{prefix_said, said_path};

/// One finding: which kind it is, whether somebody has to act on it, the line
/// the text form prints for it, and the facts of that kind.
///
/// `kind` is the device layer's variant in snake case, and every reason is the
/// variant's own name — `DeletedLocally`, `ForeignFile` — which is how the
/// explorer's server names the finding behind a declined fetch. Every variant
/// is matched rather than left to a wildcard, so that a finding added below
/// has to be given its shape here on purpose.
#[derive(Clone, Serialize)]
pub struct Found {
    kind: &'static str,
    needs_attention: bool,
    said: String,
    #[serde(flatten)]
    facts: Facts,
}

/// What a finding of one kind says beyond its sentence.
#[derive(Clone, Serialize)]
#[serde(untagged)]
enum Facts {
    /// An Entry a run left as it found it.
    Entry { path: String, reason: &'static str },
    /// A mapped folder a run did not go into.
    ///
    /// The folder is the one the text form names on standard output.
    Root {
        prefix: Option<String>,
        local_root: String,
        reason: &'static str,
    },
    /// A Container a run could not open, or could not trash.
    Container { container_id: String },
    /// A batch an earlier run left, settled.
    Settled {
        container_id: String,
        settlement: &'static str,
    },
    /// A committed Keyring read with replicas missing.
    DegradedKeyring {
        generation: u64,
        replicas: u16,
        lost: u16,
        unfetched: u16,
    },
    /// A committed Keyring a run put replicas back into.
    KeyringRepaired { generation: u64, rewritten: usize },
    /// Nothing beyond the sentence.
    Nothing {},
}

impl From<&Finding> for Found {
    fn from(finding: &Finding) -> Self {
        let (kind, facts) = match finding {
            Finding::Surfaced { path, reason } => (
                "surfaced",
                Facts::Entry {
                    path: path.as_str().to_owned(),
                    reason: reason_name(reason),
                },
            ),
            Finding::UnavailableRoot {
                prefix,
                local_root,
                reason,
            } => (
                "unavailable_root",
                Facts::Root {
                    prefix: prefix_said(prefix.as_ref()),
                    local_root: said_path(local_root),
                    reason: match reason {
                        RootUnavailable::Missing => "Missing",
                        RootUnavailable::AnotherFilesystem => "AnotherFilesystem",
                    },
                },
            ),
            Finding::RefusedRoot {
                prefix,
                local_root,
                reason,
            } => (
                "refused_root",
                Facts::Root {
                    prefix: prefix_said(prefix.as_ref()),
                    local_root: said_path(local_root),
                    reason: match reason {
                        RootRefused::NoExpectedIdentity => "NoExpectedIdentity",
                        RootRefused::ManagementAreaMissing => "ManagementAreaMissing",
                        RootRefused::ManagementAreaNotADirectory => "ManagementAreaNotADirectory",
                        RootRefused::MarkerMissing => "MarkerMissing",
                        RootRefused::MarkerNotARegularFile => "MarkerNotARegularFile",
                        RootRefused::MarkerMalformed { .. } => "MarkerMalformed",
                        RootRefused::MarkerMismatch => "MarkerMismatch",
                    },
                },
            ),
            Finding::LockedContainer { container_id } => (
                "locked_container",
                Facts::Container {
                    container_id: container_id.to_string(),
                },
            ),
            Finding::DegradedKeyring {
                generation,
                replicas,
                lost,
                unfetched,
            } => (
                "degraded_keyring",
                Facts::DegradedKeyring {
                    generation: generation.get(),
                    replicas: *replicas,
                    lost: *lost,
                    unfetched: *unfetched,
                },
            ),
            Finding::KeyringRepaired {
                generation,
                rewritten,
            } => (
                "keyring_repaired",
                Facts::KeyringRepaired {
                    generation: generation.get(),
                    rewritten: rewritten.get(),
                },
            ),
            Finding::Settled(settled) => {
                let (container_id, settlement) = match settled {
                    Settled::Completed { container_id, .. } => (container_id, "completed"),
                    Settled::Retained { container_id } => (container_id, "retained"),
                    Settled::Disposed {
                        container_id,
                        disposal,
                    } => (
                        container_id,
                        match disposal {
                            Disposal::NeverUploaded => "never_uploaded",
                            Disposal::Trashed => "trashed",
                            Disposal::LeftInStorage { .. } => "left_in_storage",
                        },
                    ),
                };
                (
                    "settled",
                    Facts::Settled {
                        container_id: container_id.to_string(),
                        settlement,
                    },
                )
            }
            Finding::UntrashedRemoval { container_id, .. } => (
                "untrashed_removal",
                Facts::Container {
                    container_id: container_id.to_string(),
                },
            ),
            Finding::CheckpointFailed { .. } => ("checkpoint_failed", Facts::Nothing {}),
        };
        Self {
            kind,
            needs_attention: finding.needs_attention(),
            said: finding.to_string(),
            facts,
        }
    }
}

/// A reason by the device layer's name for it.
fn reason_name(reason: &FindingReason) -> &'static str {
    match reason {
        FindingReason::ChangedInPack => "ChangedInPack",
        FindingReason::DeletedLocally => "DeletedLocally",
        FindingReason::KeyLost => "KeyLost",
        FindingReason::ForeignFile => "ForeignFile",
        FindingReason::LocallyChanged => "LocallyChanged",
        FindingReason::WitnessedDeletion => "WitnessedDeletion",
        FindingReason::UnreachablePlace { .. } => "UnreachablePlace",
        FindingReason::ReservedComponent => "ReservedComponent",
    }
}
