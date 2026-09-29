//! The command line a person keeps a Library from.
//!
//! Everything here is a shell. Each subcommand reads what was typed, hands
//! `coffret-device` a way to ask for the secrets it needs — a Passphrase, and
//! the Recovery Code where a Library is being joined — calls
//! it, and prints what came back; no flow, no layout, and no decision about
//! where a Library lives is made in this crate. That is what lets the
//! browser-based explorer's server do the same things without either of them
//! being the odd one out — and it is why the manifest depends on
//! `coffret-device` and on nothing beneath it.
//!
//! Starting a process is the one part not written here either: pointing the run
//! at its log file and reading a Passphrase from the terminal are the same in
//! both binaries, so both take them from `coffret-shell` — and the reader that
//! asks for a Recovery Code without echoing it is there too.
//!
//! What a run did goes to a log file under the state directory, and the file it
//! chose is printed to standard error so that whoever started the run can find
//! it. Standard output carries only what was asked for, so a Recovery Code, a
//! list of mappings, or a run's summary and its findings — the Keyring repairs
//! it performed among them — can be piped somewhere. A run that failed after a
//! Keyring repair still puts that repair there, in the same words, because the
//! replicas it put back stand whatever became of the run (spec: KL-15); the
//! failure itself goes to standard error.
//!
//! What a long run is *doing* goes to standard error for that reason: it is
//! neither an answer nor a failure, and a pipe reading the answer must not find
//! it there. [`progress`] is where it is rendered, and it renders differently
//! for a terminal and for anything else.
//!
//! # Three exit statuses
//!
//! `0` is a run that did everything it was asked to. `1` is a run that failed.
//! `2` is the one in between, and it is why the statuses are worth spelling out:
//! a sync that returns successfully may still have left a changed file inside a
//! Pack, and a fetch may still have declined half a folder (spec: PK-14,
//! EP-11). Those are findings rather than failures — the run did what it could
//! and said what it did not do — and a script that only asked whether the
//! command failed would call them a backup. So they get a status of their own.
//!
//! # Two forms of answer
//!
//! Everything above is the text form, which is written for a person. A script
//! passes `--json` instead and gets one JSON object on standard output when the
//! run finishes — what the command answered, or why it failed — and nothing
//! else there; standard error says what it always says. [`answer`] is where
//! that object is shaped, and why it is shaped from types of its own.

use std::path::Path;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use coffret_device::Findings;

// What a run answers a script with under `--json`.
mod answer;
use answer::{Document, Failure, Form, Ran};

mod authorize;
mod consent;
mod drive_client;
mod fetch;
mod freeze;
mod init;
mod join;

mod library_args;
use library_args::LibraryArgs;

mod map;
mod mappings;

// What a long run says while it runs, which is the one thing printed here that
// is neither an answer nor a failure.
mod progress;

mod recovery_code;

mod report;

mod storage_location;
mod sync;

#[derive(Parser)]
#[command(
    name = "coffret",
    version,
    about = "Keep a folder in an encrypted Library on Storage you do not have to trust"
)]
struct Cli {
    /// Answer in one JSON object on standard output instead of in text
    #[arg(long, global = true, long_help = answer::SHAPE)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a Library on this device and on Storage
    Init(init::InitArgs),
    /// Take up a Library another device created, from its Recovery Code
    Join(join::JoinArgs),
    /// Renew this device's grant on a Storage account, for every Library that
    /// references it
    Authorize(authorize::AuthorizeArgs),
    /// Record that a folder on this device holds part of the Library
    Map(map::MapArgs),
    /// List what this device has mapped
    Mappings(mappings::MappingsArgs),
    /// Print the Library's Recovery Code again
    RecoveryCode(LibraryArgs),
    /// Carry the mapped folders into the Library
    Sync(LibraryArgs),
    /// Pack the eligible local files in a folder directly into Packs
    Freeze(freeze::FreezeArgs),
    /// Put the Library into the mapped folders
    Fetch(fetch::FetchArgs),
}

impl Command {
    /// The subcommand as it is typed, which is how the `--json` answer names
    /// the command it answers for.
    fn name(&self) -> &'static str {
        match self {
            Self::Init(_) => "init",
            Self::Join(_) => "join",
            Self::Authorize(_) => "authorize",
            Self::Map(_) => "map",
            Self::Mappings(_) => "mappings",
            Self::RecoveryCode(_) => "recovery-code",
            Self::Sync(_) => "sync",
            Self::Freeze(_) => "freeze",
            Self::Fetch(_) => "fetch",
        }
    }
}

/// The kind a refusal of what was typed travels as under `--json`, before any
/// command has run.
const USAGE: &str = "usage";

