//! One grant per account on a device, opened by every Library that references
//! it (spec: SA-8, SA-9).
//!
//! Every case here has a device of its own ([`isolated::device`]): accounts
//! are the device's and not a Library's, so a case that counts them, or takes
//! away the Libraries that reference one, cannot share a state directory with
//! the rest of this binary.
//!
//! SA-8: A grant belongs to a device and an account. A device keeps one sealed
//! cache per account (KD-10), under that account's own account-cache key
//! (KD-12), and never a second: a Library that uses the account **references**
//! it by its device-local account name, and every Library on the device that
//! references one account reaches Storage through that one cache.
//!
//! - The **device-local account name** is the person's, because coffret cannot
//!   tell accounts apart by what it was granted: the one permission it asks for
//!   names no account (SA-3). Like a device-local Library name, it is chosen by
//!   the person, is never written to Storage, and never reaches a diagnostic
//!   event (EL-1). It is optional while the device holds one account and
//!   required when a second is added, so creating a Library on a device holding
//!   more than one account refuses to start without it. An account the person
//!   leaves unnamed is named `default`, the name the promotion below also
//!   gives, so every envelope has a name to be bound to (SA-9).
//! - One account name binds to one OAuth client id on the device: the
//!   account's grant is renewed, and consented to, only through the client the
//!   account first consented to, since a token minted through another client is
//!   a different grant with a different reach.
//! - Joining a Library tries each grant the device holds, and the joined
//!   Library references the account whose Storage has the app folder the person
//!   named; the authorization flow runs only when none does, and its grant
//!   becomes a new account on the device.
//! - Renewal is per account: it replaces the account's one cache, so every
//!   Library that references the account uses the renewed grant from its next
//!   run, and none of them is renewed on its own (SA-4, SA-6).
//! - A cache no Library on the device references any longer — the last
//!   envelope that opened it removed with its Library — can no longer be
//!   opened, and the device discards it the next time it opens the directory it
//!   keeps accounts in.
//! - A Library found holding its own per-Library cache, sealed under the
//!   `coffret/v1/token-cache` purpose key (KD-4, KD-10), and no account-cache
//!   key envelope, has that cache **promoted** the first time it is opened — the
//!   one path by which a Library leaves its previous shape. The refresh token is
//!   resealed under a fresh account-cache key into an account named `default`,
//!   the Library gains its envelope (SA-9) and references `default`, and only
//!   then is the per-Library cache removed, so an interrupted promotion leaves
//!   the previous shape to promote again. Where the device already holds an
//!   account named `default`, the Library references it if that account's grant
//!   reaches the Library's app folder — the choice joining makes — and its own
//!   cache is removed unused; otherwise the promotion stops, says so, and asks
//!   the person to name an account for this Library.
//!
//! SA-9: A Library that references an account holds, in its own directory on
//! the device, one **account-cache key envelope**: that account's account-cache
//! key sealed under the Library's `coffret/v1/account-cache-wrap` purpose key
//! (KD-3, KD-4), with the device-local account name bound as associated data,
//! in the form KD-12 lays down. It is one envelope per Library, so unlocking
//! any one Library that references an account opens that account's cache, and a
//! Library's directory still holds everything the device keeps for that Library
//! alone.
//!
//! - An envelope opened under a name other than the one it was sealed with
//!   fails to authenticate, so an envelope cannot be carried over to stand for
//!   another account; renaming an account on the device re-seals every
//!   envelope of that account.
//! - The purpose key is derived from the Master Key and changes when it rotates
//!   (KD-3), so a device that moves a Library to a new Master Key epoch
//!   re-seals that Library's envelope under the new epoch's purpose key in the
//!   same step.
//! - When the envelope of a Library that references an account is missing,
//!   malformed, or fails to authenticate, it is reported as an unreadable
//!   envelope, never as a Library that references no account, so a damaged
//!   envelope is not quietly answered with a second consent.
//!
//! Two of SA-9's clauses have no case yet, because no flow on a device does
//! what they are about: a device renames no account and rotates no Master Key.
//! The flow that does either is where its case goes. The binding they rest on —
//! an envelope opens under one Library's purpose key and one account name, and
//! no other — is sampled here and in the envelope's own cases (KD-12).

use std::fs;
use std::sync::Arc;

use coffret_format::{Purpose, PurposeKey, StoredMasterKey};
use coffret_logging::testing::CapturedLogs;
use coffret_model::Passphrase;
use google_drive_store::{StoredTokens, TokenCache};
use zeroize::Zeroizing;

