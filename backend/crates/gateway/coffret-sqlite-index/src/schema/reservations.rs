use coffret_usecase::IndexResult;
use rusqlite::{Connection, TransactionBehavior};

use super::{
    replace_the_catalog, report_the_discard, stamp, unsupported, RESERVATIONS_DDL, SCHEMA_VERSION,
};
use crate::error::classify;

/// Brings a layout-17 file to this one, keeping every row of its device-local
/// group, laying out the tables of reservations empty — no build before
/// layout 18 recorded a drop's choice of a Pack, so nothing the file holds
/// waits for one (spec: PK-17) — and discarding its catalog, whose Entries
/// carry no deposit time, for the next catch-up to rebuild from Storage
/// (spec: RV-5, FM-15).
///
/// One `BEGIN IMMEDIATE` covers it, from re-reading the stamp to writing the new
/// one, for the reason a discard is one: a second process opening the file at
/// the same moment reads the stamp the first left and does nothing, and a crash
/// part-way leaves the layout-17 file as it was.
pub(super) fn upgrade(connection: &mut Connection) -> IndexResult<()> {
    const OPERATION: &str = "adding the reservations to an older layout";
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify(OPERATION))?;
    match stamp(&transaction)? {
        SCHEMA_VERSION => return Ok(()),
        17 => {}
        found => return Err(unsupported(found)),
    }
    transaction
        .execute_batch(RESERVATIONS_DDL)
        .map_err(classify(OPERATION))?;
    replace_the_catalog(&transaction)?;
    transaction.commit().map_err(classify(OPERATION))?;
    report_the_discard(OPERATION, 17);
    Ok(())
}

/// Brings a layout-18 or layout-19 file to this one by giving its reservations
/// the mark saying whether their drop finished receiving, unset on every one
/// (spec: PK-17, LA-12).
///
/// Unset because no build before layout 20 said a drop had finished: a
/// reservation such a build left may be one whose server stopped part way
/// through receiving it, and packing it as it stands would leave the rest of
/// the book, dropped again, for a second Pack. So each is read as a drop cut
/// off, which waits for its folder to be dropped again — a drop that had in
/// fact finished included: its files stay out of a Pack, and out of what a sync
/// carries, until the next drop of its folder takes it over and packs it whole,
/// and each start counts it among the drops in progress until then. A layout-19
/// file keeps its catalog; a layout-18 one then loses it, whose Entries carry
/// no deposit time, for the next catch-up to rebuild from Storage
/// (spec: RV-5, FM-15).
///
/// One `BEGIN IMMEDIATE` covers it, for the reason [`upgrade`] gives.
pub(super) fn add_the_received_mark(connection: &mut Connection) -> IndexResult<()> {
    const OPERATION: &str = "adding the received mark to the reservations";
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify(OPERATION))?;
    let found = match stamp(&transaction)? {
        SCHEMA_VERSION => return Ok(()),
        found @ (18 | 19) => found,
        found => return Err(unsupported(found)),
    };
    transaction
        .execute_batch(
            "ALTER TABLE reservations
                 ADD COLUMN received INTEGER NOT NULL DEFAULT 0 CHECK (received IN (0, 1));",
        )
        .map_err(classify(OPERATION))?;
    if found == 18 {
        replace_the_catalog(&transaction)?;
    } else {
        transaction
            .pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(classify("stamping the Index file's schema version"))?;
    }
    transaction.commit().map_err(classify(OPERATION))?;
    if found == 18 {
        report_the_discard(OPERATION, found);
    }
    Ok(())
}
