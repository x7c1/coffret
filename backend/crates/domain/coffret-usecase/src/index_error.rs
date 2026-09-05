use std::error;
use std::fmt;
use std::path::PathBuf;

use coffret_model::{ContainerId, EntryPath, Redacted};

/// Result alias for [`Index`](crate::Index) operations.
pub type IndexResult<T> = std::result::Result<T, IndexError>;

/// Everything an [`Index`](crate::Index) operation can fail with.
///
/// It is a vocabulary of its own rather than the Storage port's
/// [`Error`](crate::Error), because the two ports fail at different things: the
/// Index is a device-local catalog that no provider is involved in, so nothing
/// here is a lost race, a throttle, or a transport fault, and nothing here is
/// worth retrying unchanged.
///
/// The catalog being a cache and never the source of truth (spec: RV-5) is what
/// makes the failures here small: whatever cannot be read back can be rebuilt
/// from Storage, so the type says what the caller must do — rebuild, resolve a
/// conflict, install a build that knows the file, or name the local path or the
/// number the catalog cannot keep — rather than describing a backend.
#[derive(Debug)]
pub enum IndexError {
    /// The Index has never been given a committed state to stand on.
    ///
    /// A fresh Index catalogs nothing and checkpoints nothing until a
    /// [`restore`](crate::Index::restore) adopts a Snapshot or an
    /// [`apply`](crate::Index::apply) replays a record, so there is no content
    /// to hand back and no checkpoint to write into one (spec: CK-9).
    NoCheckpoint,
    /// Two Entries would occupy one Entry Path.
    ///
    /// At every committed Library state one Entry Path identifies at most one
    /// current Entry, so a replay or a restore that would place a second one
    /// there is describing a state no commit could have produced (spec: EP-5,
    /// EP-6).
    DuplicatePath {
        /// The path claimed twice.
        path: EntryPath,
    },
    /// One Container would be added to the current set twice.
    DuplicateContainer {
        /// The Container claimed twice.
        container_id: ContainerId,
    },
    /// An Entry names a Container the current set does not hold.
    ///
    /// A record carries the Entries of the Containers it adds, so an Entry
    /// without its Container is a record or a Snapshot that cannot be replayed
    /// as it stands (spec: CP-11).
    UnknownContainer {
        /// The Container the Entry names.
        container_id: ContainerId,
    },
    /// A local path the catalog has no way to keep.
    ///
    /// A filesystem may hand out a name that is not valid UTF-8, and a catalog
    /// that keeps paths as text cannot store one without changing it. Keeping a
    /// lossy spelling would point a mapping or a spool at a file that is not the
    /// one meant, so the path is refused instead.
    ///
    /// The path travels in the value so a caller can say which file it is
    /// about; the message leaves it out, because a local path is the user's own
    /// and a message may end up wherever an error is reported.
    UnrepresentablePath {
        /// What the Index was doing.
        operation: &'static str,
        /// The path that cannot be kept.
        path: PathBuf,
    },
    /// A number the catalog has no column wide enough to hold.
    ///
    /// The counterpart of [`IndexError::UnrepresentablePath`] for the numbers a
    /// catalog keeps — an offset, a size, an epoch, a generation. A catalog
    /// spells them in signed 64-bit columns while the domain counts them
    /// unsigned, so the top half of the unsigned range has no spelling there,
    /// and the choice is between refusing a value that reaches it and storing
    /// it under a sign it does not have.
    ///
    /// No value the format produces in practice reaches it: a Storage Object is
    /// at most a few terabytes, and a generation counts commits. A Library that
    /// somehow held one is not a Library this device can catalog, and being told
    /// so is the honest answer — where storing the same bits under a negative
    /// sign would leave the catalog reading back a value nothing ever wrote.
    ///
    /// The number travels in the message: an offset, an epoch, or a generation
    /// is the format's own arithmetic rather than anything a Library holds.
    UnrepresentableValue {
        /// What the Index was doing.
        operation: &'static str,
        /// The column the value was headed for.
        column: &'static str,
        /// The value that has no spelling there.
        value: u64,
    },
    /// The Index file is laid out in a way this build cannot open, and cannot
    /// repair by discarding the catalog alone.
    ///
    /// The catalog being a cache is what lets an adapter throw one away and
    /// rebuild it from Storage (spec: RV-5), so an older layout is not
    /// ordinarily refused at all. This is the case where that is not enough:
    /// beside the catalog the file holds the state that is only ever this
    /// device's — where the Library is mapped onto its folders, what it has on
    /// disk, what it spooled and never committed (spec: EP-9, EP-10, OC-2) —
    /// and no adapter can keep that across a layout it does not read, nor
    /// recover it from anywhere else. A file from a *newer* build is refused
    /// for the plainer reason that this one cannot read any of it.
    ///
    /// So the answer is the owner's rather than the adapter's, and the message
    /// states it: the mappings can still be read out of the file before it
    /// goes, so delete it, record them again, and catch up.
    UnsupportedSchema {
        /// The version found in the file.
        found: i64,
        /// The version this build writes and reads.
        supported: i64,
    },
    /// The catalog holds a value this build cannot read back.
    ///
    /// A Container kind spelled in a vocabulary this build has no reading for,
    /// a stored digest the domain does not admit, half a reference where a
    /// whole one belongs: the file was written by something else, or damaged.
    /// The answer is the one [`IndexError::UnsupportedSchema`] states — the
    /// file cannot be carried forward, so it goes and the catalog is rebuilt
    /// from Storage (spec: RV-5) — and not the one a store that merely failed
    /// asks for, which is why the two are separate.
    UnreadableCatalog {
        /// What the Index was doing.
        operation: &'static str,
        /// What could not be read, as whatever refused it reported.
        cause: Box<dyn error::Error + Send + Sync>,
    },
    /// The store the catalog is kept in failed.
    ///
    /// The store itself, not what it holds: a file that cannot be opened, a
    /// write that cannot be carried out, a thread that never finished.
    Backend {
        /// What the Index was doing.
        operation: &'static str,
        /// What the store reported.
        cause: Box<dyn error::Error + Send + Sync>,
    },
}

