use std::fmt;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use coffret_model::{ContainerId, EntryPath, Generation};
use coffret_usecase::commit::{DegradedKeyring, KeyringRepair};
use coffret_usecase::sync::{Disposal, Settled};
use coffret_usecase::{Error as StorageError, RootRefused, RootUnavailable};

use crate::finding_reason::FindingReason;

/// One thing a run that returned `Ok` still has to say.
///
/// A run reports a failure by failing. These are the other half: the work it
/// deliberately did not do, the folders it could not read, and the Containers it
/// could not open — each of them a state the person who asked for the run is the
/// only one who can act on — together with what is said for the record and
/// nobody has to act on: the batches the run settled on the way, what its
/// commit could not finish after its record, and the Keyring set it read
/// through short or repaired. [`needs_attention`](Self::needs_attention) is what
/// tells the two apart.
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
    /// The committed Keyring set the run read through, where the read had to
    /// step over a position of it and nothing later in the run examined the set
    /// (spec: KL-5, KL-15).
    ///
    /// Said to whoever ran the flow because a run that only reads — a fetch, or
    /// a freeze that committed nothing — repairs nothing, and a device that only
    /// ever reads would otherwise never hear of it. Nobody has to act on it:
    /// reads go on (spec: RV-2), and the next run that commits repairs the set
    /// before it commits (spec: KL-13, KL-16).
    ///
    /// The counts are the read's, and floors: the read stops at the first
    /// replica that answers. `lost` is what the read established as lost, and
    /// `unfetched` what Storage would not hand over, about which nothing is
    /// known — so a finding with no position lost does not say the set is
    /// degraded, only that whether it is was not established.
    DegradedKeyring {
        /// The committed generation whose set the read walked.
        generation: Generation,
        /// How many replicas its commitment declares (spec: KL-2).
        replicas: u16,
        /// How many positions the read found lost: absent, unreadable, not a
        /// Keyring, or carrying another mapping (spec: KL-1, KL-5).
        lost: u16,
        /// How many positions Storage did not hand over.
        unfetched: u16,
    },
    /// A committed Keyring generation this run repaired before it wrote, whether
    /// the run then committed or failed (spec: KL-13, KL-15).
    ///
    /// Said because replica loss and the repair performed are never silent, and
    /// the person who asked for the run is the one who can decide whether a
    /// Library losing objects is worth looking into. Nobody has to act on it:
    /// the set is whole again, which is the news, and the replicas a commit put
    /// back stand on Storage whatever became of the batch.
    ///
    /// The generation and a count, and never a position's object name or
    /// anything of the person's: the sentence says how many replicas were put
    /// back, which is all a reader needs to weigh it (spec: EL-1).
    KeyringRepaired {
        /// The committed generation that was examined and repaired (spec: KL-3).
        generation: Generation,
        /// How many replica positions the repair rewrote, which is at least one.
        rewritten: NonZeroUsize,
    },
    /// What this run made of a batch an interrupted run left behind
    /// (spec: OC-2, OC-7).
    ///
    /// Reported because the two ways it can go are opposite outcomes: one says a
    /// Container left the Library's Storage, the other says a file this device
    /// holds is accounted for after all. A disposal Storage would not finish is
    /// said as that, and not as a disposal: its object is still in Storage.
    Settled(Settled),
    /// A Container this run's commit removed whose object Storage would not
    /// move to the trash (spec: OC-6, CP-14).
    ///
    /// The commit stands — the record already took the Container out of the
    /// current set — and what is left is an object the record proves removed,
    /// which the Library lets any later run trash (spec: OC-6). Said with what
    /// Storage answered, because what finishes it differs by which refusal it
    /// was: a provider having a bad minute needs only another run, credentials
    /// that may write but not delete need a person.
    UntrashedRemoval {
        /// The Container the commit removed.
        container_id: ContainerId,
        /// What Storage answered the trash with.
        cause: StorageError,
    },
    /// A checkpoint this run's commit was due to write and could not
    /// (spec: CK-8).
    ///
    /// The commit stands, because a checkpoint is not part of it (spec: CP-1),
    /// and the records it would have covered stay replayable until the next
    /// qualifying commit writes one.
    CheckpointFailed {
        /// What stopped it, as its whole chain renders.
        ///
        /// A sentence rather than the error itself, because the commit's error is
        /// not a value a finding can be copied with; what a person reads of it is
        /// the sentence, and the variant is what a caller decides from.
        cause: String,
    },
}