use crate::account_dir::AccountDir;
use crate::account_envelope::AccountEnvelope;
use crate::account_name::AccountName;
use crate::accounts::Accounts;
use crate::authorize::{authorize_through, AuthorizeRequest};
use crate::create_library::{create_library_through, CreateLibraryRequest, NewProvider};
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::drive;
use crate::error::{Error, PromotionObstacle, Result};
use crate::join_library::{join_library_through, JoinLibraryRequest, JoinedProvider};
use crate::library_dir::{isolated, LibraryDir};
use crate::open_library::open_library_through;
use crate::reach::Reach;
use crate::stored_master_key_file::StoredMasterKeyFile;
#[cfg(unix)]
use crate::testing::mode_of;
use crate::testing::{consent, create_s3, every_link, DriveStub, CLIENT_ID};

/// The Passphrase the cases' Libraries are kept under.
const PASSPHRASE: &[u8] = b"one passphrase for this device";

/// A second one, for a Library kept under a Passphrase of its own.
const OTHER_PASSPHRASE: &[u8] = b"a passphrase of this Library's own";

/// The Passphrase `bytes`, as the flows ask for it.
fn pass(bytes: &'static [u8]) -> impl FnOnce() -> Result<Passphrase> + Send {
    move || Ok(Passphrase::from_bytes(bytes.to_vec()))
}

/// A Passphrase callback that fails the case if it is ever called.
fn unasked() -> Result<Passphrase> {
    panic!("no Passphrase may be asked for before a refusal that needs none")
}

/// A consent callback that fails the case if a consent is ever asked for.
fn no_consent(_: &str) {
    panic!("no consent may be asked for where the device already holds the grant")
}

/// What a case reaches Drive through.
fn reach(drive: &Arc<DriveStub>) -> Reach {
    Reach::this_device().reaching_drive_through(Arc::clone(drive) as _)
}

/// What creating a Drive Library called `name` on `account` asks for.
fn drive_request(name: &str, account: Option<&str>) -> CreateLibraryRequest {
    CreateLibraryRequest {
        name: name.to_owned(),
        provider: NewProvider::Drive {
            parent: "stub-parent".to_owned(),
            client_id: CLIENT_ID.to_owned(),
            client_secret: None,
            account: account.map(str::to_owned),
        },
        referencing_passphrase: crate::ReferencingPassphrase::unasked(),
    }
}

/// Creates a Drive Library called `name` on `account`, consenting where asked.
async fn create(drive: &Arc<DriveStub>, name: &str, account: Option<&str>) {
    create_library_through(
        &reach(drive),
        drive_request(name, account),
        pass(PASSPHRASE),
        consent,
    )
    .await
    .unwrap_or_else(|error| panic!("{name} must be created: {}", every_link(&error)));
}

/// The account the Library called `name` references, as its settings say.
fn account_of(name: &str) -> Option<String> {
    let dir = LibraryDir::resolve(name).expect("a case names a Library one component long");
    match DeviceSettings::read(&dir)
        .expect("the Library's settings read")
        .provider
    {
        ProviderSettings::Drive { account, .. } => account,
        ProviderSettings::S3 { .. } => None,
    }
}

