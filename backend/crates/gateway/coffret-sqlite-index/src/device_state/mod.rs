//! The five tables that never leave this device.
//!
//! Nothing here is ever part of an Index Snapshot (spec: CK-7), and nothing the
//! Library-wide operations do reaches these tables.

mod held_parcels;
pub(crate) use held_parcels::{held_parcels, hold_parcel, let_go_parcel};

mod local_entries;
pub(crate) use local_entries::{
    forget_local_entry, local_entry_at, mark_absent, mark_present, present_under,
    present_without_entry,
};

mod mappings;
pub(crate) use mappings::{mappings, set_mapping};

mod pending_rows;
pub(crate) use pending_rows::{
    attempted_slots, clear_pending_row, mark_commit_attempted, mark_spooled, pending_rows,
    record_pending_row,
};

mod relocations;
pub(crate) use relocations::{
    follow_adopted, follow_renames, followed_entries, following_any_of, release_vacated,
};