impl Finding {
    /// Whether somebody still has to act on this.
    ///
    /// Completed settlement and post-commit cleanup are reports of work
    /// performed. A retained uncertain commit still needs attention: the run
    /// cannot prove whether it entered the Catalog (spec: OC-1, OC-3).
    /// Failed trash keeps its provenance for a later retry (spec: OC-2), and
    /// the next qualifying commit retries a checkpoint (spec: CK-8).
    ///
    /// A degraded Keyring is said and not escalated for the same reason: the
    /// read went on (spec: RV-2), and the next run that commits repairs the set
    /// before it commits (spec: KL-13, KL-16), so a run that only reports one
    /// exits as it would without it. A repaired one even less so: it is work the
    /// run did, and the set is whole again.
    pub fn needs_attention(&self) -> bool {
        !matches!(
            self,
            Self::Settled(Settled::Completed { .. } | Settled::Disposed { .. })
                | Self::UntrashedRemoval { .. }
                | Self::CheckpointFailed { .. }
                | Self::DegradedKeyring { .. }
                | Self::KeyringRepaired { .. }
        )
    }
}

/// The finding a read that stepped over a position of the committed Keyring set
/// makes of it (spec: KL-5, KL-15).
///
/// What every run that carries a [`DegradedKeyring`] on its outcome reports it
/// as, and what a caller that runs several such reads under one flow of its
/// own — one Entry at a time, say — reports it as too, so the two never say it
/// in different words.
impl From<&DegradedKeyring> for Finding {
    fn from(found: &DegradedKeyring) -> Self {
        Self::DegradedKeyring {
            generation: found.generation(),
            replicas: found.replicas(),
            lost: found.lost(),
            unfetched: found.unfetched(),
        }
    }
}

/// The finding a repair a commit performed makes of it (spec: KL-15).
///
/// One conversion for a commit that went on to land and for one that failed
/// after it, so a repair reads the same whichever way the run ended.
impl From<&KeyringRepair> for Finding {
    fn from(repair: &KeyringRepair) -> Self {
        Self::KeyringRepaired {
            generation: repair.generation,
            rewritten: repair.rewritten.count(),
        }
    }
}