/// The accounts on this case's device, by name.
fn accounts_on(device: &isolated::Device) -> Vec<String> {
    let Ok(listing) = fs::read_dir(device.path().join("accounts")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = listing
        .map(|entry| {
            entry
                .expect("an entry reads")
                .file_name()
                .into_string()
                .expect("an account's name is Unicode")
        })
        .collect();
    names.sort();
    names
}

/// The refresh token the account called `account` holds, opened through the
/// envelope of the Library called `library`.
fn refresh_token_of(library: &str, account: &str, passphrase: &[u8]) -> Option<String> {
    let dir = LibraryDir::resolve(library).expect("a case names a Library one component long");
    let unlocked = StoredMasterKeyFile::unlock(&dir, &Passphrase::from_bytes(passphrase.to_vec()))
        .expect("the Library opens with its Passphrase");
    let name = AccountName::parse(account).expect("a case names an account a name");
    let key = AccountEnvelope::open(&dir, &unlocked.master_key, &name)
        .expect("the Library's envelope opens for its account");
    let account = AccountDir::resolve(&name).expect("the account resolves");
    drive::token_cache(&account, Arc::new(key))
        .load()
        .expect("the account's cache opens")
        .map(|tokens| tokens.refresh_token)
}

// SA-8: two Libraries of one account on one device hold one cache, and the
// second costs no consent. SA-9: each holds an envelope of its own, and its
// directory holds no grant.
#[tokio::test]
async fn two_libraries_of_one_account_hold_one_cache() {
    let device = isolated::device();
    let drive = DriveStub::empty();

    create(&drive, "first", None).await;
    create_library_through(
        &reach(&drive),
        drive_request("second", None),
        pass(PASSPHRASE),
        no_consent,
    )
    .await
    .expect("a second Library of the account the device holds needs no consent");

    assert_eq!(drive.consents(), 1, "one consent for one account");
    assert_eq!(accounts_on(&device), ["default"]);
    assert!(device
        .path()
        .join("accounts/default/token-cache.cftc")
        .is_file());
    let mut envelopes = Vec::new();
    for name in ["first", "second"] {
        let dir = LibraryDir::resolve(name).expect("the name is one component");
        assert_eq!(account_of(name).as_deref(), Some("default"));
        assert!(
            !dir.previous_token_cache_file().exists(),
            "{name} keeps no grant of its own"
        );
        envelopes.push(fs::read(dir.account_envelope_file()).expect("each holds an envelope"));
    }
    assert_ne!(
        envelopes[0], envelopes[1],
        "one envelope per Library, each under its own Master Key's purpose key"
    );

    // Owner-only, as everything else a device keeps is.
    #[cfg(unix)]
    {
        let account = device.path().join("accounts/default");
        assert_eq!(mode_of(&account), 0o700);
        assert_eq!(mode_of(&account.join("token-cache.cftc")), 0o600);
        assert_eq!(mode_of(&account.join("settings.json")), 0o600);
        let first = LibraryDir::resolve("first").expect("resolves");
        assert_eq!(mode_of(&first.account_envelope_file()), 0o600);
    }
}

// SA-9: unlocking either Library opens the account's one cache, each through
// its own envelope, whatever Passphrase each Library is kept under.
#[tokio::test]
async fn either_library_s_passphrase_opens_the_one_cache() {
    let _device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "first", None).await;
    create(&drive, "second", None).await;
    keep_under(OTHER_PASSPHRASE, "second");

    for (name, passphrase) in [("first", PASSPHRASE), ("second", OTHER_PASSPHRASE)] {
        open_library_through(&reach(&drive), name, pass(passphrase))
            .await
            .unwrap_or_else(|error| panic!("{name} must open: {}", every_link(&error)));
        assert_eq!(
            refresh_token_of(name, "default", passphrase).as_deref(),
            Some("1//stub-refresh-1"),
            "{name} reaches the one grant the device holds"
        );
    }
}

// SA-8: removing one Library leaves the account's cache open to the other;
// removing both leaves nothing that can open it, and the next opening of the
// accounts discards it.
#[tokio::test]
async fn a_cache_outlives_each_library_but_not_all_of_them() {
    let device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "first", None).await;
    create(&drive, "second", None).await;

    fs::remove_dir_all(LibraryDir::resolve("first").expect("resolves").path())
        .expect("the Library's directory is removable");
    open_library_through(&reach(&drive), "second", pass(PASSPHRASE))
        .await
        .expect("the other Library still opens the account's cache");
    assert_eq!(accounts_on(&device), ["default"]);

    fs::remove_dir_all(LibraryDir::resolve("second").expect("resolves").path())
        .expect("the Library's directory is removable");
    assert!(
        device
            .path()
            .join("accounts/default/token-cache.cftc")
            .is_file(),
        "the cache is still there, and nothing on the device can open it"
    );
    Accounts::open().expect("the accounts open");
    assert_eq!(
        accounts_on(&device),
        Vec::<String>::new(),
        "a cache no Library references is discarded"
    );
}

// SA-8: a name is optional while the device holds one account and required
// once it holds two — refused before a Passphrase is asked for — and naming
// one of them takes it without a consent.
#[tokio::test]
async fn a_second_account_requires_a_name() {
    let device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "at-work", Some("work")).await;
    create(&drive, "at-home", Some("home")).await;
    assert_eq!(accounts_on(&device), ["home", "work"]);

    let result = create_library_through(
        &reach(&drive),
        drive_request("unnamed", None),
        unasked,
        no_consent,
    )
    .await;
    let Err(refused @ Error::AccountNameRequired { held: 2 }) = &result else {
        panic!("expected a name to be required, got {result:?}");
    };
    assert_eq!(
        refused.to_string(),
        "this device holds 2 accounts, so which one the Library uses has to be named: give \
         --account with the name of one of them, or a new name to consent as another"
    );

    create_library_through(
        &reach(&drive),
        drive_request("named", Some("work")),
        pass(PASSPHRASE),
        no_consent,
    )
    .await
    .expect("naming an account the device holds takes it");
    assert_eq!(account_of("named").as_deref(), Some("work"));
    assert_eq!(drive.consents(), 2);
}

