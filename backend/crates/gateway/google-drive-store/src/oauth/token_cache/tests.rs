use std::fs;
use std::sync::Arc;

use coffret_format::{Purpose, PurposeKey};
use coffret_model::MasterKey;

use super::TokenCache;
use crate::error::{Error, TokenCacheDefect};
use crate::oauth::stored_tokens::StoredTokens;

/// A refresh token shaped like the ones Google issues, so a search for it in
/// the written file would find it if anything were written in the clear.
const REFRESH_TOKEN: &str = "1//0gSecretRefreshToken";

/// The key a cache is sealed under here, derived as a device derives it.
fn cache_key() -> Arc<PurposeKey> {
    derived([0x3d; MasterKey::BYTE_LEN])
}

/// The same for another device, whose Master Key is a different one.
fn another_cache_key() -> Arc<PurposeKey> {
    derived([0x3e; MasterKey::BYTE_LEN])
}

fn derived(bytes: [u8; MasterKey::BYTE_LEN]) -> Arc<PurposeKey> {
    Arc::new(PurposeKey::derive(
        &MasterKey::from_bytes(bytes),
        Purpose::TokenCache,
    ))
}

fn tokens() -> StoredTokens {
    StoredTokens {
        refresh_token: REFRESH_TOKEN.to_owned(),
    }
}

/// A cache holding [`tokens`], and the directory it lives in.
fn stored() -> (tempfile::TempDir, TokenCache) {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let cache = TokenCache::new(directory.path().join("tokens.bin"), cache_key());
    cache.store(&tokens()).expect("storing must succeed");
    (directory, cache)
}

#[test]
fn an_empty_cache_reads_as_nothing_cached() {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let cache = TokenCache::new(directory.path().join("tokens.bin"), cache_key());

    assert_eq!(cache.load().expect("a missing file is not an error"), None);
}

#[test]
fn what_is_stored_is_what_is_loaded() {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let cache = TokenCache::new(directory.path().join("nested/tokens.bin"), cache_key());

    cache.store(&tokens()).expect("storing must succeed");
    assert_eq!(cache.load().expect("loading must succeed"), Some(tokens()));
}

// The write goes to a neighbour and is renamed over the cache, and the
// neighbour is gone afterwards: a directory littered with half-written grants
// would be a second place to look for the credential.
#[test]
fn nothing_is_left_beside_the_written_cache() {
    let (directory, cache) = stored();
    cache.store(&tokens()).expect("a rewrite must succeed");

    let names: Vec<_> = fs::read_dir(directory.path())
        .expect("the directory must be readable")
        .map(|entry| entry.expect("an entry must be readable").file_name())
        .collect();
    assert_eq!(
        names,
        [cache.path().file_name().expect("the cache is a file")]
    );
}

// The credential is on disk as ciphertext: none of it appears in the file.
#[test]
fn the_written_file_carries_none_of_the_tokens() {
    let (_directory, cache) = stored();
    let written = fs::read(cache.path()).expect("the file must be readable");

    for secret in [REFRESH_TOKEN, "refresh_token"] {
        assert!(
            !written
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()),
            "{secret:?} must not appear in the written cache"
        );
    }
}

// A cache is bound to the Master Key that wrote it; another device's key
// yields an error rather than tokens.
#[test]
fn a_cache_written_under_another_master_key_is_refused() {
    let (_directory, cache) = stored();
    let other = TokenCache::new(cache.path(), another_cache_key());

    assert!(matches!(
        other.load(),
        Err(Error::MalformedTokenCache {
            cause: TokenCacheDefect::Sealed(_),
            ..
        })
    ));
}

// A file that opens is still not a cache if what is inside is not the token
// document.
#[test]
fn a_sealed_file_holding_something_else_is_refused_too() {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let path = directory.path().join("tokens.bin");
    let sealed = coffret_format::encode_token_cache(b"not a token document", &cache_key())
        .expect("sealing must succeed");
    fs::write(&path, sealed).expect("the file must be writable");

    let cache = TokenCache::new(&path, cache_key());
    assert!(matches!(
        cache.load(),
        Err(Error::MalformedTokenCache {
            cause: TokenCacheDefect::Document(_),
            ..
        })
    ));
}