/// An error and every cause under it, joined the way a command line prints a
/// chain it can walk.
///
/// A finding is not an error type, so the line it renders is the whole of what
/// its reader gets: an error's own line that leaves the rest to its chain would
/// otherwise leave it unsaid. [`StorageError`] is one such — where a gateway
/// handed over a value, its own line says only what kind of refusal it was.
pub(crate) fn chained(error: &dyn std::error::Error) -> String {
    let mut said = error.to_string();
    let mut below = error.source();
    while let Some(link) = below {
        said.push_str(": ");
        said.push_str(&link.to_string());
        below = link.source();
    }
    said
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

/// Why the device would not place anything into a root, with the defect a
/// malformed marker leaves to its cause.
///
/// [`RootRefused::MarkerMalformed`] says that the marker names no identity and
/// leaves *what is wrong with its content* to the `cause` it carries, because
/// every error that carries the refusal hands that cause on as the chain's next
/// link. Nothing on the way to this sentence does: neither a [`Finding`] nor a
/// [`RootRefused`] is an error type, so the line this renders is the whole of
/// what its reader gets. Left at the refusal alone, a person would read that
/// the marker was rejected and never why — past the cap, not text, no identity
/// spelled in it — so the marker's own answer is said here, where the reader
/// is, the way a diagnostic event's rendering spells out the causes a `Display`
/// leaves to the chain for exactly the same reason.
///
/// The whole of that cause's chain and not its first link, joined the way a
/// command line prints a chain it can walk. One of the three defects is a
/// wrapper in its own right — the content is text and no spelling of an
/// identity — and its own line says only that, which beside "names no identity"
/// is one statement made twice. What a person can act on stands under it: how a
/// root's identity is spelled, and how this content missed it.
fn refusal_said(reason: &RootRefused) -> String {
    match reason {
        RootRefused::MarkerMalformed { cause } => format!("{reason} ({})", chained(cause)),
        // Nothing is left to a chain: each of these says the whole of what it
        // knows in its own line. Listed rather than left to a wildcard, so that
        // a refusal added with a cause has to say here how it is read.
        RootRefused::NoExpectedIdentity
        | RootRefused::ManagementAreaMissing
        | RootRefused::ManagementAreaNotADirectory
        | RootRefused::MarkerMissing
        | RootRefused::MarkerNotARegularFile
        | RootRefused::MarkerMismatch => reason.to_string(),
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
                    // No gesture: what remedies this is plugging the disk in or
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
            // The gesture: recording that mapping again is what decides which
            // folder it is, with a new identity asked for where the identity is
            // meant to change.
            Self::RefusedRoot {
                prefix,
                local_root,
                reason,
            } => write!(
                f,
                "refused root {}, which this device maps {} into: {}; nothing was placed \
                 into it, and `coffret map` records that mapping again — with `--reset-marker` \
                 where the identity is meant to change",
                local_root.display(),
                mapping_said(prefix.as_ref()),
                refusal_said(reason),
            ),
            Self::LockedContainer { container_id } => {
                write!(f, "locked container {container_id}")
            }
            Self::Settled(Settled::Retained { container_id }) => write!(
                f, "retained container {container_id}: its commit outcome is unknown; its object and local provenance were kept"
            ),
            Self::Settled(Settled::Completed { container_id, .. }) => write!(
                f,
                "settled container {container_id}: its commit had landed, and the bookkeeping is \
                 now complete"
            ),
            Self::Settled(Settled::Disposed {
                container_id,
                disposal: Disposal::NeverUploaded | Disposal::Trashed,
            }) => write!(
                f,
                "settled container {container_id}: nothing committed it, so what it left was \
                 disposed of"
            ),
            // The row remains the proof needed to retry this trash.
            Self::Settled(Settled::Disposed {
                container_id,
                disposal: Disposal::LeftInStorage { cause },
            }) => write!(
                f,
                "settled container {container_id}: nothing committed it, and Storage would not \
                 move its object to the trash ({}); the object is still in Storage, and its \
                 provenance is kept for retry",
                chained(cause),
            ),
            Self::UntrashedRemoval {
                container_id,
                cause,
            } => write!(
                f,
                // Not "no current state names it", which would read as the
                // suspected orphan a refused disposal leaves: the record proves
                // this removal, so it is no orphan, and the trash is any later
                // run's to retry (spec: OC-6).
                "untrashed container {container_id}: the commit removed it and stands, and \
                 Storage would not move its object to the trash ({}); the object is still in \
                 Storage, and any later run may trash it",
                chained(cause),
            ),
            Self::DegradedKeyring {
                generation,
                replicas,
                lost,
                unfetched,
            } => {
                let generation = generation.get();
                if *lost == 0 {
                    // Nothing is known to be lost, so the sentence does not say
                    // "degraded": the next run may read every one of them
                    // (spec: KL-15).
                    return write!(
                        f,
                        "unread keyring replicas: Storage did not hand over {unfetched} of the \
                         {replicas} replicas of Keyring generation {generation}, so whether its \
                         set is degraded is not established; reads go on, and the next run that \
                         commits examines the set",
                    );
                }
                let also = match unfetched {
                    0 => String::new(),
                    more => format!(", and Storage did not hand over {more} more"),
                };
                write!(
                    f,
                    "degraded keyring: at least {lost} of the {replicas} replicas of Keyring \
                     generation {generation} {} missing or unreadable{also}; reads go on, and the \
                     next run that commits repairs the set",
                    if *lost == 1 { "is" } else { "are" },
                )
            }
            Self::KeyringRepaired {
                generation,
                rewritten,
            } => {
                // The words the concept documentation uses, because this is
                // where a person meets them: replicas of a Keyring generation,
                // missing or unreadable, and rewritten from one that survived
                // (spec: KL-6, KL-13). One sentence for every shell, so the
                // repair reads alike at a terminal and in the explorer.
                let (replicas, was) = match rewritten.get() {
                    1 => ("1 replica".to_owned(), "was"),
                    many => (format!("{many} replicas"), "were"),
                };
                write!(
                    f,
                    "repaired the Keyring: {replicas} of generation {} {was} missing or \
                     unreadable, and {was} rewritten from a surviving one",
                    generation.get(),
                )
            }
            Self::CheckpointFailed { cause } => write!(
                f,
                "checkpoint not written ({cause}): the commit stands, and the next qualifying \
                 commit writes the checkpoint"
            ),
        }
    }
}