// SA-8: joining tries each grant the device holds and takes the account whose
// Drive lists the named folder, with no consent.
#[tokio::test]
async fn join_takes_the_account_whose_drive_lists_the_folder() {
    let _device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "at-work", Some("work")).await;
    create(&drive, "at-home", Some("home")).await;
    let origin = create_s3("origin").await;
    // Only the second consent's grant — `home` — sees the folder.
    drive.add_folder_for(
        "joined-folder",
        &origin.settings.library_id.app_folder_name(),
        2,
    );

    let joined = join_library_through(
        &reach(&drive),
        join_request("joined", "joined-folder", None),
        || Ok(Zeroizing::new(origin.recovery_code.to_grouped_string())),
        pass(PASSPHRASE),
        no_consent,
    )
    .await
    .unwrap_or_else(|error| panic!("the join must find the account: {}", every_link(&error)));

    assert_eq!(joined.settings.library_id, origin.settings.library_id);
    assert_eq!(account_of("joined").as_deref(), Some("home"));
    assert_eq!(drive.consents(), 2, "the join asked for no consent");
}

// SA-8: where none of the accounts reaches the folder, the new account the
// consent would bring needs a name, and the join stops to ask for one; named,
// it is a new account, consented to for this Library.
#[tokio::test]
async fn a_join_no_account_reaches_asks_for_a_name() {
    let _device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "at-work", Some("work")).await;
    let origin = create_s3("origin").await;
    // Seen only by the grant the next consent brings, which `work`'s is not.
    drive.add_folder_for(
        "elsewhere",
        &origin.settings.library_id.app_folder_name(),
        2,
    );

    let result = join_library_through(
        &reach(&drive),
        join_request("joined", "elsewhere", None),
        || Ok(Zeroizing::new(origin.recovery_code.to_grouped_string())),
        pass(PASSPHRASE),
        no_consent,
    )
    .await;
    let Err(Error::LibraryNotJoined { cause, .. }) = &result else {
        panic!("expected the join to stop, got {result:?}");
    };
    assert!(
        matches!(**cause, Error::NoAccountReachesFolder),
        "{}",
        every_link(cause.as_ref())
    );
    assert_eq!(
        cause.to_string(),
        "none of the accounts this device holds reaches that folder, and a new account needs a \
         name while this device holds any: give --account with a name for the account the \
         folder is in"
    );

    join_library_through(
        &reach(&drive),
        join_request("joined", "elsewhere", Some("other")),
        || Ok(Zeroizing::new(origin.recovery_code.to_grouped_string())),
        pass(PASSPHRASE),
        consent,
    )
    .await
    .unwrap_or_else(|error| panic!("a named account is consented to: {}", every_link(&error)));
    assert_eq!(account_of("joined").as_deref(), Some("other"));
    assert_eq!(drive.consents(), 2);
}

// SA-9: an envelope opens under the Library's own key and the account's own
// name, and under no other — and a missing or damaged envelope is an
// unreadable one, never a Library with no account.
#[tokio::test]
async fn an_envelope_opens_for_its_own_library_and_account_only() {
    let _device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "first", None).await;
    create(&drive, "second", None).await;
    let first = LibraryDir::resolve("first").expect("resolves");
    let second = LibraryDir::resolve("second").expect("resolves");

    // Another Library's envelope, carried over: sealed under another key.
    fs::copy(
        first.account_envelope_file(),
        second.account_envelope_file(),
    )
    .expect("the envelope is copyable");
    let result = open_library_through(&reach(&drive), "second", pass(PASSPHRASE)).await;
    assert!(
        matches!(
            &result,
            Err(Error::UnreadableAccountEnvelope { cause: Some(_), .. })
        ),
        "another Library's envelope must not open here: {:?}",
        result.err()
    );

    // The same account under another name: the envelope is bound to the name.
    let mut settings = DeviceSettings::read(&first).expect("reads");
    if let ProviderSettings::Drive { account, .. } = &mut settings.provider {
        *account = Some("renamed".to_owned());
    }
    settings.write(&first).expect("writes");
    let result = open_library_through(&reach(&drive), "first", pass(PASSPHRASE)).await;
    let Err(refused @ Error::UnreadableAccountEnvelope { cause: Some(_), .. }) = &result else {
        panic!(
            "an envelope must not open for another name: {:?}",
            result.err()
        );
    };
    assert_eq!(
        refused.to_string(),
        "the envelope that opens the account \"renamed\" for the Library \"first\" could not be \
         read; nothing was renewed and no consent was asked for"
    );

    // And one that is not there at all.
    fs::remove_file(second.account_envelope_file()).expect("removable");
    let result = open_library_through(&reach(&drive), "second", pass(PASSPHRASE)).await;
    assert!(
        matches!(
            &result,
            Err(Error::UnreadableAccountEnvelope { cause: None, .. })
        ),
        "a missing envelope is an unreadable one: {:?}",
        result.err()
    );
}

