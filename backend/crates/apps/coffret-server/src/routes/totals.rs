use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use coffret_device::KeptTally;
use serde::Serialize;

use crate::api_error::ApiError;
use crate::state::ServerState;
use crate::timestamp::iso8601;

/// What the whole Library comes to, for the explorer's details of the Library
/// itself (spec: LA-39).
///
/// Beside `/api/library` rather than in it: that answer is the one a locked
/// server still gives, read off the settings file, and these are read off the
/// catalog, which goes with the keys.
#[derive(Serialize)]
pub struct TotalsDto {
    /// How many objects the Library's current state rests on, as Storage
    /// counts objects: every current Container once, a Pack being one, the
    /// control head, and each replica of the committed Keyring.
    objects: u64,
    /// How many files the Library holds at every depth, and how many of them
    /// this device has (spec: EP-10).
    files_total: u64,
    files_present: u64,
    /// The plaintext bytes of every file together, whether this device has
    /// them or not: what a folder's `size` is, over the whole Library.
    size: u64,
    /// When the newest file anywhere in the Library was modified, ISO 8601 in
    /// UTC; `null` for a Library holding no file, and for a count of seconds no
    /// calendar reaches.
    mtime: Option<String>,
    /// What the Library takes on this device's disk: the plaintext bytes of
    /// the `files_present` files, by the size each Entry records, and the
    /// bytes the held parcels' files take. Where the held parcels could not be
    /// counted (`held_parcels` is `null`), the present files' part alone.
    size_on_device: u64,
    /// The parcels this device holds for reading again (spec: PK-21), and
    /// `null` where they could not be counted.
    held_parcels: Option<HeldParcelsDto>,
}

/// The parcels this device holds, and what their files take on its disk.
///
/// A fetch holds every parcel it read until every Entry it covers that this
/// device maps is on the device or witnessed absent, or its Container leaves
/// the current set (spec: PK-21), and nothing else tells a person the
/// directory they are held in exists: a reader who opens the first page of
/// many books leaves a parcel or two behind for each, which over time is
/// gigabytes.
#[derive(Serialize)]
struct HeldParcelsDto {
    /// How many parcels are held.
    count: u64,
    /// How many bytes their files take on this device's disk.
    bytes: u64,
}

impl HeldParcelsDto {
    fn of(tally: KeptTally) -> Self {
        Self {
            count: tally.count,
            bytes: tally.bytes,
        }
    }
}

/// `GET /api/library/totals`
///
/// Out of the catalog and a stat of each held parcel's file, and nothing of
/// Storage: the explorer asks again each time what it shows of the Library
/// changes, which is after every run that moved it.
pub async fn totals(State(state): State<Arc<ServerState>>) -> Result<Json<TotalsDto>, ApiError> {
    let library = state.unlocked()?;
    let totals = library.totals().await?;
    let held = library.kept_tally().await;
    let size_on_device = totals.size_present + held.map_or(0, |tally| tally.bytes);
    Ok(Json(TotalsDto {
        objects: totals.objects,
        files_total: totals.files_total,
        files_present: totals.files_present,
        size: totals.size,
        mtime: totals.mtime.and_then(iso8601),
        size_on_device,
        held_parcels: held.map(HeldParcelsDto::of),
    }))
}
