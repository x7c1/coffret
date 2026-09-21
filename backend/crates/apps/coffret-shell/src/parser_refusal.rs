//! The argument parser's own refusal, read before it is printed (spec: DK-10).
//!
//! [`stdin_flags`](crate::stdin_flags) catches a secret written after a flag
//! that names one, and it can do that before parsing because the flag in front
//! of the value says what the value is. A secret written as a bare argument —
//! `coffret sync --library alpha <passphrase>`, `coffret join --name second
//! <recovery code>`, `coffret <recovery code>` — has nothing in front of it to
//! recognise, so nothing short of the parser can tell it from a mistyped
//! command. It is caught here instead, in what the parser answered: clap
//! refuses an argument it did not expect by quoting it (`unexpected argument
//! 'X' found`, or `unrecognized subcommand 'X'`), and a quoted argument on
//! standard error is that secret on standard error and in whatever collects it.
//!
//! An argument that starts with `-` keeps clap's own message. A flag is not a
//! secret, and a person who typed `--folder` needs to see which flag was wrong
//! and the suggestion clap offers for it. One flag is carved out of that:
//! `--client-secret`, whose nearest name in the commands that have one is
//! `--client-id`, so clap's help would be an invitation to paste a secret
//! where the id goes.

use clap::error::{ContextKind, ContextValue, ErrorKind};

use crate::stdin_flags::already_seen;

/// The refusal an argument the parser did not expect deserves — never quoting
/// it — and `None` for a refusal that is clap's own to print.
///
/// `Some` is the answer for an unexpected argument that is not spelled like a
/// flag, and for an unrecognised subcommand: both are places a person may have
/// typed a Passphrase or a Recovery Code, and neither is a place where quoting
/// what was typed would help anyone.
pub fn argument_refused_without_being_quoted(error: &clap::Error) -> Option<String> {
    let argument = match error.kind() {
        // `unexpected argument 'X' found`: X is a value nothing asked for, and
        // what a person writes where nothing asked for anything is, often
        // enough, the secret they thought the command took.
        ErrorKind::UnknownArgument => quoted(error, ContextKind::InvalidArg),
        // `unrecognized subcommand 'X'`: the top level of a command, where a
        // Recovery Code pasted on its own lands.
        ErrorKind::InvalidSubcommand => quoted(error, ContextKind::InvalidSubcommand),
        _ => return None,
    };
    // A flag is not a secret, so clap's message — with the suggestion that goes
    // with it — is the more useful answer. Except for the one flag whose
    // suggestion is the harm.
    if argument.starts_with('-') {
        return client_secret_flag(&argument);
    }
    Some(refusal(&argument, error))
}

/// The flag that never existed and would be guessed at, answered here instead.
///
/// `--client-secret` is spelled a character or two from `--client-id`, which
/// the commands that reach Drive do have, so clap answers it by offering that
/// name — and a person who takes the offer pastes their secret where the id
/// goes: into this shell's history, into the process table, and from there into
/// the Library's settings as the client it was created as. The answer is ours
/// instead, and it says where a secret is read from.
///
/// Nothing of what was typed is repeated, the `--client-secret=value` spelling
/// included: what stands after the `=` there is the secret itself.
///
/// Which run reads the variable is named rather than left open the way
/// [`refusal`] leaves it, and for the same reason it is left open there: this
/// flag is refused on every command of both binaries, and creating or joining
/// a Library is the only run that reads the variable. "Run this again" would
/// be telling somebody who typed it on a sync, or on the server, to set a
/// variable and watch a run that never looks at it.
fn client_secret_flag(argument: &str) -> Option<String> {
    let typed = argument.split('=').next().unwrap_or(argument);
    if typed != "--client-secret" {
        return None;
    }
    Some(format!(
        "there is no --client-secret. A client secret is read from the environment variable \
         COFFRET_DRIVE_CLIENT_SECRET and from nowhere else, because an argument would leave it \
         in this shell's history and in the process table; set that variable for the command \
         that creates or joins a Library, and run that again. It does not belong in \
         --client-id either — that flag takes the client's id, which is not a secret and is \
         not stored as one. {}",
        already_seen("a client secret typed as an argument"),
    ))
}

