use std::path::PathBuf;

use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;

use crate::error::{Error, Result};
use crate::log_settings::LogSettings;
use crate::rotating_files::RotatingFiles;

/// Points this process's events at a file, and reports which file.
///
/// Called once, by whatever *builds* an application: a binary, an example, a
/// test harness that talks to a live API. No library crate calls it — they emit
/// events and leave the question of where events go to whoever assembled the
/// program, which is what lets a test, or an embedding binary that wants no
/// logging of its own, run the same code with nothing collecting anything.
///
/// Two filters stand in front of the file, and they answer different questions.
/// The level says how loud an event has to be, and is capped at `DEBUG` by
/// [`LogSettings::with_level`] so that no setting can turn on the instrumentation
/// where an HTTP stack and a cloud SDK print headers and signing material. The
/// target says whose event it is, and keeps the file to coffret's own crates
/// unless somebody names another. Neither can substitute for the other: naming
/// a dependency's target lets through the same `DEBUG` that coffret's own
/// crates already reach the file with, and never anything below it.
///
/// The returned path is worth reporting to whoever started the program: a log
/// nobody can find is not evidence.
pub fn install(settings: &LogSettings) -> Result<PathBuf> {
    let files = RotatingFiles::open(settings).map_err(|cause| Error::Directory {
        path: settings.directory().to_path_buf(),
        cause,
    })?;
    let path = files.current_path();

    let wanted = settings.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(files)
        // A file is read by a text editor, not by a terminal: colour codes in
        // it are noise, and they would also break anything grepping through it.
        .with_ansi(false)
        .with_max_level(settings.level())
        .finish()
        .with(filter_fn(move |metadata| wanted.keeps(metadata.target())));

    tracing::subscriber::set_global_default(subscriber).map_err(|_| Error::AlreadyInstalled)?;
    Ok(path)
}
