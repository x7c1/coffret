use super::authorize;
use crate::error::Error;
use crate::testing::{create_s3, PASSPHRASE};

// There is no grant to renew for a Library that is not on Drive, and saying so
// is better than running a flow whose result nothing would read.
#[tokio::test]
async fn authorizing_a_library_that_is_not_on_drive_is_refused() {
    create_s3("no-grant").await;

    let result = authorize("no-grant", PASSPHRASE, |_| {
        panic!("no consent is asked for a Library that is not on Drive")
    })
    .await;
    assert!(
        matches!(&result, Err(Error::NotADriveLibrary { name }) if name == "no-grant"),
        "expected an S3 Library to have no grant to renew, got {result:?}"
    );
}