/// What clap would have quoted, or an empty string if it carried nothing to
/// quote.
///
/// clap always carries the argument it refused. Were that ever not so, a flag
/// could not be told from a secret, and the only answer that cannot leak one is
/// the one that prints neither — which is what an empty string asks for here.
fn quoted(error: &clap::Error, kind: ContextKind) -> String {
    match error.get(kind) {
        Some(ContextValue::String(argument)) => argument.clone(),
        _ => String::new(),
    }
}

/// What is said instead of what was typed.
///
/// The parts of clap's own message that name nothing a person typed — the name
/// it thought was meant, the usage line, and where the rest of the answer is —
/// are kept. Without them, a mistyped subcommand is answered with nothing but a
/// paragraph about secrets and no way on.
fn refusal(argument: &str, error: &clap::Error) -> String {
    // Which command asks for them is left open. The two flags are on the
    // subcommands that read a secret, and a refusal that promised the command
    // in hand takes them would send a person to an argument that command would
    // itself refuse: `coffret-server` has no `--recovery-code-stdin`, and
    // neither has `coffret mappings`.
    let mut refusal = format!(
        "this command takes no such argument. A Passphrase or a Recovery Code is asked for at \
         the prompt of the command that needs it, or, for a script, read from one line of \
         standard input behind that command's `--passphrase-stdin` or `--recovery-code-stdin`; \
         neither is ever written as an argument. {}",
        already_seen("a secret typed as an argument"),
    );
    if let Some(tip) = similar_subcommands(error, argument) {
        refusal.push_str(&format!("\n\n  {tip}"));
    }
    if let Some(usage) = usage(error, argument) {
        refusal.push_str(&format!("\n\n{usage}"));
    }
    refusal.push_str("\n\nFor more information, try '--help'.");
    refusal
}

/// clap's own `did you mean`, when it offered one and it is not the argument
/// itself.
///
/// What clap offers is a name out of the command's own list — it reaches for one
/// only where what was typed is a near miss of a name it knows — so it is never
/// the argument itself, and it is the part of clap's message most worth keeping:
/// a mistyped subcommand is the likeliest way to arrive here, and the name that
/// was meant is the whole answer to it. A secret, being nothing like a
/// subcommand, draws no suggestion at all.
fn similar_subcommands(error: &clap::Error, argument: &str) -> Option<String> {
    // Nothing known to have been typed is nothing to check a suggestion
    // against, and the usage line is held back on the same grounds.
    if argument.is_empty() {
        return None;
    }
    let suggested = match error.get(ContextKind::SuggestedSubcommand)? {
        ContextValue::Strings(names) => names.clone(),
        ContextValue::String(name) => vec![name.clone()],
        _ => return None,
    };
    // Nothing leaves here unchecked against what was typed. The check is
    // equality and not containment, which the usage line can afford and this
    // cannot: the commonest miss of all is a name cut short — `syn` for `sync`,
    // `fetc` for `fetch` — and the name that was meant contains it, so dropping
    // a suggestion for carrying what was typed would drop it in exactly the
    // case it exists for. What a suggestion cannot be is the typed string
    // itself: that would have parsed rather than been refused.
    let suggested: Vec<String> = suggested
        .into_iter()
        .filter(|name| name.as_str() != argument)
        .map(|name| format!("'{name}'"))
        .collect();
    match suggested.as_slice() {
        [] => None,
        [one] => Some(format!("tip: a similar subcommand exists: {one}")),
        many => Some(format!(
            "tip: some similar subcommands exist: {}",
            many.join(", ")
        )),
    }
}

/// clap's usage line, when the error carries one and that line does not carry
/// `argument`.
///
/// The check is the whole point of reaching for this line rather than
/// `Error::render`, which puts the argument back. The usage line is worth
/// printing because it says what the command does take; it is worth printing
/// only while it is known not to say what was typed.
fn usage(error: &clap::Error, argument: &str) -> Option<String> {
    let usage = match error.get(ContextKind::Usage)? {
        ContextValue::StyledStr(usage) => usage.to_string(),
        ContextValue::String(usage) => usage.clone(),
        _ => return None,
    };
    match argument.is_empty() || usage.contains(argument) {
        true => None,
        false => Some(usage),
    }
}

#[cfg(test)]
mod tests {
    use clap::{Arg, ArgAction, Command};

    use super::*;

    /// What a leak would have put on a terminal. Nothing else in the suite
    /// spells it, so finding it anywhere is finding it having escaped.
    const TYPED: &str = "coffret1-sentinel-8c4d2e70-never-echoed";

