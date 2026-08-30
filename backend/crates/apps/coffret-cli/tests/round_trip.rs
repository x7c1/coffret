//! A folder into the Library and out again, from the command line.
//!
//! One machine, one bucket, and three Libraries under three names, which is what
//! a second device is from Storage's point of view: a device that entered a
//! Recovery Code and mapped a folder of its own. Everything between is what a
//! person types — `init`, `map`, `sync`, `freeze`, `join`, `fetch` — and what
//! the round trip proves is that the bytes that come back out are the bytes that
//! went in.
//!
//! It needs an S3 implementation that really evaluates a conditional create,
//! keeps continuation tokens, and reports ETags, which is what `make
//! s3-store-it` starts. Without that environment the cases report themselves
//! skipped, so an ordinary `cargo test` neither needs Docker nor pretends to
//! have covered any of this.
//!
//! The bucket a Library is asked to live in is here for the same reason: what a
//! real implementation answers about a bucket that is not there is the one thing
//! a socket saying `200` cannot stand in for.

mod support;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Output;

use support::{
    code, printed_code, printed_prefix, stderr, stdout, succeeded, write_file, Device, Minio,
    FINDINGS, PASSPHRASE, RECOVERY_CODE_PREFIX, REGION,
};

/// What the Library calls the folder every case here maps.
const PREFIX: &str = "albums";

/// How large a Pack comes out in this case, in bytes before padding.
///
/// Small, so that a handful of files a few hundred bytes each is a freeze worth
/// looking at rather than one Entry short of the default gibibyte.
const TARGET: &str = "1024";

/// The Passphrase the joining device chooses, which is deliberately not the one
/// the Library was created under: the stored form is per device (spec: KD-9).
const OWN_PASSPHRASE: &str = "a second device, a second passphrase";

// The whole journey, in the order a person makes it. One case rather than five,
// because each step is the state the next one runs against: a fetch has nothing
// to fetch until a sync has uploaded, and neither says anything until a second
// device has joined.
#[tokio::test]
async fn a_folder_goes_into_the_library_and_comes_back_out_of_it() {
    let Some(minio) = support::minio("round-trip") else {
        eprintln!("skipped: no S3 implementation is configured");
        return;
    };
    minio.ensure_bucket().await;
    let device = Device::new();

    // 1. A Library, a folder of files, and a sync that carries them into it.
    let created = init(&device, "a", &minio);
    let recovery_code = printed_code(&created);
    let prefix = printed_prefix(&created);

    let source = device.folder("source");
    let files = written_files(&source);
    map(&device, "a", &source);

    let synced = device.run_with(
        &["sync", "--library", "a", "--passphrase-stdin"],
        Some(PASSPHRASE),
    );
    succeeded(&synced, "sync");
    let said = summary(&synced);
    assert!(
        said.starts_with(&format!("added {}, ", files.len())),
        "the sync must say what it added: {said:?}"
    );
    assert!(
        said.contains("committed head "),
        "a sync that uploaded has committed a record (spec: CP-1): {said:?}"
    );

    // 2. A freeze, which puts what the sync left one Container per file into
    //    Packs (spec: PK-1, PK-7).
    let frozen = device.run_with(
        &[
            "freeze",
            "--library",
            "a",
            "--under",
            PREFIX,
            "--target",
            TARGET,
            "--passphrase-stdin",
        ],
        Some(PASSPHRASE),
    );
    succeeded(&frozen, "freeze");
    let said = summary(&frozen);
    assert!(
        !said.starts_with("packs 0 "),
        "the freeze must build at least one Pack: {said:?}"
    );
    assert!(
        said.contains(&format!("absorbed {}", files.len())),
        "every one-file Container the sync made is absorbed: {said:?}"
    );

    // 3. A second device, from the Recovery Code and the prefix `init` printed,
    //    and a fetch that fills a folder that has never held any of this.
    let elsewhere = device.folder("elsewhere");
    join(&device, "b", &recovery_code, &prefix, &minio);
    map(&device, "b", &elsewhere);

    let fetched = device.run_with(
        &[
            "fetch",
            "--library",
            "b",
            "--under",
            PREFIX,
            "--passphrase-stdin",
        ],
        Some(OWN_PASSPHRASE),
    );
    succeeded(&fetched, "fetch");
    let said = summary(&fetched);
    assert!(
        said.starts_with(&format!("fetched {}, ", files.len())),
        "the fetch must say what it placed: {said:?}"
    );
    assert_eq!(
        read_files(&elsewhere),
        files,
        "what comes back out must be what went in, byte for byte"
    );

    // And one Entry on its own, which reads the part of its Container that holds
    // it rather than the Container around it (spec: PK-16).
    let one = device.folder("one-entry");
    join(&device, "c", &recovery_code, &prefix, &minio);
    map(&device, "c", &one);

    let (wanted, contents) = files.iter().next().expect("the folder is not empty");
    let placed = device.run_with(
        &[
            "fetch",
            "--library",
            "c",
            "--entry",
            &format!("{PREFIX}/{wanted}"),
            "--passphrase-stdin",
        ],
        Some(OWN_PASSPHRASE),
    );
    succeeded(&placed, "fetch --entry");
    assert_eq!(
        read_files(&one),
        BTreeMap::from([(wanted.clone(), contents.clone())]),
        "one Entry means one file and no others"
    );

    // 4. A file gone from the folder it was synced from. Propagating a deletion
    //    is an explicit flow of its own, so the run reports it and leaves the
    //    Library exactly as it is (spec: EP-10, PK-14) — and says so with a
    //    status a script notices without reading a line.
    std::fs::remove_file(source.join(wanted)).expect("the source file must be removable");
    let again = device.run_with(
        &["sync", "--library", "a", "--passphrase-stdin"],
        Some(PASSPHRASE),
    );
    assert_eq!(
        code(&again),
        FINDINGS,
        "a run that left something for somebody to act on says so; stderr was:\n{}",
        stderr(&again)
    );
    let reported = stdout(&again);
    assert!(
        reported.contains(&format!("surfaced {PREFIX}/{wanted}: ")),
        "the finding must name the file that is gone: {reported:?}"
    );

    // 5. A Passphrase that is not the one the stored form was written under.
    //    Nothing is read as key material on the way to finding out (spec: DK-5),
    //    so nothing reaches Storage either.
    let before = minio.keys_under(&prefix).await;
    assert!(!before.is_empty(), "the Library has been synced by now");

    let refused = device.run_with(
        &["sync", "--library", "a", "--passphrase-stdin"],
        Some("not the Passphrase"),
    );
    assert_eq!(code(&refused), 1, "a wrong Passphrase must fail the run");
    assert_eq!(
        minio.keys_under(&prefix).await,
        before,
        "a run that never opened the Master Key cannot have written anything"
    );
}

