use std::fmt;
use std::path::PathBuf;

use coffret_model::{ContainerId, EntryPath};
use coffret_usecase::sync::Reconciled;
use coffret_usecase::{RootRefused, RootUnavailable};

use crate::finding_reason::FindingReason;

/// One thing a run that returned `Ok` still has to say.
///
/// A run reports a failure by failing. These are the other half: the work it
/// deliberately did not do, the folders it could not read, and the Containers it
/// could not open — each of them a state the person who asked for the run is the
/// only one who can act on — together with the batches the run settled on the
/// way, which are said for the record and are the one kind nobody has to act on.
/// [`needs_attention`](Self::needs_attention) is what tells the two apart.
///
/// The Entry Path and the local root travel in the value because whoever
/// rendered it is who decides what to do about them. Neither ever travels
/// into a diagnostic event; [`Display`](fmt::Display) is the deliberate act
/// of putting one in front of the person who asked.
///
/// Deliberately no `PartialEq`: one of these carries a refused root's reason,
/// which carries the marker's own refusal, and error values here are reported
/// rather than compared. A caller that wants to assert on a finding asserts on
/// the variant or on the sentence it renders to.
#[derive(Debug, Clone)]
pub enum Finding {
    /// An Entry the run left exactly as it found it (spec: PK-14, EP-11).
    Surfaced {
        /// Where in the Library it stands.
        path: EntryPath,
        /// Why the run did not act on it.
        reason: FindingReason,
    },
    /// A mapping whose local root the device could not vouch for (spec: EP-12).
    ///
    /// Nothing under it was walked and no Entry under it was read as deleted, so
    /// a run carrying one of these has covered less than the device's mappings
    /// do — which is exactly what an unplugged disk should look like, and
    /// nothing like a folder a person emptied.
    UnavailableRoot {
        /// The top-level component the mapping stands for, or `None` for the
        /// Library root.
        ///
        /// The half of the mapping a finding may name: it is a name inside the
        /// Library rather than a path on this device, so it reaches the person
        /// who asked for the run and never a diagnostic event (spec: EL-1). A
        /// device with more than one mapping would otherwise be told a folder
        /// of theirs was not read, and left to work out which of their mappings
        /// that was.
        prefix: Option<EntryPath>,
        /// The folder on this device the mapping names.
        local_root: PathBuf,
        /// What made it unavailable.
        reason: RootUnavailable,
    },
    /// A mapping whose local root is not the root it was recorded against
    /// (spec: EP-13).
    ///
    /// A separate finding from [`UnavailableRoot`](Self::UnavailableRoot) on
    /// purpose, because the two answer different questions: EP-12's asks whether
    /// the root is *there to be read from*, and this asks whether the folder
    /// standing at it is the one whose marker the mapping recorded. A root can be
    /// perfectly available and still be the wrong folder.
    ///
    /// Nothing was placed under the mapping and the run went on with the device's
    /// others, so a run carrying one of these has placed less than its mappings
    /// cover. Reported once for the mapping rather than once per Entry: what went
    /// wrong is the root.
    RefusedRoot {
        /// The top-level component the mapping stands for, or `None` for the
        /// Library root.
        ///
        /// Carried and named for the reason
        /// [`UnavailableRoot`](Self::UnavailableRoot)'s is (spec: EL-1).
        prefix: Option<EntryPath>,
        /// The folder on this device the mapping names.
        local_root: PathBuf,
        /// Why the device would not place anything into it.
        reason: RootRefused,
    },
    /// A Container the committed Keyring records no key for (spec: KL-7).
    ///
    /// Reported at the Container level as well as per Entry, because that is the
    /// level the loss is at: one explicit key-lost marker locks every Entry the
    /// Container holds, and healing it is one act rather than one per file
    /// (spec: KL-17, RV-7).
    LockedContainer {
        /// The Container whose key the Library has none of.
        container_id: ContainerId,
    },
    /// What this run made of a batch an interrupted run left behind
    /// (spec: OC-2, OC-7).
    ///
    /// Reported because the two ways it can go are opposite outcomes: one says a
    /// Container left the Library's Storage, the other says a file this device
    /// holds is accounted for after all.
    Settled(Reconciled),
}

