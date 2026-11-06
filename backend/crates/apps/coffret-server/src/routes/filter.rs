use std::num::NonZeroUsize;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use coffret_device::{FilterAsk, FilterRow, FilterRowKind, LibraryFilter, NameQuery};
use serde::{Deserialize, Serialize};

use crate::api_error::ApiError;
use crate::state::ServerState;
use crate::timestamp::deposit_iso8601;

/// What a filter is asked with: `?name=`, `?recent=`, or both.
///
/// Both as text, read here rather than by the extractor, so that a `recent`
/// that is not a positive whole number is refused in the one shape every
/// refusal takes rather than in the extractor's own.
#[derive(Debug, Deserialize)]
pub struct FilterQuery {
    name: Option<String>,
    recent: Option<String>,
}

impl FilterQuery {
    /// The filter the query asks for, or the refusal of one it cannot be
    /// (spec: LA-42).
    fn ask(&self) -> Result<FilterAsk, ApiError> {
        let name = self
            .name
            .as_deref()
            .map(|typed| {
                NameQuery::new(typed).ok_or_else(|| ApiError::bad_filter("the name is empty"))
            })
            .transpose()?;
        let recent = self
            .recent
            .as_deref()
            .map(|count| {
                count.parse::<NonZeroUsize>().map_err(|_| {
                    ApiError::bad_filter("the count of recent books is not a positive whole number")
                })
            })
            .transpose()?;
        FilterAsk::new(name, recent)
            .ok_or_else(|| ApiError::bad_filter("it names neither a name nor a count of books"))
    }
}

/// The rows a filtered tree keeps (spec: LA-40, LA-41, LA-42).
#[derive(Serialize)]
pub struct FilterDto {
    /// The rows matched, in path order, at most
    /// [`MOST_FILTER_ROWS`](coffret_device::MOST_FILTER_ROWS) of them.
    rows: Vec<FilterRowDto>,
    /// How many rows matched over the whole Library, those not answered
    /// included.
    total: usize,
    /// Whether `rows` stops short of `total`.
    truncated: bool,
}

#[derive(Serialize)]
struct FilterRowDto {
    /// A file's Entry Path, or a folder's path; the Library root, which is a
    /// row only as a book, is the empty string.
    path: String,
    kind: KindDto,
    /// For a folder, the newest deposit time among the files directly in it,
    /// ISO 8601 in UTC, and `null` where it holds none directly or no calendar
    /// reaches it. Absent on a file.
    #[serde(skip_serializing_if = "Option::is_none")]
    deposited_at: Option<Option<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum KindDto {
    Folder,
    File,
}

impl FilterDto {
    fn of(filter: LibraryFilter) -> Self {
        Self {
            rows: filter.rows.into_iter().map(FilterRowDto::of).collect(),
            total: filter.total,
            truncated: filter.truncated,
        }
    }
}

impl FilterRowDto {
    fn of(row: FilterRow) -> Self {
        let (kind, deposited_at) = match row.kind {
            FilterRowKind::Folder => (
                KindDto::Folder,
                Some(row.deposited_at.and_then(deposit_iso8601)),
            ),
            FilterRowKind::File => (KindDto::File, None),
        };
        Self {
            path: row.path,
            kind,
            deposited_at,
        }
    }
}

/// `GET /api/filter?name=…&recent=…`
///
/// Out of the catalog alone, and nothing of Storage. Refused while the Library
/// is locked, as every route that reads the catalog is (spec: DK-2), before
/// the query is read, as the listing is.
pub async fn filter(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<FilterQuery>,
) -> Result<Json<FilterDto>, ApiError> {
    let library = state.unlocked()?;
    let ask = query.ask()?;
    let filter = library.filter(&ask).await?;
    Ok(Json(FilterDto::of(filter)))
}
