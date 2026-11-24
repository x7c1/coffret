use coffret_usecase::IndexResult;
use rusqlite::{Connection, Params, Row};

use crate::error::classify;

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
    let mut statement = connection.prepare(sql).map_err(classify(operation))?;
    let mut rows = statement.query(params).map_err(classify(operation))?;
    let mut collected = Vec::new();
    while let Some(row) = rows.next().map_err(classify(operation))? {
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
    let mut statement = connection.prepare(sql).map_err(classify(operation))?;
    let mut rows = statement.query(params).map_err(classify(operation))?;
    match rows.next().map_err(classify(operation))? {
        Some(row) => read(row).map(Some),
        None => Ok(None),
    }
}

/// How SQLite plans one statement, a line for each step, every parameter bound
/// to the empty string: what a test of what a statement reads looks at.
#[cfg(test)]
pub(crate) fn plan(connection: &Connection, sql: &str) -> Vec<String> {
    let mut statement = connection
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .expect("a statement of this crate is one SQLite plans");
    let count = statement.parameter_count();
    let params: Vec<&dyn rusqlite::ToSql> = (0..count).map(|_| &"" as _).collect();
    statement
        .query_map(params.as_slice(), |row| row.get::<_, String>(3))
        .expect("planning must succeed")
        .collect::<Result<_, _>>()
        .expect("planning must succeed")
}