impl Finding {
    /// Whether somebody still has to act on this.
    ///
    /// A settled batch is reported for the record — the run already did what
    /// there was to do about it — so it is the one finding that leaves nothing
    /// behind.
    pub fn needs_attention(&self) -> bool {
        !matches!(self, Self::Settled(_))
    }
}

/// How a finding names the mapping it is about, beside the folder it already
/// named.
///
/// The two findings about a mapped root say this the same way, because they are
/// two questions about one mapping (spec: EP-12, EP-13) and a person who met
/// both should not have to work out that the two sentences are about the same
/// one. `None` is the mapping that stands for the whole Library, which EP-9
/// admits and which has no component to be named by, so it is named in the only
/// words there are for it.
///
/// The prefix is quoted, the way every other sentence a person reads about these
/// states spells it: it stands next to a local path here, and a bare name beside
/// one reads as a second path.
///
/// Not the clause inside the sentence a refusal *raised as an error* is shown
/// as ([`RefusedRoot`](coffret_usecase::RefusedRoot)'s own `Display`): that one
/// carries "the mapping for" inside it, and these two sentences have already
/// said *which this device maps … into* by the time they reach this.
fn mapping_said(prefix: Option<&EntryPath>) -> String {
    match prefix {
        Some(prefix) => format!("{:?}", prefix.as_str()),
        None => "the Library root".to_owned(),
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Surfaced { path, reason } => write!(f, "surfaced {path}: {reason}"),
            Self::UnavailableRoot {
                prefix,
                local_root,
                reason,
            } => {
                let said = match reason {
                    // No gesture: what settles this is plugging the disk in or
                    // mounting the share, and recording the mapping again is not
                    // something there is a folder to do it to.
                    RootUnavailable::Missing => "it is not there",
                    // The gesture, in the voice the refused root below says its
                    // own in. EP-12 leaves exactly one state a person has to act
                    // their way out of — a folder genuinely emptied whose
                    // filesystem identity also moved — and every later run
                    // reports it again until they do, because an emptied root on
                    // an unrecorded filesystem is the same thing this device sees
                    // when a mount point is standing bare. Recording the mapping
                    // again clears the identity, so the next run stamps whatever
                    // the root stands on.
                    RootUnavailable::AnotherFilesystem => {
                        "it is empty and stands on another filesystem, which is what an unmounted \
                         mount point looks like; where the folder really is empty, `coffret map` \
                         records that mapping again and the next run stamps what it finds"
                    }
                };
                write!(
                    f,
                    "unavailable root {}, which this device maps {} into: {said}",
                    local_root.display(),
                    mapping_said(prefix.as_ref()),
                )
            }
            // The gesture: recording that mapping again is what settles which
            // folder it is, with a new identity asked for where the identity is
            // meant to change.
            Self::RefusedRoot {
                prefix,
                local_root,
                reason,
            } => write!(
                f,
                "refused root {}, which this device maps {} into: {reason}; nothing was placed \
                 into it, and `coffret map` records that mapping again — with `--reset-marker` \
                 where the identity is meant to change",
                local_root.display(),
                mapping_said(prefix.as_ref()),
            ),
            Self::LockedContainer { container_id } => {
                write!(f, "locked container {container_id}")
            }
            Self::Settled(Reconciled::Completed { container_id, .. }) => write!(
                f,
                "settled container {container_id}: its commit had landed, and the bookkeeping is \
                 now complete"
            ),
            Self::Settled(Reconciled::Disposed { container_id, .. }) => write!(
                f,
                "settled container {container_id}: nothing committed it, so what it left was \
                 disposed of"
            ),
        }
    }
}
