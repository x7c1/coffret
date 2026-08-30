//! Opening an S3 Library against a real S3 implementation.
//!
//! What the unit tests cannot answer is whether the settings file says enough:
//! a device records the bucket, the prefix, the endpoint, the region, and how
//! the bucket is addressed, and deliberately records no credential at all. The
//! case for that is a store built from those five things reaching the bucket.

use coffret_device::{
    create_library, mappings, open_library, set_mapping, CreateLibraryRequest, NewProvider,
    ProviderSettings,
};
mod minio;

/// The Passphrase every case here uses.
const PASSPHRASE: &[u8] = b"correct horse battery staple";

// The whole journey a device makes before its first sync: create the Library,
// map a folder, open it again, and find the Storage it names.
#[tokio::test]
async fn a_library_opens_onto_the_prefix_its_settings_name() {
    let Some(target) = minio::target("open-library").await else {
        eprintln!("skipped: no S3 implementation is configured");
        return;
    };
    let folders = tempfile::tempdir().expect("a temporary directory must be available");

    let created = create_library(
        CreateLibraryRequest {
            name: "opened".to_owned(),
            passphrase: PASSPHRASE.to_vec(),
            provider: NewProvider::S3 {
                bucket: target.bucket.clone(),
                base_prefix: target.base_prefix.clone(),
                endpoint: Some(target.endpoint.clone()),
                region: Some(minio::REGION.to_owned()),
                path_style: true,
            },
        },
        |_| panic!("an S3 Library asks nobody for consent"),
    )
    .await
    .expect("an S3 Library needs nothing but this device");

    // FM-18: the Library's keys start at the base the user chose with the
    // Library's own name after it.
    let ProviderSettings::S3 { prefix, .. } = &created.settings.provider else {
        panic!("an S3 Library must be recorded as one");
    };
    assert_eq!(
        prefix,
        &format!(
            "{}coffret-{}/",
            target.base_prefix,
            created.settings.library_id.to_hex()
        )
    );

    set_mapping("opened", None, folders.path())
        .await
        .expect("the Library root must be mappable");

    let open = open_library("opened", PASSPHRASE)
        .await
        .expect("the Passphrase must open the Library");

    // A Library that has never been synced has nothing on Storage: the first
    // commit is what writes Keyring generation 1 and Journal record 1, so the
    // prefix answers and answers empty.
    let page = open
        .store
        .list(None)
        .await
        .expect("the store must reach the bucket the settings name");
    assert!(
        page.objects.is_empty(),
        "a Library that has never been synced holds nothing: {:?}",
        page.objects
    );

    // The catalog that comes back is the one the mapping was recorded in, not a
    // fresh file beside it.
    let listed = open
        .index
        .mappings()
        .await
        .expect("the catalog must be readable");
    assert_eq!(listed, mappings("opened").await.expect("the mappings read"));
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].prefix, None);
    assert_eq!(
        listed[0].local_root,
        folders.path().canonicalize().expect("the folder is there")
    );

    assert_eq!(open.library_id, created.settings.library_id);
    assert_eq!(open.epoch.get(), 1);
    assert!(open.spool.is_dir());
}