/// What is left to do with an Index file that cannot be carried forward.
///
/// Deleting the file is the whole of the repair for the catalog, and none of it
/// for the rest: the mappings go with it and nothing else has ever held them
/// (spec: EP-9). Reading them back does not ask the owner to recall them from
/// memory, though: the two columns that carry a mapping are the one part of a
/// refused file that stays readable whatever else about its layout is not, so
/// a caller can read them straight out of the file before it goes rather than
/// having nowhere left to look. Said in the domain's own words rather than as a
/// sequence of commands — what runs above this layer knows what it calls each
/// of these, and the message has to read the same wherever a refusal is
/// reported.
const RECOVERY: &str = "the mappings this device holds can still be read from the file before \
                        anything is done to it; delete the Index file, record those mappings \
                        again, and catch up from Storage";

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCheckpoint => f.write_str("the Index stands at no committed Library state"),
            // The Entry Path is what identifies the conflict, so the message
            // carries it — which is why a log line renders this through
            // [`Redacted`] instead: an Entry Path never belongs in one.
            Self::DuplicatePath { path } => {
                write!(f, "two Entries claim the Entry Path {path:?}")
            }
            Self::DuplicateContainer { container_id } => {
                write!(f, "Container {container_id} is added twice")
            }
            Self::UnknownContainer { container_id } => {
                write!(f, "no current Container {container_id} to hold this Entry")
            }
            // The path stays out of the message, and stays in the value: see
            // the variant.
            Self::UnrepresentablePath { operation, .. } => write!(
                f,
                "a local path given while {operation} is not one this catalog can keep: \
                 it is not valid UTF-8"
            ),
            // The number goes into the message, where a local path stays out of
            // it: see the variant.
            Self::UnrepresentableValue {
                operation,
                column,
                value,
            } => write!(
                f,
                "the value {value} given while {operation} is past what this catalog's \
                 {column} column can hold"
            ),
            // Which of the two refusals it is, said in words: the pair of
            // versions already distinguishes them, and a reader deciding what
            // to do should not have to compare two numbers to find out that a
            // build older than their file is a different situation from a file
            // older than their build.
            Self::UnsupportedSchema { found, supported } if found < supported => write!(
                f,
                "the Index file is at schema version {found}, an older layout than the \
                 version {supported} this build can carry forward: {RECOVERY}"
            ),
            Self::UnsupportedSchema { found, supported } => write!(
                f,
                "the Index file is at schema version {found}, newer than the version \
                 {supported} this build reads: use the build that wrote it. Otherwise, \
                 {RECOVERY}"
            ),
            Self::UnreadableCatalog { operation, cause } => {
                write!(
                    f,
                    "the Index file holds something this build cannot read while {operation}: \
                     {cause}"
                )
            }
            Self::Backend { operation, cause } => {
                write!(f, "the Index store failed while {operation}: {cause}")
            }
        }
    }
}

impl error::Error for IndexError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::UnreadableCatalog { cause, .. } | Self::Backend { cause, .. } => {
                Some(cause.as_ref())
            }
            _ => None,
        }
    }
}

