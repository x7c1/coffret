//! How a domain value is spelled in one column, and what one row of a table
//! holds.
//!
//! The two grow along different axes — a spelling is added when a domain type
//! gains a stored form, a row reader when a table gains one — so they are
//! separate files here, and the rows follow the schema's own division into the
//! Library-wide tables and the device-local ones.

mod catalog;
pub(crate) use catalog::{checkpoint, container_summary, entry_location};

mod columns;
pub(crate) use columns::{kind_text, spool_state_text, state_text, to_integer};

mod device;
pub(crate) use device::{local_entry, mapping, pending_upload, refused_mapping};
