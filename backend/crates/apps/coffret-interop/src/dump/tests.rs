use std::path::PathBuf;

use coffret_format::{
    encode, encode_control_object, encode_keyring, generate_container_id, generate_container_key,
    keyring_set_digest, wrap_container_key, ControlEncodeRequest, EncodeRequest, EntrySource,
};
use coffret_model::{
    ContainerId, ContainerKind, ControlObjectName, EntryPath, Generation, KeyTable, KeyringElement,
    MasterKey, MasterKeyEpoch, Mtime, ReplicaPosition,
};

use super::*;

/// A Library of one Pack, as `dump` is handed it: the Pack, a Keyring
/// replica saved under its own name, and the Recovery Code — each a file
/// in `dir`.
struct Fetched {
    container: PathBuf,
    container_name: String,
    container_id: ContainerId,
    keyring: PathBuf,
    recovery_code: PathBuf,
}

fn fetched(dir: &Path) -> Fetched {
    let master_key = [0x5a; MasterKey::BYTE_LEN];
    let code = RecoveryCode::encode(MasterKey::from_bytes(master_key), MasterKeyEpoch::FIRST);
    let recovery_code = dir.join("recovery-code.txt");
    std::fs::write(&recovery_code, code.as_str()).expect("the code is written");
    let master_key = MasterKey::from_bytes(master_key);

    let container_id = generate_container_id().expect("an ID is drawn");
    let container_key = generate_container_key().expect("a key is drawn");
    let entries = [
        EntrySource::new(
            EntryPath::parse("books/atlas/001.jpg").expect("a path"),
            Mtime::from_unix_seconds(1_700_000_000),
            b"the first page",
        ),
        EntrySource::new(
            EntryPath::parse("books/atlas/002.jpg").expect("a path"),
            Mtime::from_unix_seconds(1_700_000_001),
            b"the second page, a little longer",
        ),
    ];
    let encoded = encode(&EncodeRequest::new(
        container_id,
        ContainerKind::Pack,
        &container_key,
        &entries,
    ))
    .expect("the Pack encodes");
    let container_name = container_id.object_name();
    let container = dir.join(&container_name);
    std::fs::write(&container, encoded.bytes()).expect("the Pack is written");

    let envelope = wrap_container_key(
        &PurposeKey::derive(&master_key, Purpose::ContainerWrap),
        &container_id,
        &container_key,
    )
    .expect("the key wraps");
    let table = KeyTable::canonical(vec![KeyringElement::envelope(container_id, envelope)])
        .expect("one element is a key table");
    let name = ControlObjectName::keyring_replica(
        Generation::FIRST,
        &keyring_set_digest(&table).expect("the table digests"),
        ReplicaPosition::new(0, 1).expect("one of one is a position"),
    )
    .expect("a real digest names a replica");
    let replica = encode_control_object(&ControlEncodeRequest::new(
        &name,
        ControlObjectKind::Keyring,
        &PurposeKey::derive(
            &master_key,
            Purpose::of_control_object(ControlObjectKind::Keyring),
        ),
        &encode_keyring(&table, MasterKeyEpoch::FIRST).expect("the table encodes"),
    ))
    .expect("the replica seals");
    let keyring = dir.join(replica.object_name());
    std::fs::write(&keyring, replica.bytes()).expect("the replica is written");

    Fetched {
        container,
        container_name,
        container_id,
        keyring,
        recovery_code,
    }
}

// FM-9: handed a Keyring replica, a Container is opened with the key it
// wraps and its meta section rendered — the kind, the padding and the
// entry table — and none of its content.
#[test]
fn a_container_renders_its_meta_section_given_a_keyring() {
    let dir = tempfile::tempdir().expect("a directory");
    let fetched = fetched(dir.path());

    let rendered = dump(
        &fetched.container,
        &fetched.container_name,
        &fetched.recovery_code,
        Some(&fetched.keyring),
    )
    .expect("the Container renders");
    let rendered: serde_json::Value = serde_json::from_str(&rendered).expect("the dump is JSON");

    assert_eq!(rendered["container_id"], fetched.container_id.to_hex());
    assert_eq!(rendered["kind"], "Pack");
    assert!(rendered["pad_len"].is_u64(), "{rendered}");
    let entries = rendered["entries"].as_array().expect("an entry table");
    let paths: Vec<&str> = entries
        .iter()
        .map(|entry| entry["path"].as_str().expect("a path"))
        .collect();
    assert_eq!(paths, ["books/atlas/001.jpg", "books/atlas/002.jpg"]);
    assert_eq!(entries[0]["offset"], 0);
    assert_eq!(entries[0]["size"], 14);
    assert_eq!(entries[1]["offset"], 14);
    assert!(!rendered.to_string().contains("the first page"));
}

// Without a Keyring there is no key to open a Container with, and the dump
// says so rather than that no key opens the object.
#[test]
fn a_container_without_a_keyring_says_to_pass_one() {
    let dir = tempfile::tempdir().expect("a directory");
    let fetched = fetched(dir.path());

    let refused = dump(
        &fetched.container,
        &fetched.container_name,
        &fetched.recovery_code,
        None,
    )
    .expect_err("a Container needs a Keyring to render");

    let Error::ContainerNeedsKeyring { object_name } = refused else {
        panic!("a Container without a Keyring asks for one: {refused:?}");
    };
    assert_eq!(object_name, fetched.container_name);
}

// A control object still renders as before, with or without a Keyring
// beside it.
#[test]
fn a_keyring_replica_still_renders_as_a_control_object() {
    let dir = tempfile::tempdir().expect("a directory");
    let fetched = fetched(dir.path());
    let name = fetched
        .keyring
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a Unicode name")
        .to_owned();

    let rendered =
        dump(&fetched.keyring, &name, &fetched.recovery_code, None).expect("the replica renders");
    let rendered: serde_json::Value = serde_json::from_str(&rendered).expect("the dump is JSON");
    assert_eq!(rendered["kind"], "Keyring");
}

// A name the object is not stored under opens it under no kind's key — the
// name is part of what authenticates it — and its header names a Container
// stored under another name, so the dump says no key opens it.
#[test]
fn an_object_under_another_name_is_opened_by_no_key() {
    let dir = tempfile::tempdir().expect("a directory");
    let fetched = fetched(dir.path());

    let refused = dump(
        &fetched.keyring,
        "head-00000000000000000001",
        &fetched.recovery_code,
        None,
    )
    .expect_err("the name is not the replica's");

    let Error::NoKeyOpens { object_name } = refused else {
        panic!("no key opens the object: {refused:?}");
    };
    assert_eq!(object_name, "head-00000000000000000001");
}
