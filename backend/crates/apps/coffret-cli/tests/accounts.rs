//! What the command line says about the Storage accounts a device holds
//! (spec: SA-8).
//!
//! Every refusal here is made before a Passphrase is read or a consent asked
//! for, so each case runs the binary with nothing on standard input and a
//! state directory laid out by hand: a Library's settings file, and the
//! account directory it names.

mod support;

use std::path::Path;

use support::{code, stderr, write_file, Device};

/// The OAuth client a laid-out account was consented to.
const CLIENT: &str = "first.apps.googleusercontent.com";

/// Lays out a Drive Library called `library` referencing the account `account`,
/// and the account itself, consented to through `client`.
fn lay_out(device: &Device, library: &str, account: &str, client: &str) {
    let state = device
        .libraries()
        .parent()
        .expect("Libraries are kept under the state directory")
        .to_path_buf();
    write_file(
        &device.libraries().join(library).join("settings.json"),
        format!(
            r#"{{"version":1,"library_id":"0123456789abcdef","provider":{{"kind":"drive","folder_id":"f","client_id":"{client}","account":"{account}"}}}}"#
        )
        .as_bytes(),
    );
    write_file(
        &state.join("accounts").join(account).join("settings.json"),
        format!(r#"{{"version":1,"client_id":"{client}"}}"#).as_bytes(),
    );
}

/// `init` putting a Drive Library called `name` here through `client`.
fn init_drive<'a>(name: &'a str, client: &'a str, account: Option<&'a str>) -> Vec<&'a str> {
    let mut arguments = vec![
        "init",
        "--name",
        name,
        "--drive",
        "--parent",
        "1a2B3c",
        "--client-id",
        client,
        "--passphrase-stdin",
    ];
    if let Some(account) = account {
        arguments.extend(["--account", account]);
    }
    arguments
}

/// Asserts that a run was refused, saying `said`, and put no Library `name`
/// here.
fn refused(device: &Device, arguments: &[&str], said: &str, name: &str) {
    let output = device.run(arguments);
    let printed = stderr(&output);
    assert_eq!(
        code(&output),
        1,
        "{arguments:?} must fail; stderr was:\n{printed}"
    );
    assert!(
        printed.contains(said),
        "{arguments:?} must say {said:?}; stderr was:\n{printed}"
    );
    assert!(!Path::new(&device.libraries().join(name).join("settings.json")).exists());
}

// SA-8: a name that is not a plain directory name is refused with the sentence
// that says what a name may be, before anything is asked for — by each of the
// three commands that take one.
#[test]
fn an_account_name_that_is_not_one_is_refused_saying_what_one_is() {
    let device = Device::new();
    for (command, name) in [
        ("init", "bad-init"),
        ("join", "bad-join"),
        ("authorize", "bad-authorize"),
    ] {
        let mut arguments = match command {
            "init" => init_drive(name, CLIENT, None),
            "authorize" => vec!["authorize", "--passphrase-stdin"],
            _ => vec![
                "join",
                "--name",
                name,
                "--drive",
                "--folder-id",
                "1a2B3c",
                "--client-id",
                CLIENT,
                "--recovery-code-stdin",
                "--passphrase-stdin",
            ],
        };
        arguments.extend(["--account", "work/home"]);
        refused(
            &device,
            &arguments,
            "\"work/home\" cannot name an account: an account name is 1 to 64 characters, each an \
             ASCII letter, a digit, '-' or '_'",
            name,
        );
    }
}

// SA-8: optional while the device holds one account, required once it holds
// two.
#[test]
fn a_device_holding_two_accounts_requires_a_name() {
    let device = Device::new();
    lay_out(&device, "at-work", "work", CLIENT);
    lay_out(&device, "at-home", "home", CLIENT);

    refused(
        &device,
        &init_drive("unnamed", CLIENT, None),
        "this device holds 2 accounts, so which one the Library uses has to be named: give \
         --account with the name of one of them, or a new name to consent as another",
        "unnamed",
    );
}

// SA-8: one account name binds to one OAuth client, and a Library naming
// another is refused with both clients named.
#[test]
fn a_library_of_another_client_is_refused_the_account() {
    let device = Device::new();
    lay_out(&device, "first", "default", CLIENT);

    refused(
        &device,
        &init_drive("second", "second.apps.googleusercontent.com", None),
        "the Library \"second\" names the OAuth client \"second.apps.googleusercontent.com\" and \
         the account \"default\" names \"first.apps.googleusercontent.com\"; an account's grant \
         is used only through the client it was consented to; give --account a new name to \
         consent through the Library's client as another account",
        "second",
    );
}

// SA-8: renewal is per account, named by itself or by a Library; one of the two
// has to be named, and an account the device does not hold is refused before a
// Passphrase is read.
#[test]
fn authorize_names_an_account_or_a_library() {
    let device = Device::new();

    let output = device.run(&["authorize", "--passphrase-stdin"]);
    assert_eq!(code(&output), 1, "stderr was:\n{}", stderr(&output));
    assert!(
        stderr(&output).contains("--library") && stderr(&output).contains("--account"),
        "the refusal names both ways to say which grant: {}",
        stderr(&output)
    );

    refused(
        &device,
        &["authorize", "--account", "nowhere", "--passphrase-stdin"],
        "no account \"nowhere\" is on this device",
        "nowhere",
    );
}

// The flag is Drive's: an S3 Library references no account.
#[test]
fn an_account_is_not_named_for_an_s3_library() {
    let device = Device::new();
    let output = device.run(&[
        "init",
        "--name",
        "on-s3",
        "--s3",
        "--bucket",
        "photos",
        "--account",
        "work",
        "--passphrase-stdin",
    ]);
    assert_eq!(code(&output), 1, "stderr was:\n{}", stderr(&output));
    assert!(stderr(&output).contains("--account"), "{}", stderr(&output));
}

// SA-9: a script has nobody to ask for another Library's Passphrase, so where
// its one Passphrase opens none of the Libraries referencing the account, the
// refusal names the Library whose Passphrase would.
#[test]
fn a_script_is_told_which_library_s_passphrase_opens_the_account() {
    let device = Device::new();
    lay_out(&device, "at-work", "work", CLIENT);

    let output = device.run_with(
        &init_drive("second", CLIENT, Some("work")),
        Some("a passphrase of its own"),
    );
    let printed = stderr(&output);
    assert_eq!(code(&output), 1, "stderr was:\n{printed}");
    assert!(
        printed.contains(
            "the account \"work\" opens only through a Library that references it, and the \
             Passphrase given does not open one; the Passphrase of the Library \"at-work\" does"
        ),
        "stderr was:\n{printed}"
    );
    assert!(!device.libraries().join("second").exists());
}
