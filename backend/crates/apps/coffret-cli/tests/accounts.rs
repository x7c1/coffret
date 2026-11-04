//! What the command line says about the Storage accounts a device holds
//! (spec: SA-8).
//!
//! Every refusal here is made before a consent is asked for, and before
//! anything reaches Storage, so each case runs the binary with at most a
//! Passphrase on standard input and a state directory laid out by hand: a
//! Library's settings file, its stored Master Key file, and the account
//! directory it names.

mod support;

use std::fs;
use std::sync::OnceLock;

use support::{
    code, stderr, stdout, stub_endpoint, succeeded, write_file, Device, PASSPHRASE, REGION,
};

/// The OAuth client a laid-out account was consented to.
const CLIENT: &str = "first.apps.googleusercontent.com";

/// A stored Master Key file kept under [`PASSPHRASE`], made once by `init` on
/// a device of its own and copied into every Library a case lays out.
fn stored_master_key() -> &'static [u8] {
    static STORED: OnceLock<Vec<u8>> = OnceLock::new();
    STORED.get_or_init(|| {
        let donor = Device::new();
        let output = donor.run_with(
            &[
                "init",
                "--name",
                "donor",
                "--s3",
                "--bucket",
                "photos",
                "--endpoint",
                stub_endpoint(),
                "--region",
                REGION,
                "--path-style",
                "--passphrase-stdin",
            ],
            Some(PASSPHRASE),
        );
        succeeded(&output, "init");
        fs::read(donor.library("donor").join("master-key.cfmk"))
            .expect("the stored Master Key must be readable")
    })
}

/// Lays out a Drive Library called `library` referencing the account `account`,
/// with a stored Master Key kept under [`PASSPHRASE`], and the account itself,
/// consented to through `client`.
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
        &device.libraries().join(library).join("master-key.cfmk"),
        stored_master_key(),
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
    assert!(device.library_named(name).is_none());
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

// SA-9, KD-9: only a failed authentication is told as a Passphrase that does
// not open the Library. A referencing Library whose stored Master Key file is
// missing, or cannot be read, is refused as that — even under the right
// Passphrase — so nobody retypes a right one at it.
#[test]
fn a_missing_or_unreadable_master_key_file_is_not_told_as_a_passphrase_that_does_not_open() {
    type Break = fn(&std::path::Path);
    let cases: [(&str, Break, &str); 2] = [
        (
            "a missing file",
            |file| fs::remove_file(file).expect("removable"),
            "is missing; the Passphrase was not checked",
        ),
        (
            "a file that cannot be read",
            // A directory where the file goes opens, and refuses to be read.
            |file| {
                fs::remove_file(file).expect("removable");
                fs::create_dir(file).expect("creatable");
            },
            "could not be read",
        ),
    ];
    for (fault, damage, told) in cases {
        let device = Device::new();
        lay_out(&device, "at-work", "work", CLIENT);
        damage(&device.libraries().join("at-work").join("master-key.cfmk"));

        let mut arguments = init_drive("second", CLIENT, Some("work"));
        arguments.insert(0, "--json");
        let output = device.run_with(&arguments, Some(PASSPHRASE));
        assert_eq!(
            code(&output),
            1,
            "{fault}: stderr was:\n{}",
            stderr(&output)
        );
        let answer: serde_json::Value =
            serde_json::from_str(stdout(&output).trim()).expect("--json answers one object");
        let message = answer["error"]["message"]
            .as_str()
            .expect("a failure says what it was");
        assert!(
            message.contains(told)
                && message.contains("master-key.cfmk")
                && !message.contains("does not open one")
                && !message.contains("is on this device"),
            "{fault}: {answer}"
        );
        assert!(device.library_named("second").is_none(), "{fault}");
    }
}