    /// A command shaped like the two binaries: subcommands, and a flag that
    /// takes no value. It declares no positional, so anything bare is an
    /// argument the parser did not expect.
    fn command() -> Command {
        Command::new("coffret")
            .subcommand_required(true)
            .subcommand(
                Command::new("sync").arg(Arg::new("flag").long("flag").action(ArgAction::SetTrue)),
            )
    }

    /// What the parser answers `arguments` with, run through the guard.
    fn refused(arguments: &[&str]) -> Option<String> {
        let error = command()
            .try_get_matches_from(arguments)
            .expect_err("the parser must refuse these arguments");
        argument_refused_without_being_quoted(&error)
    }

    // DK-10: an argument clap did not expect is refused without being repeated,
    // wherever in the command line it was typed.
    #[test]
    fn an_argument_the_parser_did_not_expect_is_refused_without_being_quoted() {
        for arguments in [
            // A bare value after a subcommand that takes none.
            vec!["coffret", "sync", TYPED],
            // One in the subcommand's own place, which is where a Recovery Code
            // pasted on its own lands.
            vec!["coffret", TYPED],
            // And one after a flag that takes no value, which the parser meets
            // as a positional rather than as that flag's value.
            vec!["coffret", "sync", "--flag", TYPED],
        ] {
            let said = refused(&arguments)
                .unwrap_or_else(|| panic!("{arguments:?} must be refused by this guard"));

            assert!(!said.contains(TYPED), "{said}");
            assert!(said.contains("takes no such argument"), "{said}");
            assert!(said.contains("standard input"), "{said}");
            // And that the secret already typed is one that has been seen: the
            // run is refused, but the argument list it travelled in and the
            // shell's record of it are not undone.
            assert!(said.contains("having been seen"), "{said}");
        }
    }

    // A mistyped flag is not a secret, and hiding it would cost the person the
    // one thing they need — which flag was wrong — for nothing.
    #[test]
    fn a_mistyped_flag_keeps_the_parsers_own_message() {
        assert_eq!(refused(&["coffret", "sync", "--flg"]), None);
    }

    // A mistyped subcommand is the likeliest way to reach this guard, and what
    // it needs is the name that was meant — which clap works out from its own
    // list of subcommands, so saying it repeats nothing that was typed.
    #[test]
    fn a_mistyped_subcommand_is_still_told_which_one_was_meant() {
        let said = refused(&["coffret", "snyc"]).expect("a name no subcommand has is refused here");

        assert!(said.contains("'sync'"), "{said}");
        assert!(!said.contains("snyc"), "{said}");
        // And the way to the rest of the answer, which every refusal written
        // for an argument that is not spelled like a flag ends with — it is all
        // there is where clap has no guess to offer.
        assert!(said.contains("try '--help'"), "{said}");
    }

    // The case the equality check in `similar_subcommands` is there for: a
    // containment check would drop this tip.
    #[test]
    fn a_subcommand_cut_short_is_told_the_whole_name() {
        let said = refused(&["coffret", "syn"]).expect("a name no subcommand has is refused here");

        assert!(
            said.contains("tip: a similar subcommand exists: 'sync'"),
            "{said}"
        );
    }

    // The flag that was removed, whose nearest name takes the client's id. A
    // person who took clap's suggestion would paste their secret into it.
    #[test]
    fn the_secret_flag_that_no_longer_exists_says_where_a_secret_is_read_from() {
        let joined = format!("--client-secret={TYPED}");
        for arguments in [
            vec!["coffret", "sync", "--client-secret", TYPED],
            vec!["coffret", "sync", joined.as_str()],
        ] {
            let said = refused(&arguments)
                .unwrap_or_else(|| panic!("{arguments:?} must be refused by this guard"));

            assert!(!said.contains(TYPED), "{said}");
            assert!(said.contains("COFFRET_DRIVE_CLIENT_SECRET"), "{said}");
            assert!(said.contains("--client-id"), "{said}");
            // And that what was typed after it is a secret that has been seen.
            assert!(said.contains("having been seen"), "{said}");
        }
    }

    // Everything else the parser answers with — a missing required value, a
    // `--help` that is not a refusal at all — is clap's to print.
    #[test]
    fn a_refusal_about_no_argument_in_particular_is_left_alone() {
        assert_eq!(refused(&["coffret"]), None);
    }
}