#[tokio::main]
async fn main() -> ExitCode {
    // Whether a script asked for JSON, for the refusals below that are made
    // before the arguments are parsed — or because they could not be. A plain
    // look for the flag rather than the parser's reading of it, since there is
    // no reading yet.
    let asked_for = match std::env::args_os().any(|argument| argument == "--json") {
        true => Form::Json,
        false => Form::Text,
    };

    // Before clap, because clap refuses an argument it did not expect by
    // quoting it — and a value typed after `--recovery-code-stdin` or
    // `--passphrase-stdin` is the Recovery Code or the Passphrase itself
    // (spec: DK-10). What is said instead is what the flag is. A secret typed
    // with no flag in front of it has nothing to recognise before parsing, and
    // is caught below instead.
    if let Some(refusal) =
        coffret_shell::stdin_flags::value_typed_after_a_secret_flag(std::env::args_os())
    {
        return refused_before_running(asked_for, refusal.to_string());
    }

    // Parsed here rather than through `parse`, so that what a person typed
    // wrongly exits the way everything else that failed does: clap's own status
    // for a usage error is the one this binary spends on findings.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            // The other half of the guard above: an argument clap did not
            // expect is one it would refuse by quoting, and a bare argument
            // where none was asked for is where a Passphrase or a Recovery Code
            // pasted by hand lands (spec: DK-10). The refusal says so without
            // repeating it.
            if let Some(refusal) =
                coffret_shell::parser_refusal::argument_refused_without_being_quoted(&error)
            {
                return refused_before_running(asked_for, refusal.to_string());
            }
            // Everything else is clap's own to print, a mistyped flag included:
            // a flag is not a secret, and which one was wrong is what the
            // person needs to see.
            let _ = error.print();
            // `--help` and `--version` arrive here too, and they are answers
            // rather than refusals — in text either way, since they are about
            // the command line rather than a run of it.
            if !error.use_stderr() {
                return ExitCode::SUCCESS;
            }
            if asked_for == Form::Json {
                Document::failed(
                    None,
                    None,
                    Failure::plain(USAGE, parser_sentence(&error)),
                    &Findings::default(),
                )
                .print();
            }
            return ExitCode::FAILURE;
        }
    };

    let form = match cli.json {
        true => Form::Json,
        false => Form::Text,
    };
    let command = cli.command.name();

    let log = match coffret_shell::logging::start() {
        Ok(log) => log,
        Err(error) => return failed(form, command, None, &error.into()),
    };

    match run(cli.command, form).await {
        Ok(ran) => {
            if form == Form::Json {
                Document::succeeded(command, Some(&log), &ran).print();
            }
            ExitCode::from(ran.report.exit_status())
        }
        Err(error) => failed(form, command, Some(&log), &error),
    }
}

/// Runs the command, and hands back what it answered.
async fn run(command: Command, form: Form) -> anyhow::Result<Ran> {
    match command {
        Command::Init(args) => init::run(args, form).await,
        Command::Join(args) => join::run(args).await,
        Command::Authorize(args) => authorize::run(args).await,
        Command::Map(args) => map::run(args).await,
        Command::Mappings(args) => mappings::run(args, form).await,
        Command::RecoveryCode(args) => recovery_code::run(args, form),
        Command::Sync(args) => sync::run(args, form).await,
        Command::Freeze(args) => freeze::run(args, form).await,
        Command::Fetch(args) => fetch::run(args, form).await,
    }
}

/// Says that `command` failed with `error`, in the form asked for, and exits
/// as a failure.
///
/// What the run did before it failed, and then the whole chain — what failed,
/// and under it what each layer reported, down to the format crate's or the
/// provider's own words — with what a person can do about it after the cause
/// rather than before it. The chain goes to standard error in either form, for
/// the person watching; under `--json` the repairs are in the answer instead
/// of on lines of their own.
fn failed(
    form: Form,
    command: &'static str,
    log: Option<&Path>,
    error: &anyhow::Error,
) -> ExitCode {
    let failed = report::failed(error);
    if form.is_text() {
        for repaired in failed.repaired {
            println!("{repaired}");
        }
    }
    for line in failed.said {
        eprintln!("{line}");
    }
    if form == Form::Json {
        Document::failed(
            Some(command),
            log,
            Failure::of(error),
            &Findings::repaired_before(error.as_ref()),
        )
        .print();
    }
    ExitCode::FAILURE
}

/// A refusal of what was typed, made before any command ran — and so before
/// there was a log to name.
fn refused_before_running(form: Form, refusal: String) -> ExitCode {
    eprintln!("error: {refusal}");
    if form == Form::Json {
        Document::failed(
            None,
            None,
            Failure::plain(USAGE, refusal),
            &Findings::default(),
        )
        .print();
    }
    ExitCode::FAILURE
}

/// The sentence the argument parser refused with, without the usage and the
/// hints it prints under it.
fn parser_sentence(error: &clap::Error) -> String {
    let rendered = error.render().to_string();
    let first = rendered.lines().next().unwrap_or_default();
    first.strip_prefix("error: ").unwrap_or(first).to_owned()
}
