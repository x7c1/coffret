//! [`Index`](coffret_usecase::Index) kept in a SQLite file, one file per
//! Library.
//!
//! The catalog is a cache: it answers which Container holds the Entry at an
//! Entry Path without asking Storage, and losing it costs a rebuild rather than
//! any Library data (spec: RV-5). A local database is the right shape for that,
//! and SQLite the right one to build it on — the file is the unit of backup and
//! of deletion, the primary key on Entry Path is the range scan a subtree query
//! needs, and a transaction is the all-or-nothing a replay has to be
//! (spec: CP-1).
//!
//! Four things are the adapter's whole job beyond the SQL:
//!
//! - **Two groups of tables.** The Library-wide ones are exactly what an Index
//!   Snapshot carries; the device-local ones are never uploaded (spec: CK-7).
//!   They are separated by table rather than by column, so no query can carry
//!   device state into a Snapshot by forgetting a `WHERE`.
//! - **Bytewise Entry Paths.** Every path column is `COLLATE BINARY`, and
//!   SQLite's case-folding collation appears nowhere: equality is
//!   case-sensitive and NFC merges neither case nor width variants, so two
//!   paths that differ in either are two rows (spec: EP-3).
//! - **A layout it opens, discards, or refuses — never migrates.** A file at
//!   the version this build writes is opened untouched. An older one whose
//!   device-local tables this build still reads keeps them and loses only its
//!   catalog, which the next catch-up rebuilds from Storage (spec: RV-5).
//!   Anything else is refused rather than guessed at.
//! - **A file two processes may hold at once.** One catalog is one file and one
//!   connection per process, and more than one process is the ordinary
//!   arrangement rather than a mistake: a server answering a browser while a
//!   sync runs in a terminal. Write-ahead logging is what lets those coexist,
//!   and a busy timeout is what a writer meeting the other writer spends instead
//!   of failing. See [`SqliteIndex`].
//!
//! A file [`SqliteIndex::open`] refuses is not always a dead end for this
//! device's own state: the `mappings` table's `prefix` and `local_root`
//! columns are the one part of the device-local group every layout has kept,
//! and [`RefusedIndex`] reads exactly those two, with no `prepare` and no
//! write of its own, so a refusal does not cost the owner the one record of
//! where this device maps the Library onto its folders (spec: EP-9).
//!
//! Where a Library's catalog lives by default, and the permissions its file is
//! created with, are the composition root's business. This crate is handed a
//! path.
//!
//! ```no_run
//! use coffret_usecase::{Index, IndexResult};
//! use coffret_sqlite_index::SqliteIndex;
//!
//! # async fn example() -> IndexResult<()> {
//! let index = SqliteIndex::open("/var/lib/coffret/library-alpha.sqlite")?;
//! let checkpoint = index.checkpoint().await?;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod device_state;

mod error;

mod library_state;

mod path_prefix;

mod query;

mod refused_index;
pub use refused_index::RefusedIndex;

mod rows;

mod schema;

mod sqlite_index;
pub use sqlite_index::SqliteIndex;