// A bucket that is not in the implementation, which is what a mistyped one is.
// On S3 a prefix exists by being written under, so nothing else about setting a
// Library up would notice: without the question `init` asks, a person would hold
// a Recovery Code for a Library that is nowhere and find out at the first sync
// (spec: FM-18). Against a real implementation rather than a port nothing is
// listening at, because the answer under test is S3's own for a bucket that is
// not there — a refusal from a reachable, correctly signed endpoint — and no
// stub can be trusted to give it.
#[tokio::test]
async fn a_bucket_the_implementation_does_not_hold_creates_nothing() {
    let Some(minio) = support::minio("absent-bucket") else {
        eprintln!("skipped: no S3 implementation is configured");
        return;
    };
    let device = Device::new();

    // Named after the bucket the run does have, so that what separates the two
    // is only whether this one was created.
    let absent = format!("{}-absent", minio.bucket);
    let output = device.run_with(
        &[
            "init",
            "--name",
            "nowhere",
            "--s3",
            "--bucket",
            &absent,
            "--endpoint",
            &minio.endpoint,
            "--region",
            REGION,
            "--path-style",
            "--passphrase-stdin",
        ],
        Some(PASSPHRASE),
    );

    assert_eq!(
        code(&output),
        1,
        "a bucket that is not there must fail the run; stderr was:\n{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains(&absent),
        "the refusal must name the bucket: {:?}",
        stderr(&output)
    );
    // The one thing worse than the refusal would be a code for a Library that
    // does not exist anywhere.
    assert!(!stdout(&output).contains(RECOVERY_CODE_PREFIX));
    assert!(!device.libraries().join("nowhere").exists());
}

/// Creates the Library `name` in the bucket.
fn init(device: &Device, name: &str, minio: &Minio) -> Output {
    let output = device.run_with(
        &[
            "init",
            "--name",
            name,
            "--s3",
            "--bucket",
            &minio.bucket,
            "--prefix",
            &minio.base_prefix,
            "--endpoint",
            &minio.endpoint,
            "--region",
            REGION,
            "--path-style",
            "--passphrase-stdin",
        ],
        Some(PASSPHRASE),
    );
    succeeded(&output, "init");
    output
}

/// Takes the same Library up under `name`, from the code and prefix `init`
/// printed.
fn join(device: &Device, name: &str, recovery_code: &str, prefix: &str, minio: &Minio) {
    let output = device.run_with(
        &[
            "join",
            "--name",
            name,
            "--recovery-code",
            recovery_code,
            "--s3",
            "--bucket",
            &minio.bucket,
            "--prefix",
            prefix,
            "--endpoint",
            &minio.endpoint,
            "--region",
            REGION,
            "--path-style",
            "--passphrase-stdin",
        ],
        Some(OWN_PASSPHRASE),
    );
    succeeded(&output, "join");
}

/// Records that `local_root` holds the one folder these cases work in.
fn map(device: &Device, name: &str, local_root: &Path) {
    let output = device.run(&[
        "map",
        "--library",
        name,
        "--prefix",
        PREFIX,
        local_root.to_str().expect("the folder has a usable name"),
    ]);
    succeeded(&output, "map");
}

/// The summary line a run printed, which is the first line of its output.
fn summary(output: &Output) -> String {
    stdout(output)
        .lines()
        .next()
        .unwrap_or_else(|| {
            panic!(
                "a run must print a summary; stderr was:\n{}",
                stderr(output)
            )
        })
        .to_owned()
}

/// Writes the folder every case here syncs, and reports what is in it.
///
/// A subfolder among them, because an Entry Path is a path rather than a name
/// and a fetch has to make the folders on its way to the file (spec: EP-9).
fn written_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let files = BTreeMap::from([
        ("first.txt".to_owned(), b"the first file".to_vec()),
        ("second.txt".to_owned(), b"the second file".to_vec()),
        (
            "nested/third.txt".to_owned(),
            b"the third file, one folder down".to_vec(),
        ),
    ]);
    for (name, contents) in &files {
        write_file(&root.join(name), contents);
    }
    files
}

/// Every file under `root`, by the path it stands at relative to it.
fn read_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the folder must be readable") {
            let path = entry.expect("the entry must be readable").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .expect("everything found is under the root")
                .to_str()
                .expect("the folders these cases make have usable names")
                .to_owned();
            found.insert(
                relative,
                std::fs::read(&path).expect("the file must be readable"),
            );
        }
    }
    found
}
