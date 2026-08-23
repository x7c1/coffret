//! What Drive does with a pre-minted id once the file it named has been purged.
//!
//! A commit slot on Drive is an id from `files.generateIds`, and a Journal
//! record's successor is created under it. After a later Master Key epoch is
//! activated, the rotation purges the old epoch's control objects (MR-3) — so
//! the id a consumed slot named can end up naming nothing. Whether Drive then
//! lets a late writer create under that id again, or refuses it as already
//! used, decides whether the head re-read before a spend (CP-16) is a second
//! guard or the only one this store has. The published documentation does not
//! say, so this case asks Drive and reports the answer.
//!
//! It takes the same environment as the conformance target (see its module
//! doc) and is skipped without it. Both outcomes pass: the point is the
//! observation, printed as the case runs and recorded in the run's log.

mod support;

use coffret_usecase::{ByteStream, Error, ObjectStore};

/// The name the slot is reserved under; opaque to Drive, a head name here
/// because that is what commit slots hold.
const SUCCESSOR: &str = "head-1.cfrt";

#[tokio::test]
async fn a_purged_pre_minted_id_reports_how_a_second_create_is_answered() {
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

    match drive
        .put_if_absent(
            &slot,
            ByteStream::from(b"a late writer reusing the id".to_vec()),
        )
        .await
    {
        Ok(second) => {
            let finding = "a purged pre-minted id ACCEPTS a second create: the head re-read \
                           before a spend (CP-16) is the only guard Drive leaves";
            eprintln!("DRIVE FINDING: {finding}");
            tracing::warn!(slot = SUCCESSOR, finding, "drive.pre_minted_id_reuse");
            drive
                .purge(&second)
                .await
                .expect("cleaning up the second object must succeed");
        }
        Err(Error::AlreadyExists { .. }) => {
            let finding = "a purged pre-minted id is BURNED: a second create is refused as \
                           already existing, so Drive itself keeps a consumed slot consumed";
            eprintln!("DRIVE FINDING: {finding}");
            tracing::warn!(slot = SUCCESSOR, finding, "drive.pre_minted_id_reuse");
        }
        Err(other) => panic!("Drive answered the second create with something else: {other:?}"),
    }
}