impl Redacted for IndexError {
    /// Which refusal it is, which operation met it, and nothing the catalog
    /// holds.
    ///
    /// The one number that does travel is
    /// [`UnrepresentableValue`](Self::UnrepresentableValue)'s: a reader of the
    /// log who is not told which value was refused cannot tell a build with a
    /// narrow column from a Library that really holds one.
    ///
    /// The two boxed causes stop here rather than going underneath. What they
    /// carry is the Index store's own answer — SQLite's, in the shipped build —
    /// and a store that names the file it could not read is naming a local
    /// path. The `operation` is what a reader of the log needs from those two
    /// anyway: it says which statement was running, which is the half this
    /// crate put there.
    fn redacted(&self) -> String {
        match self {
            Self::NoCheckpoint => "Index::NoCheckpoint".to_owned(),
            Self::DuplicatePath { path } => {
                format!("Index::DuplicatePath(path_len={})", path.as_str().len())
            }
            Self::DuplicateContainer { container_id } => {
                format!("Index::DuplicateContainer(container={container_id})")
            }
            Self::UnknownContainer { container_id } => {
                format!("Index::UnknownContainer(container={container_id})")
            }
            Self::UnrepresentablePath { operation, .. } => {
                format!("Index::UnrepresentablePath(operation={operation})")
            }
            Self::UnrepresentableValue {
                operation,
                column,
                value,
            } => format!(
                "Index::UnrepresentableValue(operation={operation}, column={column}, \
                 value={value})"
            ),
            Self::UnsupportedSchema { found, supported } => {
                format!("Index::UnsupportedSchema(found={found}, supported={supported})")
            }
            Self::UnreadableCatalog { operation, .. } => {
                format!("Index::UnreadableCatalog(operation={operation})")
            }
            Self::Backend { operation, .. } => {
                format!("Index::Backend(operation={operation})")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_paths::entry_path;

    // EP-1: the path is what identifies the conflict to whoever is keeping the
    // Library, and it is the one thing a log line may not say.
    #[test]
    fn a_conflict_over_one_path_says_how_long_it_was_and_no_more() {
        let error = IndexError::DuplicatePath {
            path: entry_path("albums/spring.jpg"),
        };

        assert!(error.to_string().contains("albums/spring.jpg"));
        assert_eq!(error.redacted(), "Index::DuplicatePath(path_len=17)");
    }

    // A build older than the file and a file older than the build are two
    // different situations, and the one thing neither message may do is name a
    // command: what the steps are called belongs to whatever shows the refusal.
    #[test]
    fn an_older_layout_and_a_newer_one_ask_for_different_things() {
        let older = IndexError::UnsupportedSchema {
            found: 3,
            supported: 5,
        };
        let newer = IndexError::UnsupportedSchema {
            found: 9,
            supported: 5,
        };

        assert!(older.to_string().contains("an older layout"), "{older}");
        assert!(newer.to_string().contains("newer than"), "{newer}");
        assert!(
            newer.to_string().contains("use the build that wrote it"),
            "{newer}"
        );
        for message in [older.to_string(), newer.to_string()] {
            assert!(message.contains("delete the Index file"), "{message}");
            assert!(message.contains("record those mappings again"), "{message}");
            assert!(message.contains("catch up from Storage"), "{message}");
        }
    }

    // A value past what a column can hold is the format's own arithmetic, so
    // both renderings name the column and the number: a log saying only that
    // something was refused would leave the reader unable to tell which.
    #[test]
    fn a_value_no_column_can_hold_is_named_with_the_number_and_the_column() {
        let error = IndexError::UnrepresentableValue {
            operation: "restoring from a Snapshot",
            column: "offset",
            value: 1 << 63,
        };

        let message = error.to_string();
        assert!(message.contains("9223372036854775808"), "{message}");
        assert!(message.contains("offset"), "{message}");
        assert_eq!(
            error.redacted(),
            "Index::UnrepresentableValue(operation=restoring from a Snapshot, column=offset, \
             value=9223372036854775808)",
        );
    }

    // The store's own message may name the catalog file, so what survives is
    // the statement that was running.
    #[test]
    fn what_the_index_store_reported_is_named_by_its_operation() {
        let error = IndexError::Backend {
            operation: "recording a mapping",
            cause: "unable to open database file /home/someone/library/index.db".into(),
        };

        assert!(error.to_string().contains("/home/someone"));
        assert_eq!(
            error.redacted(),
            "Index::Backend(operation=recording a mapping)",
        );
    }
}
