use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use coffret_device::{MovePreview, RenamePair};
use serde::Serialize;

use crate::api_error::ApiError;
use crate::entry_query::shaped;
use crate::moves::{move_pairs, refusals_of};
use crate::reported::Reported;
use crate::state::ServerState;

use super::work::{MoveDto, PairDto};
use super::RefusalDto;

/// `POST /api/move?from=<path>&to=<path>&from=…&to=…`
///
/// Moves, or renames, the files and folders the query names, in one Journal
/// record (spec: FM-15): each `from` with everything under it to its `to`,
/// nothing read, encrypted or uploaded, and no Pack rebuilt. It is what the
/// explorer's drag onto a folder, "Rename…" and "Move to…" arm once the person
/// has been shown what [`GET /api/move`](preview) counted and has said yes.
///
/// Once the record has committed, this device's own files of the renamed
/// Entries follow them — within a mapped folder, or across two on one
/// filesystem — and those that cannot follow leave this device, an unedited
/// one discarded into the trash (spec: EP-10). It answers what it did, `200`,
/// once all of that is done — where the files could not be relocated after
/// the commit, with what kept them — and the move is logged with the rest of
/// the server's finished work (spec: LA-12).
///
/// Refused before anything is written exactly where the preview says it is
/// refused, asked again now: a set the Library does not admit, a file of this
/// device's in it edited since it was placed, and a file under it the Library
/// does not hold yet — each a `409 refused_move` naming which, and not logged
/// (spec: LA-31). So is a set the rename's own plan refuses over the catalog
/// its catch-up brought to the head. A locked Library is refused as every
/// keyed route refuses it (spec: DK-2).
pub async fn move_entries(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Json<MoveDto>, ApiError> {
    let pairs = pairs_of(&query)?;
    // Asked before anything else that needs the Library, so a locked one is
    // refused as locked rather than as anything about the pairs.
    state.unlocked()?;
    let run = move_pairs(&state, pairs).await?;
    Ok(Json(MoveDto::of(&run)))
}

/// `GET /api/move?from=<path>&to=<path>&from=…&to=…`
///
/// What [`POST /api/move`](move_entries) of the same pairs would do, worked
/// out before it is asked for: how many Entries it renames; how many of this
/// device's files would follow them and how many would leave this device, and
/// what those that leave come to; and every refusal the move would meet, in
/// the shape every refusal takes — the set the Library does not admit, a file
/// of this device's edited since it was placed or that cannot be read to tell,
/// a file under it still on its way into the Library (spec: LA-30).
///
/// The count is the run's own plan over the catalog caught up to the head
/// (spec: CK-9), so the two can differ only where the Library or the disk
/// changes between them, or where a rename the preview counted as one that
/// follows is refused by the disk as crossing filesystems (spec: EP-10).
///
/// It changes nothing in the Library or on this device. A set the move would
/// refuse is answered `200` with its refusals, since showing them is what the
/// preview is for; a locked Library, and a query that is not a set of pairs,
/// are refused as they are for the `POST`.
pub async fn preview(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Json<MovePreviewDto>, ApiError> {
    let pairs = pairs_of(&query)?;
    let library = state.unlocked()?;
    let preview = library.preview_move(&pairs).await?;
    Ok(Json(MovePreviewDto::of(&pairs, &preview)))
}

/// The pairs a query names: `?from=` and `?to=` in turn, the n-th `to` the
/// destination of the n-th `from`. Anything else in it is not read.
///
/// A query that names no pair, or more of one side than of the other, is not
/// a set of pairs, and is refused before the Library is asked anything.
fn pairs_of(query: &[(String, String)]) -> Result<Vec<RenamePair>, ApiError> {
    let mut froms = Vec::new();
    let mut tos = Vec::new();
    for (name, value) in query {
        match name.as_str() {
            "from" => froms.push(shaped(value)?),
            "to" => tos.push(shaped(value)?),
            _ => {}
        }
    }
    if froms.is_empty() || froms.len() != tos.len() {
        return Err(ApiError::bad_path(
            "a move names each file or folder with `from` and where it goes with `to`, once \
             each, in turn",
        ));
    }
    Ok(froms
        .into_iter()
        .zip(tos)
        .map(|(from, to)| RenamePair::new(from, to))
        .collect())
}

/// What a move would do, as the explorer shows it before it offers to move.
#[derive(Serialize)]
pub struct MovePreviewDto {
    /// The pairs it was asked about, in the order the query named them.
    pairs: Vec<PairDto>,
    /// How many Entries the move would rename; `0` where the set is refused.
    entries: usize,
    /// How many of this device's files of them would follow, with one rename.
    following: usize,
    /// How many of them could not follow and would leave this device.
    leaving: usize,
    /// How many bytes those come to on this device.
    leaving_bytes: u64,
    /// This device's files in the move that changed since it placed them.
    edited: Vec<String>,
    /// The files under the move this device has and the Library does not hold
    /// yet.
    uploading: Vec<String>,
    /// Every refusal the move would meet, in the shape every refusal takes;
    /// empty where it may go ahead.
    refused: Vec<RefusalDto>,
}

impl MovePreviewDto {
    fn of(pairs: &[RenamePair], preview: &MovePreview) -> Self {
        Self {
            pairs: pairs.iter().map(PairDto::of).collect(),
            entries: preview.rename.renamed.len(),
            following: preview.rename.following,
            leaving: preview.rename.leaving,
            leaving_bytes: preview.rename.leaving_bytes,
            edited: preview
                .rename
                .edited
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
            uploading: preview
                .uploading
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
            refused: refusals_of(preview)
                .iter()
                .map(|refusal| RefusalDto::of(&Reported::of(refusal)))
                .collect(),
        }
    }
}
