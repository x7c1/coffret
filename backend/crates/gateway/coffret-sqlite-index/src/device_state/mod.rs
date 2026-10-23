//! The tables that never leave this device.
//!
//! Nothing here is ever part of an Index Snapshot (spec: CK-7), and the
//! Library-wide operations reach these tables only to keep each
//! materialization record about the Entry it names (spec: EP-10, FM-15).

mod held_parcels;
pub(crate) use held_parcels::{held_parcels, hold_parcel, let_go_parcel};

mod local_entries;
pub(crate) use local_entries::{
    awaiting_relocation, entries_absent_under, forget_local_entry, local_entry_at, mark_absent,
    mark_present, present_under, present_without_entry, reconcile, reconcile_all,
    relocate_local_entries,
};

mod mappings;
pub(crate) use mappings::{mappings, set_mapping};

mod reservations;
pub(crate) use reservations::{
    extend_reservation, mark_received, release_reservation, reservations, reserve,
};

mod pending_rows;
pub(crate) use pending_rows::{
    attempted_slots, clear_pending_row, mark_commit_attempted, mark_spooled, pending_rows,
    record_pending_row,
};

mod entry_records;
pub(crate) use entry_records::{
    carry_over, copies_of, current_containers, holds_entry_of, standing, standing_in,
};