// SA-8: one account name binds to one OAuth client — a Library naming another
// is refused, before a Passphrase is asked for where it is being put here.
#[tokio::test]
async fn a_library_of_another_client_is_refused_the_account() {
    let _device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "first", None).await;

    let mut request = drive_request("other-client", None);
    if let NewProvider::Drive { client_id, .. } = &mut request.provider {
        *client_id = "another-client.apps.googleusercontent.com".to_owned();
    }
    let result = create_library_through(&reach(&drive), request, unasked, no_consent).await;
    let Err(refused @ Error::ClientMismatch(_)) = &result else {
        panic!("expected the clients to be told apart, got {result:?}");
    };
    assert_eq!(
        refused.to_string(),
        format!(
            "the Library \"other-client\" names the OAuth client \
             \"another-client.apps.googleusercontent.com\" and the account \"default\" names \
             \"{CLIENT_ID}\"; an account's grant is used only through the client it was \
             consented to; give --account a new name to consent through the Library's client as \
             another account"
        )
    );

    // And a Library already here whose settings came to name another.
    let dir = LibraryDir::resolve("first").expect("resolves");
    let mut settings = DeviceSettings::read(&dir).expect("reads");
    if let ProviderSettings::Drive { client_id, .. } = &mut settings.provider {
        *client_id = "edited.apps.googleusercontent.com".to_owned();
    }
    settings.write(&dir).expect("writes");
    let result = open_library_through(&reach(&drive), "first", pass(PASSPHRASE)).await;
    assert!(
        matches!(&result, Err(Error::ClientMismatch(_))),
        "{:?}",
        result.err()
    );
}

// SA-8: renewal is per account — it replaces the one cache, and every Library
// that references the account uses the renewed grant.
#[tokio::test]
async fn renewing_an_account_renews_it_for_every_library() {
    let _device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "first", None).await;
    create(&drive, "second", None).await;

    authorize_through(
        &reach(&drive),
        AuthorizeRequest::Account {
            name: "default".to_owned(),
        },
        pass(PASSPHRASE),
        consent,
    )
    .await
    .unwrap_or_else(|error| panic!("the account must be renewed: {}", every_link(&error)));
    for name in ["first", "second"] {
        assert_eq!(
            refresh_token_of(name, "default", PASSPHRASE).as_deref(),
            Some("1//stub-refresh-2"),
            "{name} uses the renewed grant"
        );
    }

    // Named by a Library, the same renewal.
    authorize_through(
        &reach(&drive),
        AuthorizeRequest::Library {
            name: "second".to_owned(),
            account: None,
        },
        pass(PASSPHRASE),
        consent,
    )
    .await
    .expect("the account a Library references is renewed through it");
    assert_eq!(
        refresh_token_of("first", "default", PASSPHRASE).as_deref(),
        Some("1//stub-refresh-3")
    );
}

// SA-8: a Library found with its own previous cache and no envelope has it
// promoted the first time it is opened — into `default`, under a fresh key —
// and its own cache goes only once the account holds the grant.
#[tokio::test]
async fn a_library_s_previous_grant_is_promoted_when_it_is_opened() {
    let device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "old", None).await;
    into_previous_shape("old", "1//a-grant-kept-per-library");
    fs::remove_dir_all(device.path().join("accounts")).expect("the accounts are removable");

    open_library_through(&reach(&drive), "old", pass(PASSPHRASE))
        .await
        .unwrap_or_else(|error| panic!("the old Library must open: {}", every_link(&error)));

    let dir = LibraryDir::resolve("old").expect("resolves");
    assert_eq!(account_of("old").as_deref(), Some("default"));
    assert!(dir.account_envelope_file().is_file());
    assert!(!dir.previous_token_cache_file().exists());
    assert_eq!(
        refresh_token_of("old", "default", PASSPHRASE).as_deref(),
        Some("1//a-grant-kept-per-library"),
        "the grant the Library kept is the account's now"
    );
    assert_eq!(drive.consents(), 1, "a promotion asks for no consent");
}

