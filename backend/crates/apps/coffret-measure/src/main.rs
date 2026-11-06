//! Measures what a filter over a whole Library would rest on, in the server.
//!
//! A filter of the explorer would match a typed name against every path
//! segment of every Entry, and pick the n most recently deposited books. This
//! times both as a full scan of the device's Index, the file the server reads:
//! every row of `entries` read, then matched or grouped in memory, by the logic
//! in [`matching`] and [`recent`]. Each is run several times and reported as
//! the median and the worst, in milliseconds, with the time to read the rows
//! alone beside them for comparison. It also names the folder with the most
//! rows ([`folders`]), where the explorer's listing is timed besides the root.
//!
//! The Index is opened read-only, and queried only: nothing here can write to
//! it. It is meant to be run while nothing serves the Library, so that what is
//! timed is the scan alone (`scripts/large.sh measure` runs it before it starts
//! the server).
//!
//! It is a measurement and not a route: nothing of the server calls it, and
//! the filter it measures for does not exist yet.

mod folders;
mod matching;
mod recent;

use anyhow::{bail, Context, Result};
use clap::Parser;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
struct Args {
    /// The Index file, `index.sqlite` in the Library's directory under the
    /// state directory
    #[arg(long)]
    index: PathBuf,

    /// A name to match, as typed; given once per query
    #[arg(long = "query", required = true)]
    queries: Vec<String>,

    /// How many recent books to pick; given once per n
    #[arg(long = "recent", default_values_t = [5, 50])]
    recents: Vec<usize>,

    /// How many times each measurement is run
    #[arg(long, default_value_t = 7)]
    runs: usize,

    /// Where to write the path of the folder with the most rows, as an Entry
    /// Path on one line (empty for the Library root)
    #[arg(long)]
    largest_folder_out: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.runs == 0 {
        bail!("--runs must be at least 1");
    }
    let index = open(&args)?;

    let rows = read(&index)?;
    println!("Entries in the Index: {}", rows.len());
    let largest = folders::largest_folder(rows.iter().map(|(path, _)| path.as_str()))
        .context("the Index holds no Entry, so there is nothing to measure")?;
    let largest_path = largest.folder.clone().unwrap_or_default();
    println!(
        "Largest folder: {} ({} rows)",
        shown(largest.folder.as_deref()),
        largest.children
    );
    if let Some(out) = &args.largest_folder_out {
        std::fs::write(out, format!("{largest_path}\n"))
            .with_context(|| format!("cannot write {}", out.display()))?;
    }

    println!();
    println!(
        "1. Full-scan match in the server: every row of the Index's `entries` read, then \
         matched in memory ({} runs each)",
        args.runs
    );
    let read_only = timed(args.runs, || read(&index).map(|rows| rows.len()))?;
    report("reading every row, nothing else", &read_only, None);

    for typed in &args.queries {
        let query = matching::Query::new(typed);
        let times = timed(args.runs, || {
            let rows = read(&index)?;
            Ok(query.count(rows.iter().map(|(path, _)| path.as_str())))
        })?;
        report(
            &format!("match {typed:?}"),
            &times,
            Some(format!("{} Entries match", times.answer)),
        );
    }

    for &n in &args.recents {
        let times = timed(args.runs, || {
            let rows = read(&index)?;
            let books =
                recent::most_recent_books(rows.iter().map(|(path, at)| (path.as_str(), *at)), n);
            Ok(books.len())
        })?;
        report(
            &format!("the {n} most recent books"),
            &times,
            Some(format!("{} books", times.answer)),
        );
    }
    Ok(())
}

/// The Index, opened read-only.
///
/// `query_only` on top of the read-only open, so that even a statement that
/// would write is refused by SQLite before it is tried.
fn open(args: &Args) -> Result<Connection> {
    let index = Connection::open_with_flags(
        &args.index,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("cannot open the Index {} read-only", args.index.display()))?;
    index
        .pragma_update(None, "query_only", true)
        .context("cannot make the Index connection query-only")?;
    Ok(index)
}

/// Every Entry's path and `deposited_at`, as the full scan reads them.
fn read(index: &Connection) -> Result<Vec<(String, i64)>> {
    let mut statement = index
        .prepare_cached("SELECT path, deposited_at FROM entries")
        .context("cannot read the Index's entries")?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .context("cannot read the Index's entries")?
        .collect::<rusqlite::Result<Vec<(String, i64)>>>()
        .context("cannot read a row of the Index's entries")?;
    Ok(rows)
}

/// One measurement's runs, and what the last of them answered.
struct Timings {
    millis: Vec<f64>,
    answer: usize,
}

impl Timings {
    fn median(&self) -> f64 {
        let mut sorted = self.millis.clone();
        sorted.sort_by(f64::total_cmp);
        let middle = sorted.len() / 2;
        if sorted.len() % 2 == 1 {
            sorted[middle]
        } else {
            (sorted[middle - 1] + sorted[middle]) / 2.0
        }
    }

    fn worst(&self) -> f64 {
        self.millis.iter().copied().fold(0.0, f64::max)
    }
}

/// Runs `measured` `runs` times, timing each run.
fn timed(runs: usize, mut measured: impl FnMut() -> Result<usize>) -> Result<Timings> {
    let mut millis = Vec::with_capacity(runs);
    let mut answer = 0;
    for _ in 0..runs {
        let started = Instant::now();
        answer = measured()?;
        millis.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    Ok(Timings { millis, answer })
}

fn report(what: &str, times: &Timings, answered: Option<String>) {
    let answered = answered.map(|a| format!("  ({a})")).unwrap_or_default();
    println!(
        "   {what:<40} median {:>9.1} ms   worst {:>9.1} ms{answered}",
        times.median(),
        times.worst()
    );
}

fn shown(folder: Option<&str>) -> String {
    match folder {
        None => "the Library root".to_owned(),
        Some(path) => format!("{path:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_median_is_the_middle_and_the_worst_the_slowest() {
        let odd = Timings {
            millis: vec![5.0, 1.0, 9.0],
            answer: 0,
        };
        assert_eq!((odd.median(), odd.worst()), (5.0, 9.0));
        let even = Timings {
            millis: vec![4.0, 1.0, 2.0, 8.0],
            answer: 0,
        };
        assert_eq!((even.median(), even.worst()), (3.0, 8.0));
    }

    #[test]
    fn reads_an_index_without_writing_to_it() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let path = dir.path().join("index.sqlite");
        {
            let index = Connection::open(&path).expect("a database");
            index
                .execute_batch(
                    "CREATE TABLE entries (path TEXT PRIMARY KEY, deposited_at INTEGER NOT NULL);
                     INSERT INTO entries VALUES ('Books/A/001.jpg', 10), ('Books/B/001.jpg', 20);",
                )
                .expect("rows");
        }
        let args = Args::parse_from([
            "coffret-measure",
            "--index",
            path.to_str().expect("UTF-8"),
            "--query",
            "a",
        ]);
        let index = open(&args).expect("opened");
        let mut rows = read(&index).expect("read");
        rows.sort();
        assert_eq!(
            rows,
            vec![
                ("Books/A/001.jpg".to_owned(), 10),
                ("Books/B/001.jpg".to_owned(), 20)
            ]
        );
        assert!(
            index.execute("DELETE FROM entries", []).is_err(),
            "the connection refuses a write"
        );
    }
}
