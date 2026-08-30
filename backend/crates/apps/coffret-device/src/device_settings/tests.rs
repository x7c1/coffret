use std::path::Path;

use super::{DeviceSettings, ProviderSettings};
use crate::error::Error;

/// A Library whose name carries every kind of hex digit.
fn library_id() -> coffret_model::LibraryId {
    coffret_model::LibraryId::from_bytes([0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef])
}

/// The file a refusal names, for a case that never touches a real one.
fn somewhere() -> &'static Path {
    Path::new("/state/coffret/libraries/alpha/settings.json")
}

/// The bytes the settings file holds for `settings`.
fn written(settings: &DeviceSettings) -> Vec<u8> {
    serde_json::to_vec_pretty(settings).expect("settings must encode")
}

// The file is the contract the explorer will read, so what this build writes is
// what it reads: both providers go out and come back as the same value.
#[test]
fn both_providers_round_trip_through_the_file() {
    for provider in [
        ProviderSettings::Drive {
            folder_id: "1a2B3c".to_owned(),
            client_id: "client.apps.googleusercontent.com".to_owned(),
            client_secret: Some("not-a-secret".to_owned()),
        },
        ProviderSettings::Drive {
            folder_id: "1a2B3c".to_owned(),
            client_id: "client.apps.googleusercontent.com".to_owned(),
            client_secret: None,
        },
        ProviderSettings::S3 {
            bucket: "photos".to_owned(),
            prefix: "archive/coffret-0123456789abcdef/".to_owned(),
            endpoint: Some("http://127.0.0.1:19000".to_owned()),
            region: Some("us-east-1".to_owned()),
            path_style: true,
        },
        ProviderSettings::S3 {
            bucket: "photos".to_owned(),
            prefix: "coffret-0123456789abcdef/".to_owned(),
            endpoint: None,
            region: None,
            path_style: false,
        },
    ] {
        let settings = DeviceSettings::new(library_id(), provider);
        let read = DeviceSettings::from_json(somewhere(), &written(&settings))
            .expect("what this build wrote is what it reads");

        assert_eq!(read, settings);
    }
}

// The Library ID is written the one way every identifier in coffret is written,
// so the app folder's name and the settings file agree letter for letter.
#[test]
fn the_library_id_is_written_as_lowercase_hex() {
    let settings = DeviceSettings::new(
        library_id(),
        ProviderSettings::S3 {
            bucket: "photos".to_owned(),
            prefix: "coffret-0123456789abcdef/".to_owned(),
            endpoint: None,
            region: None,
            path_style: false,
        },
    );
    let document: serde_json::Value =
        serde_json::from_slice(&written(&settings)).expect("the file must be JSON");

    assert_eq!(document["version"], 1);
    assert_eq!(document["library_id"], "0123456789abcdef");
    assert_eq!(document["provider"]["kind"], "s3");
    // Nothing said is nothing written: an S3 Library on AWS carries no endpoint
    // and no region, rather than two nulls a reader has to interpret.
    assert!(document["provider"].get("endpoint").is_none());
    assert!(document["provider"].get("region").is_none());
}

// A file from a later build is refused by version rather than read as far as
// this build happens to understand it.
#[test]
fn a_version_this_build_does_not_write_is_refused() {
    let document = br#"{
      "version": 2,
      "library_id": "0123456789abcdef",
      "provider": { "kind": "s3", "bucket": "photos", "prefix": "coffret-0123456789abcdef/", "path_style": false }
    }"#;

    let result = DeviceSettings::from_json(somewhere(), document);
    assert!(
        matches!(
            &result,
            Err(Error::UnsupportedSettingsVersion { path, version: 2, expected: 1 })
                if path == somewhere()
        ),
        "expected version 2 to be refused naming the file, got {result:?}"
    );
}

// A provider this build cannot reach is refused too: a device that guessed
// would report a Library it cannot see as an empty one.
#[test]
fn a_provider_this_build_does_not_know_is_refused() {
    let document = br#"{
      "version": 1,
      "library_id": "0123456789abcdef",
      "provider": { "kind": "ftp", "host": "example.invalid" }
    }"#;

    let result = DeviceSettings::from_json(somewhere(), document);
    assert!(
        matches!(&result, Err(Error::MalformedSettings { path, .. }) if path == somewhere()),
        "expected an unknown provider to be refused naming the file, got {result:?}"
    );
}

// A Library ID that is not the one spelling every identifier is written in is
// not a Library ID, and a file carrying one names no Library.
#[test]
fn a_library_id_that_is_not_lowercase_hex_is_refused() {
    let document = br#"{
      "version": 1,
      "library_id": "0123456789ABCDEF",
      "provider": { "kind": "s3", "bucket": "photos", "prefix": "coffret-0123456789abcdef/", "path_style": false }
    }"#;

    let result = DeviceSettings::from_json(somewhere(), document);
    assert!(
        matches!(&result, Err(Error::MalformedSettings { path, .. }) if path == somewhere()),
        "expected an uppercase Library ID to be refused, got {result:?}"
    );
}