// SA-8: where the device already holds `default`, a promoted Library
// references it if its grant reaches the Library's folder, and its own cache is
// removed unused; where it does not, the promotion stops and asks for a name,
// leaving the previous shape as it was.
#[tokio::test]
async fn a_promotion_into_a_held_default_is_the_choice_a_join_makes() {
    let _device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "current", None).await;
    create(&drive, "reaching", None).await;
    create(&drive, "elsewhere", None).await;
    into_previous_shape("reaching", "1//unused");
    into_previous_shape("elsewhere", "1//kept");
    let dir = LibraryDir::resolve("elsewhere").expect("resolves");
    let mut settings = DeviceSettings::read(&dir).expect("reads");
    if let ProviderSettings::Drive { folder_id, .. } = &mut settings.provider {
        *folder_id = "a-folder-default-does-not-see".to_owned();
    }
    settings.write(&dir).expect("writes");

    open_library_through(&reach(&drive), "reaching", pass(PASSPHRASE))
        .await
        .unwrap_or_else(|error| panic!("it must open: {}", every_link(&error)));
    assert_eq!(account_of("reaching").as_deref(), Some("default"));
    assert!(!LibraryDir::resolve("reaching")
        .expect("resolves")
        .previous_token_cache_file()
        .exists());
    assert_eq!(
        refresh_token_of("reaching", "default", PASSPHRASE).as_deref(),
        Some("1//stub-refresh-1"),
        "the account's grant is untouched, and the Library's own was not used"
    );

    let result = open_library_through(&reach(&drive), "elsewhere", pass(PASSPHRASE)).await;
    let Err(
        refused @ Error::PromotionNeedsName {
            obstacle: PromotionObstacle::FolderNotReached,
            ..
        },
    ) = &result
    else {
        panic!("expected the promotion to stop: {:?}", result.err());
    };
    assert_eq!(
        refused.to_string(),
        "the Library \"elsewhere\" keeps a grant of its own from an earlier build, and the account \
         \"default\" this device already holds cannot take it in; name an account for it with \
         `coffret authorize --library elsewhere --account NAME`"
    );
    assert!(dir.previous_token_cache_file().is_file());
    assert!(!dir.account_envelope_file().exists());
    assert_eq!(account_of("elsewhere"), None);

    // Named, the promotion goes to a new account, which the renewal then asks a
    // consent for.
    authorize_through(
        &reach(&drive),
        AuthorizeRequest::Library {
            name: "elsewhere".to_owned(),
            account: Some("another".to_owned()),
        },
        pass(PASSPHRASE),
        consent,
    )
    .await
    .unwrap_or_else(|error| panic!("the named promotion must go: {}", every_link(&error)));
    assert_eq!(account_of("elsewhere").as_deref(), Some("another"));
    assert!(!dir.previous_token_cache_file().exists());
}

// EL-1: the device-local account name never reaches a diagnostic event — not
// through putting a Library on an account, joining one, renewing a grant, or
// opening a Library on a device holding two accounts.
#[tokio::test]
async fn no_account_name_reaches_a_diagnostic_event() {
    const WORK: &str = "Alices-work-account";
    const HOME: &str = "Alices-home-account";
    let _device = isolated::device();
    let drive = DriveStub::empty();
    let logs = CapturedLogs::capture();

    create(&drive, "at-work", Some(WORK)).await;
    create(&drive, "at-home", Some(HOME)).await;
    let origin = create_s3("origin").await;
    drive.add_folder_for(
        "joined-folder",
        &origin.settings.library_id.app_folder_name(),
        2,
    );
    join_library_through(
        &reach(&drive),
        join_request("joined", "joined-folder", None),
        || Ok(Zeroizing::new(origin.recovery_code.to_grouped_string())),
        pass(PASSPHRASE),
        no_consent,
    )
    .await
    .expect("the join finds the account");
    authorize_through(
        &reach(&drive),
        AuthorizeRequest::Account {
            name: WORK.to_owned(),
        },
        pass(PASSPHRASE),
        consent,
    )
    .await
    .expect("the account is renewed");
    authorize_through(
        &reach(&drive),
        AuthorizeRequest::Library {
            name: "at-home".to_owned(),
            account: None,
        },
        pass(PASSPHRASE),
        consent,
    )
    .await
    .expect("the account is renewed through a Library");
    for name in ["at-work", "at-home", "joined"] {
        open_library_through(&reach(&drive), name, pass(PASSPHRASE))
            .await
            .unwrap_or_else(|error| panic!("{name} must open: {}", every_link(&error)));
    }
    // And the refusals that are about an account: a name required, and a
    // Passphrase that opens none of an account's Libraries.
    let _ = create_library_through(
        &reach(&drive),
        drive_request("x", None),
        unasked,
        no_consent,
    )
    .await;
    let _ = authorize_through(
        &reach(&drive),
        AuthorizeRequest::Account {
            name: HOME.to_owned(),
        },
        pass(OTHER_PASSPHRASE),
        no_consent,
    )
    .await;

    assert!(!logs.events().is_empty(), "the flows recorded something");
    logs.assert_free_of(&[WORK, HOME]);
}

