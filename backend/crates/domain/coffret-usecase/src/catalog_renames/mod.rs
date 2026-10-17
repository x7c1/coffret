//! What applying a Journal record's `renames` asks of the catalog, and the
//! answer every [`Index`](crate::Index) applies (spec: FM-15, RV-5).
//!
//! A record is applied as one change: its removals taken out, then all of its
//! renames at once, then its additions added. The rules a rename has to hold
//! against the catalog are the same whatever the catalog is kept in, so they are
//! stated once, here, and each implementation hands over two lookups into its
//! own catalog — which Entry an element names, and which Entry stands at a path
//! — after it has taken the record's removals out and before it changes
//! anything else. What comes back is either every rename the record makes, with
//! the Entry it changes as it stands now, or the first rule an element breaks;
//! nothing is changed by asking, so a refusal leaves the implementation free to
//! give the record up whole.

mod check_rename_set;
pub use check_rename_set::check_rename_set;

mod check_renames;
pub use check_renames::check_renames;

mod named;
pub use named::Named;

mod renamed_entry;
pub use renamed_entry::RenamedEntry;
