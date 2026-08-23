//! What Drive does with a pre-minted id once the file it named has been purged.
//!
//! A commit slot on Drive is an id from `files.generateIds`, and a Journal
//! record's successor is created under it. After a later Master Key epoch is
//! activated, the rotation purges the old epoch's control objects (MR-3) — so
//! the id a consumed slot named can end up naming nothing. Whether Drive then
//! lets a late writer create under that id again, or refuses it as already
//! used, decides whether the head re-read before a spend (CP-16) is a second
//! guard or the only one this store has. The published documentation does not
//! say, so this case asks Drive.
//!
//! Observed 2026-08-23 against a real account: the id is burned. Drive accepts
//! the resumable session for it and then refuses the upload's final request
//! with `400` and reason `invalid` on the `fileId` parameter — not a `409`, and
//! only after the body has been sent. So Drive keeps a consumed slot consumed
//! by itself, and the pre-spend re-read is what saves a late writer from
//! streaming a whole object before learning so. The case pins that answer: a
//! second create under a purged id must be refused, and Drive starting to
//! accept one would change what CP-16 is for.
//!
//! It takes the same environment as the conformance target (see its module
//! doc) and is skipped without it.

mod support;

use coffret_usecase::{ByteStream, Error, ObjectStore};

/// The name the slot is reserved under; opaque to Drive, a head name here
/// because that is what commit slots hold.
const SUCCESSOR: &str = "head-1.cfrt";

#[tokio::test]
async fn a_purged_pre_minted_id_refuses_a_second_create() {
    let Some(drive) = support::drive(|settings| settings).await else {
        eprintln!("skipped: {} is not set", support::FOLDER_ID);
        return;
    };

    let slot = drive
        .reserve_create(SUCCESSOR)
        .await
        .expect("Drive must mint an id for the slot");
    let first = drive
        .put_if_absent(
            &slot,
            ByteStream::from(b"the first tenant of the id".to_vec()),
        )
        .await
        .expect("a fresh slot must accept its object");
    drive
        .purge(&first)
        .await
        .expect("purging the object must succeed and read back as gone");

    let error = match drive
        .put_if_absent(
            &slot,
            ByteStream::from(b"a late writer reusing the id".to_vec()),
        )
        .await
    {
        Ok(second) => {
            drive
                .purge(&second)
                .await
                .expect("cleaning up the second object must succeed");
            panic!(
                "Drive accepted a second create under a purged pre-minted id; \
                 the pre-spend head re-read (CP-16) would now be the only guard"
            );
        }
        Err(error) => error,
    };

    // Either refusal keeps the slot consumed; which one Drive picks is what
    // this case records. It is `400 invalid` today.
    match &error {
        Error::Rejected { status, .. } => {
            eprintln!("DRIVE FINDING: a purged pre-minted id is burned — refused with {status}");
            assert_eq!(
                *status, 400,
                "Drive has changed how it refuses a burned id: {error:?}"
            );
        }
        Error::AlreadyExists { .. } => {
            eprintln!(
                "DRIVE FINDING: a purged pre-minted id is burned — refused as already existing"
            );
        }
        other => panic!("Drive answered the second create with something else: {other:?}"),
    }
    assert!(
        !error.is_retryable(),
        "a burned id must not be retried: {error:?}"
    );
}