// A cache the operating system will not hand over is not a cache that was
// never written, and what it reported travels with the failure.
#[test]
fn a_cache_the_operating_system_refuses_is_reported_as_such() {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let occupied = directory.path().join("occupied");
    fs::write(&occupied, b"not a directory").expect("the file must be writable");

    // A regular file cannot be a parent directory, so the read fails as
    // something other than "nothing has been cached yet".
    let cache = TokenCache::new(occupied.join("tokens.bin"), cache_key());

    let error = cache.load().expect_err("an unreadable path must fail");
    let Error::TokenCache { cause, .. } = &error else {
        panic!("the operating system's refusal must be reported as such: {error}");
    };
    assert_ne!(cause.kind(), std::io::ErrorKind::NotFound);
}

// No byte of the file can be edited without the cache refusing to load: the
// header ahead of the ciphertext is checked and then authenticated as
// associated data, the rest by the tag.
#[test]
fn a_cache_with_any_byte_flipped_is_refused() {
    let (_directory, cache) = stored();
    let written = fs::read(cache.path()).expect("the file must be readable");

    // One byte from each part of the sealed form: the magic at the front,
    // the nonce, the ciphertext, and the trailing tag.
    let regions = [
        ("the magic", 0),
        ("the nonce", 8),
        ("the ciphertext", 32),
        ("the tag", written.len() - 1),
    ];
    for (region, index) in regions {
        let mut tampered = written.clone();
        tampered[index] ^= 0x01;
        fs::write(cache.path(), &tampered).expect("the file must be writable");

        assert!(
            matches!(cache.load(), Err(Error::MalformedTokenCache { .. })),
            "a cache with {region} edited must not load"
        );
    }
}

// Anything that is not this form — another tool's file, a future version's,
// or a plaintext cache left by a build that wrote one — is refused rather
// than read as tokens.
#[test]
fn a_file_that_is_not_a_sealed_cache_is_refused() {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let path = directory.path().join("tokens.bin");
    let cache = TokenCache::new(&path, cache_key());

    let files: [&[u8]; 3] = [
        br#"{"refresh_token":"1//0gSecretRefreshToken"}"#,
        b"CFMK1\x01\x00 not a token cache, but a coffret file",
        b"CFTC1\x02\x00 a version this build does not know about......",
    ];
    for file in files {
        fs::write(&path, file).expect("the file must be writable");
        assert!(
            matches!(cache.load(), Err(Error::MalformedTokenCache { .. })),
            "{:?} must not load as a cache",
            String::from_utf8_lossy(file)
        );
    }
}

// What the caller is told to do about an unreadable cache is re-authorize,
// not that the disk broke.
#[test]
fn an_unreadable_cache_reaches_the_port_as_unauthenticated() {
    let (_directory, cache) = stored();
    fs::write(cache.path(), b"not a cache at all").expect("the file must be writable");

    let error = cache.load().expect_err("an unreadable cache must fail");
    assert!(matches!(
        coffret_usecase::Error::from(error),
        coffret_usecase::Error::Unauthenticated { .. }
    ));
}

#[cfg(unix)]
#[test]
fn the_cache_is_readable_by_its_owner_and_nobody_else() {
    use std::os::unix::fs::PermissionsExt;

    let (_directory, cache) = stored();
    let mode = fs::metadata(cache.path())
        .expect("the file must exist")
        .permissions()
        .mode();

    assert_eq!(mode & 0o777, super::OWNER_ONLY);
}

#[cfg(unix)]
#[test]
fn a_loosely_permissioned_cache_is_tightened_on_the_next_write() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let path = directory.path().join("tokens.bin");
    fs::write(&path, b"{}").expect("the file must be writable");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
        .expect("permissions must be settable");

    TokenCache::new(&path, cache_key())
        .store(&tokens())
        .expect("storing must succeed");

    let mode = fs::metadata(&path)
        .expect("the file must exist")
        .permissions()
        .mode();

    assert_eq!(mode & 0o777, super::OWNER_ONLY);
}