/// What joining the Drive folder `folder_id` under `name` asks for.
fn join_request(name: &str, folder_id: &str, account: Option<&str>) -> JoinLibraryRequest {
    JoinLibraryRequest {
        name: name.to_owned(),
        provider: JoinedProvider::Drive {
            folder_id: folder_id.to_owned(),
            client_id: CLIENT_ID.to_owned(),
            client_secret: None,
            account: account.map(str::to_owned),
        },
        referencing_passphrase: crate::ReferencingPassphrase::unasked(),
    }
}

/// Keeps the Library called `name` under `passphrase` rather than the one it
/// was created under, the way a Passphrase change leaves it (spec: DK-6).
fn keep_under(passphrase: &'static [u8], name: &str) {
    let dir = LibraryDir::resolve(name).expect("resolves");
    let unlocked = StoredMasterKeyFile::unlock(&dir, &Passphrase::from_bytes(PASSPHRASE.to_vec()))
        .expect("opens under the Passphrase it was created under");
    let stored = StoredMasterKey::create(
        &Passphrase::from_bytes(passphrase.to_vec()),
        &unlocked.master_key,
        unlocked.epoch,
    )
    .expect("the stored form is made");
    StoredMasterKeyFile::write(&dir, &stored).expect("the stored form is written");
}

/// Lays the Library called `name` out as a build before accounts left it: a
/// grant of its own holding `refresh_token`, sealed under its token-cache
/// purpose key (spec: KD-10), no envelope, and settings that name no account.
fn into_previous_shape(name: &str, refresh_token: &str) {
    let dir = LibraryDir::resolve(name).expect("resolves");
    let unlocked = StoredMasterKeyFile::unlock(&dir, &Passphrase::from_bytes(PASSPHRASE.to_vec()))
        .expect("opens");
    TokenCache::new(
        dir.previous_token_cache_file(),
        Arc::new(PurposeKey::derive(
            &unlocked.master_key,
            Purpose::TokenCache,
        )),
    )
    .store(&StoredTokens {
        refresh_token: refresh_token.to_owned(),
    })
    .expect("the previous cache is written");
    fs::remove_file(dir.account_envelope_file()).expect("the envelope is removable");
    let mut settings = DeviceSettings::read(&dir).expect("reads");
    if let ProviderSettings::Drive { account, .. } = &mut settings.provider {
        *account = None;
    }
    settings.write(&dir).expect("writes");
}

/// A caller that answers the question about another Library's Passphrase with
/// `passphrase`, recording which Libraries it was asked about.
fn answering(
    passphrase: &'static [u8],
) -> (
    crate::ReferencingPassphrase,
    Arc<std::sync::Mutex<Vec<String>>>,
) {
    let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
    let record = Arc::clone(&asked);
    let referencing = crate::ReferencingPassphrase::asking(move |library| {
        record
            .lock()
            .expect("no case panics while holding this")
            .push(library.to_owned());
        Ok(Passphrase::from_bytes(passphrase.to_vec()))
    });
    (referencing, asked)
}

// SA-9, SA-8: a Library kept under a Passphrase of its own still goes onto the
// account the device holds, through the Passphrase of a Library already
// referencing it — asked for once, naming that Library — and never through a
// second consent.
#[tokio::test]
async fn a_library_under_another_passphrase_reaches_the_account_through_one_that_references_it() {
    let device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "first", Some("work")).await;

    let (referencing, asked) = answering(PASSPHRASE);
    let mut request = drive_request("second", Some("work"));
    request.referencing_passphrase = referencing;
    create_library_through(&reach(&drive), request, pass(OTHER_PASSPHRASE), no_consent)
        .await
        .unwrap_or_else(|error| {
            panic!("the fallback must open the account: {}", every_link(&error))
        });

    assert_eq!(*asked.lock().expect("held by nobody"), ["first"]);
    assert_eq!(accounts_on(&device), ["work"]);
    assert_eq!(drive.consents(), 1);
    assert_eq!(
        refresh_token_of("second", "work", OTHER_PASSPHRASE).as_deref(),
        Some("1//stub-refresh-1"),
        "the second Library opens the one grant through its own envelope"
    );
}

