//! The tables that never leave this device.
//!
//! Nothing here is ever part of an Index Snapshot (spec: CK-7), and the
//! Library-wide operations reach these tables only to keep each
//! materialization record about the Entry it names (spec: EP-10, FM-15).

mod held_parcels;
pub(crate) use held_parcels::{held_parcels, held_parcels_of, hold_parcel, let_go_parcel};

mod local_entries;
pub(crate) use local_entries::{
    any_remote_in, awaiting_relocation, entries_absent_under, forget_local_entry, local_entry_at,
    mark_absent, mark_present, present_in, present_outside, present_under, present_without_entry,
    reconcile, reconcile_all, relocate_local_entries,
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

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::held_parcels::HELD_PARCELS_OF;
    use super::local_entries::ANY_REMOTE_IN;
    use crate::query::plan;

    // LA-48: whether a page's folder is wholly here is a test for one Entry
    // of the folder, through the index on its parent, against its present
    // record on the primary key; and the parcels held of the Container a page
    // is read from are its own rows on the primary key. Neither scans a table.
    #[test]
    fn what_a_page_read_asks_of_the_device_s_records_is_found_by_key() {
        let mut connection = Connection::open_in_memory().expect("an in-memory database opens");
        crate::schema::prepare(&mut connection).expect("a fresh layout lays out");
        for sql in [ANY_REMOTE_IN, HELD_PARCELS_OF] {
            let steps = plan(&connection, sql);
            for step in &steps {
                // The outer `SELECT EXISTS (…)` reads no table: its one row
                // is a constant, and the subquery is what reads.
                assert!(
                    step.starts_with("SEARCH ")
                        || step.starts_with("SCALAR SUBQUERY")
                        || step == "SCAN CONSTANT ROW",
                    "{sql} reads by {step:?}, in {steps:?}"
                );
            }
        }
    }
}
