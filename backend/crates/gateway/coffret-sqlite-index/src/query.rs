use coffret_usecase::IndexResult;
use rusqlite::{Connection, Params, Row};

use crate::error::translate;

/// Reads every row a statement answers with.
///
/// It exists because rusqlite's own row iterators want the reader's error type
/// to be one this crate could convert a SQLite error into, and the port's error
/// type belongs to neither crate. Stepping the rows by hand costs one small
/// function and keeps the reader returning the port's own result.
pub(crate) fn collect<T>(
    connection: &Connection,
    sql: &str,
    params: impl Params,
    operation: &'static str,
    read: impl Fn(&Row<'_>) -> IndexResult<T>,
) -> IndexResult<Vec<T>> {
    let mut statement = connection.prepare(sql).map_err(translate(operation))?;
    let mut rows = statement.query(params).map_err(translate(operation))?;
    let mut collected = Vec::new();
    while let Some(row) = rows.next().map_err(translate(operation))? {
        collected.push(read(row)?);
    }
    Ok(collected)
}

/// Reads the first row a statement answers with, if there is one.
pub(crate) fn first<T>(
    connection: &Connection,
    sql: &str,
    params: impl Params,
    operation: &'static str,
    read: impl Fn(&Row<'_>) -> IndexResult<T>,
) -> IndexResult<Option<T>> {
    let mut statement = connection.prepare(sql).map_err(translate(operation))?;
    let mut rows = statement.query(params).map_err(translate(operation))?;
    match rows.next().map_err(translate(operation))? {
        Some(row) => read(row).map(Some),
        None => Ok(None),
    }
}
