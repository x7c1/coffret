use std::fmt;
use std::path::PathBuf;

/// Why a run left an Entry exactly as it found it.
///
/// The three flows each have their own word for this — a sync surfaces a
/// Pack-resident change, a freeze calls the same thing not frozen, a fetch
/// declines a path it cannot vouch for — and to whoever asked for the run they
/// are one kind of answer: the file is not what the Library holds, the run
/// succeeded, and here is why it did not act. So they are named once, in the
/// vocabulary the person reading them has.
///
/// None of them is an error, and none of them may be passed over quietly:
/// silence would tell a person that stale or unrecoverable content is safely
/// backed up, or that a folder is a copy of the Library when parts of it are not
/// (spec: PK-14, EP-11).
///
/// One of them names a folder on this device, which is why these are cloned
/// rather than copied. That folder reaches a person the way an unavailable
/// root's does — in a message put in front of whoever asked for the run, never
/// in a log line (spec: EP-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingReason {
    /// The file changed, and the Entry it changed from is held by a Pack.
    ///
    /// Carrying the change in means read-modify-replace over that Pack, which is
    /// `update`'s work and neither a sync's nor a freeze's (spec: PK-10, PK-11,
    /// PK-12). The Pack is left byte-for-byte as it is.
    ChangedInPack,
    /// A file this device had put in place is gone from disk.
    ///
    /// Reported and not acted on: removing the Entry from the Library is a
    /// deletion a person asks for, never one a sync infers from a missing file
    /// (spec: EP-10).
    DeletedLocally,
    /// The committed Keyring records no key for the Container holding the Entry.
    ///
    /// The ciphertext stays where it is and stays unreadable, so the Entry is
    /// locked rather than fetched or repacked (spec: KL-7, KL-17, RV-7).
    KeyLost,
    /// A file this device did not put there stands where the Entry would go.
    ///
    /// It may be content the Library has never held, so overwriting it would
    /// destroy it; which of the two wins is the person's to say (spec: EP-11).
    ForeignFile,
    /// This device put the file there and no longer recognizes what is on disk.
    ///
    /// Including one that is gone: either way it is a pending local change the
    /// sync flow owns, and fetching over it would quietly undo it (spec: EP-10).
    LocallyChanged,
    /// This device witnessed the file's deletion and is not putting it back.
    ///
    /// Restoring it is an explicit operation, the mirror of propagating the
    /// deletion on the sync side (spec: EP-10).
    WitnessedDeletion,
    /// A folder on the way to where the file belongs is not a folder of the
    /// mapped folder.
    ///
    /// A symbolic link, or an ordinary file where a folder must be. Nothing is
    /// written through such a name: what stands past it is not the folder this
    /// device maps, and a file placed there would be one the Library never
    /// pointed at (spec: EP-4, EP-11). The rest of the run is unaffected — this
    /// is the shape of one folder rather than anything about the next Entry.
    UnreachablePlace {
        /// The folder on this device the descent stopped at, which is the one
        /// thing there is to go and look at.
        component: PathBuf,
    },
}

impl fmt::Display for FindingReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let said = match self {
            Self::ChangedInPack => "it changed, and what it changed from is inside a Pack",
            Self::DeletedLocally => "this device had it and it is gone from disk",
            Self::KeyLost => "the Library records no key for the Container holding it",
            Self::ForeignFile => "a file this device did not put there stands in its place",
            // "or gone", because this reason covers a file that is no longer
            // there at all, and a person who deleted one and asked for it back
            // would otherwise be told about a file on disk that is not there —
            // next to a sync that says of the same file that it is gone.
            Self::LocallyChanged => "what this device wrote there has since changed or gone",
            Self::WitnessedDeletion => "this device witnessed its deletion",
            // The one reason with something of its own to name. Which folder it
            // is is the whole of what a person does next — `ls -l` on that one
            // name — so it is said here rather than left in the value for
            // nobody.
            Self::UnreachablePlace { component } => {
                return write!(
                    f,
                    "a folder on the way to it is not a folder of the mapped folder — {}",
                    component.display(),
                );
            }
        };
        f.write_str(said)
    }
}
