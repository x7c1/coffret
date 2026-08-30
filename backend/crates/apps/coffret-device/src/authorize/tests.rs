use super::authorize;
use crate::error::Error;
use crate::testing::{create_s3, passphrase, state_dir};

// There is no grant to renew for a Library that is not on Drive, and saying so
// is better than running a flow whose result nothing would read.
#[tokio::test]
async fn authorizing_a_library_that_is_not_on_drive_is_refused() {
    create_s3("no-grant").await;

    let result = authorize("no-grant", passphrase, |_| {
        panic!("no consent is asked for a Library that is not on Drive")
    })
    .await;
    assert!(
        matches!(&result, Err(Error::NotADriveLibrary { name }) if name == "no-grant"),
        "expected an S3 Library to have no grant to renew, got {result:?}"
    );
}

// The other refusal this call makes before there is anything to renew, and the
// one a mistyped `--library` meets: the settings file answers whether the
// Library is here at all, so it is answered before the Passphrase is asked for.
// Asking first would have a person type one — or a script spend one — only to
// be told there is no such Library.
#[tokio::test]
async fn a_library_that_is_not_here_is_refused_before_a_passphrase_is_read() {
    state_dir();
    let unasked = || panic!("no Passphrase may be asked for before a refusal that needs none");

    let result = authorize("no-grant-to-renew-here", unasked, |_| {
        panic!("no consent is asked for a Library that is not here")
    })
    .await;
    assert!(
        matches!(&result, Err(Error::NoSuchLibrary { name, .. }) if name == "no-grant-to-renew-here"),
        "expected the name to be refused, got {result:?}"
    );
}