// SA-9: with nobody to ask — a script reading one Passphrase from standard
// input — the refusal names the Library whose Passphrase would open the
// account; and a Passphrase that does not open that Library is refused the
// same way.
#[tokio::test]
async fn an_account_nobody_can_open_is_refused_naming_the_library_that_would() {
    let device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "first", Some("work")).await;

    let (wrong, _) = answering(b"not the one");
    for referencing in [crate::ReferencingPassphrase::unasked(), wrong] {
        let mut request = drive_request("second", Some("work"));
        request.referencing_passphrase = referencing;
        let result =
            create_library_through(&reach(&drive), request, pass(OTHER_PASSPHRASE), no_consent)
                .await;
        let Err(Error::LibraryNotCreated { cause, .. }) = &result else {
            panic!("expected the creation to stop, got {result:?}");
        };
        assert_eq!(
            cause.to_string(),
            "the account \"work\" opens only through a Library that references it, and the \
             Passphrase given does not open one; the Passphrase of the Library \"first\" does"
        );
    }
    assert!(!LibraryDir::resolve("second")
        .expect("resolves")
        .path()
        .exists());
    assert_eq!(accounts_on(&device), ["work"]);
    assert_eq!(drive.consents(), 1);
}

// SA-9: a Library the Passphrase does open, whose envelope does not, is refused
// as that — never as a Passphrase that opens nothing, which would send the
// person after another Library's — whether the Passphrase was the new
// Library's own or the one asked for.
#[tokio::test]
async fn a_damaged_envelope_behind_the_right_passphrase_is_refused_as_itself() {
    let _device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "first", Some("work")).await;
    let first = LibraryDir::resolve("first").expect("resolves");
    fs::write(first.account_envelope_file(), b"not an envelope").expect("writable");

    let (answered, _) = answering(PASSPHRASE);
    for (own, referencing) in [
        (PASSPHRASE, crate::ReferencingPassphrase::unasked()),
        (OTHER_PASSPHRASE, answered),
    ] {
        let mut request = drive_request("second", Some("work"));
        request.referencing_passphrase = referencing;
        let result = create_library_through(&reach(&drive), request, pass(own), no_consent).await;
        let Err(Error::LibraryNotCreated { cause, .. }) = &result else {
            panic!("expected the creation to stop, got {result:?}");
        };
        assert!(
            matches!(
                **cause,
                Error::UnreadableAccountEnvelope { cause: Some(_), .. }
            ),
            "{}",
            every_link(&**cause)
        );
    }
    assert_eq!(drive.consents(), 1);
}

// SA-8: a join tries the accounts its own Passphrase opens first, and asks for
// another Library's Passphrase only for the ones it does not — here, once, for
// the account whose Drive lists the folder.
#[tokio::test]
async fn a_join_asks_only_for_the_accounts_its_own_passphrase_does_not_open() {
    let _device = isolated::device();
    let drive = DriveStub::empty();
    create(&drive, "at-home", Some("home")).await;
    create_library_through(
        &reach(&drive),
        drive_request("at-work", Some("work")),
        pass(OTHER_PASSPHRASE),
        consent,
    )
    .await
    .expect("a second account is created under its own Passphrase");
    let origin = create_s3("origin").await;
    // Only `work`'s grant — the second consent — sees the folder.
    drive.add_folder_for(
        "joined-folder",
        &origin.settings.library_id.app_folder_name(),
        2,
    );

    let (referencing, asked) = answering(OTHER_PASSPHRASE);
    let mut request = join_request("joined", "joined-folder", None);
    request.referencing_passphrase = referencing;
    join_library_through(
        &reach(&drive),
        request,
        || Ok(Zeroizing::new(origin.recovery_code.to_grouped_string())),
        pass(PASSPHRASE),
        no_consent,
    )
    .await
    .unwrap_or_else(|error| panic!("the join must find the account: {}", every_link(&error)));

    assert_eq!(
        *asked.lock().expect("held by nobody"),
        ["at-work"],
        "`home` opened with the join's own Passphrase and was asked about first; only `work` \
         needed a question"
    );
    assert_eq!(account_of("joined").as_deref(), Some("work"));
    assert_eq!(drive.consents(), 2);
}
