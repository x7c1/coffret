//! Reading the mappings out of a file this build refused to open as a
//! catalog.
//!
//! [`SqliteIndex::open`](crate::SqliteIndex::open) refuses a file whose
//! device-local group this build cannot read, and the mappings in it are the
//! one piece of this device's own state nothing else has ever recorded
//! (spec: EP-9). The two columns every layout keeps beside
//! `DEVICE_SCHEMA_VERSION` are what make reading them back possible anyway:
//! `prefix` and `local_root` are readable by name whatever else about the
//! layout changed, so this reads exactly those two columns and nothing that
//! would touch, or even look at, the rest of the file.

use std::error;
use std::fmt;
use std::path::Path;

use coffret_usecase::device_state::Mapping;
use coffret_usecase::{IndexError, IndexResult};
use rusqlite::{Connection, OpenFlags};

use crate::query::collect;
use crate::rows;

/// A file [`SqliteIndex::open`](crate::SqliteIndex::open) refused, opened for
/// nothing but reading the two columns every layout keeps.
///
/// No layout is looked at, no journal mode is set, and nothing is written: the
/// refusal that led here promised the file untouched, and this keeps that
/// promise by running no `prepare` at all.
pub struct RefusedIndex {
    connection: Connection,
}

impl RefusedIndex {
    /// Opens the file at `path` for reading, and nothing more.
    ///
    /// Read-only is tried first, because a file this build already refused
    /// should not be written to just to be read. A WAL-mode file can refuse a
    /// read-only open when its `-shm` file is not beside it and this device
    /// cannot create one there — SQLite needs to be able to set shared memory
    /// up, which a read-only connection cannot always do — so the fallback
    /// opens read-write instead. Nothing that follows issues a write of its
    /// own, so the file is still left as it was; only the connection is
    /// allowed to be more than a reader.
    ///
    /// If the fallback fails too, the read-only attempt's cause travels
    /// alongside it rather than being the one dropped: the two usually name
    /// the same directory-level reason, but nothing here assumes that.
    pub fn open(path: impl AsRef<Path>) -> IndexResult<Self> {
        const OPERATION: &str = "opening the Index file to read its mappings";
        let path = path.as_ref();
        let connection = match Self::readable(path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
            Ok(connection) => connection,
            Err(read_only) => {
                let fallback = Self::readable(path, OpenFlags::SQLITE_OPEN_READ_WRITE);
                fallback.map_err(|read_write| IndexError::Backend {
                    operation: OPERATION,
                    cause: Box::new(BothOpensFailed {
                        read_only,
                        read_write,
                    }),
                })?
            }
        };
        Ok(Self { connection })
    }

    /// Opens `path` with `flags`, and confirms the connection actually
    /// answers before handing it back.
    ///
    /// `Connection::open_with_flags` alone cannot be trusted to fail when a
    /// WAL-mode file's `-shm` is out of reach: SQLite opens a file lazily, so
    /// the open itself still returns `Ok`, and only the first statement run
    /// against it fails. Reading the stamp is that first statement — cheap,
    /// and it is what surfaces the failure here, where [`open`](Self::open)
    /// can still fall back to a different open, rather than later out of
    /// [`mappings`](Self::mappings), where there would be nothing left to
    /// fall back to.
    fn readable(path: &Path, flags: OpenFlags) -> rusqlite::Result<Connection> {
        let connection = Connection::open_with_flags(path, flags)?;
        connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?;
        Ok(connection)
    }

    /// The mappings in the file, root first, and with no `root_identity`: a
    /// mapping read this way is about to be recorded afresh, so the next scan
    /// is what stamps it, not this read.
    ///
    /// The one thing this asks of the file is what every layout down to the
    /// first has kept true: a `mappings` table with `prefix` and `local_root`
    /// in it.
    pub fn mappings(&self) -> IndexResult<Vec<Mapping>> {
        collect(
            &self.connection,
            "SELECT prefix, local_root FROM mappings ORDER BY prefix IS NOT NULL, prefix",
            [],
            "reading the mappings from a refused Index file",
            rows::refused_mapping,
        )
    }
}

/// Neither the read-only nor the read-write open of this file succeeded.
///
/// The two are kept apart rather than one standing in for both: they usually
/// name the same directory-level reason — neither connection could set the
/// `-shm` file up — but a rarer case where they differ is not one this type
/// decides for a reader by throwing either cause away.
#[derive(Debug)]
struct BothOpensFailed {
    read_only: rusqlite::Error,
    read_write: rusqlite::Error,
}

impl fmt::Display for BothOpensFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "opened read-only: {}; opened read-write: {}",
            self.read_only, self.read_write
        )
    }
}

impl error::Error for BothOpensFailed {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.read_write)
    }
}
