use coffret_model::{EntryPath, Mtime};

use crate::local_operation::LocalOperation;
use crate::mapped_roots_conformance::mapped_roots_under_test::MappedRootsUnderTest;
use crate::MappedRelativeLocation;

/// Longer than one buffer of the size this case reads with, so a reader that
/// answered once and stopped would come back short.
const LEN: usize = 5_000;

/// How much the case takes at a time, small enough that a file of [`LEN`] bytes
/// needs several turns.
const CHUNK: usize = 512;

/// A source hands back exactly the bytes that were written, however many turns
/// it takes.
///
/// A reader rather than the whole file, because a Pack's members are larger than
/// memory (spec: PK-5, FM-5) — so what the contract has to say is that a short
/// fill is not the end and only zero is. A capability that returned everything
/// at once would pass this too, which is the point: the caller may not care
/// which.
pub async fn a_source_streams_back_the_bytes_that_were_written(fixture: &MappedRootsUnderTest) {
    let path = fixture.dir().join("photographs").join("spring.jpg");
    let content = filler(LEN);
    fixture
        .arrange()
        .write_file(&path, &content, Mtime::from_unix_seconds(1_600_000_000));

    let mut reader = fixture
        .roots()
        .open_source(fixture.dir(), &mapped("photographs/spring.jpg"))
        .await
        .unwrap_or_else(|error| panic!("opening a file that is there must succeed: {error}"));

    let mut read = Vec::new();
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let filled = reader
            .read(&mut buffer)
            .await
            .unwrap_or_else(|error| panic!("reading a file that is there must succeed: {error}"));
        if filled == 0 {
            break;
        }
        read.extend_from_slice(&buffer[..filled]);
    }

    assert_eq!(read, content, "byte for byte, and in order");
}

/// Opening a source that is not there is refused, and named as a read.
///
/// The one place absence is *not* a verdict: by the time anything opens a source
/// the walk has already found it, so a path that has gone since is a file the run
/// measured and can no longer carry. Which operation refused is what a caller
/// reports, so the refusal says `Reading` rather than leaving a person to guess
/// from the message.
pub async fn opening_a_missing_source_is_refused_as_reading(fixture: &MappedRootsUnderTest) {
    let missing = fixture.dir().join("never-created.jpg");

    let refused = fixture
        .roots()
        .open_source(fixture.dir(), &mapped("never-created.jpg"))
        .await
        .err()
        .expect("a source that is not there cannot be opened");

    assert!(
        matches!(refused.operation, LocalOperation::Reading),
        "the call that was refused was the open of a source, got {refused:?}",
    );
    assert_eq!(refused.path, missing, "and it names the file it was about");
}

/// An open reader keeps the file it acquired even if its name is replaced.
pub async fn an_open_reader_retains_its_bytes_and_length(fixture: &MappedRootsUnderTest) {
    let path = fixture.dir().join("page.jpg");
    let original = b"original bytes";
    fixture.arrange().write_file(&path, original, stamped());
    let relative = mapped("page.jpg");
    let reader = fixture
        .roots()
        .open_source(fixture.dir(), &relative)
        .await
        .expect("the original opens");
    assert_eq!(reader.len(), original.len() as u64);

    fixture
        .arrange()
        .replace_file(&path, b"a longer replacement", stamped());

    let mut reader = reader;
    let mut read = Vec::new();
    let mut buffer = [0; 4];
    loop {
        let filled = reader.read(&mut buffer).await.expect("the handle reads");
        if filled == 0 {
            break;
        }
        read.extend_from_slice(&buffer[..filled]);
    }
    assert_eq!(read, original);
}

fn stamped() -> Mtime {
    Mtime::from_unix_seconds(1_600_000_000)
}

/// A source requires directory parents and a regular final name.
pub async fn source_parents_must_be_folders_and_final_names_regular_files(
    fixture: &MappedRootsUnderTest,
) {
    let parent = fixture.dir().join("blocked");
    fixture.arrange().plant_other(&parent);
    let below = mapped("blocked/page.jpg");
    assert!(fixture
        .roots()
        .open_source(fixture.dir(), &below)
        .await
        .is_err());

    let final_path = fixture.dir().join("linked.jpg");
    fixture.arrange().plant_other(&final_path);
    let final_name = mapped("linked.jpg");
    assert!(fixture
        .roots()
        .open_source(fixture.dir(), &final_name)
        .await
        .is_err());
}

/// A link substituted after enumeration is refused by the later source open.
pub async fn a_source_substituted_after_enumeration_is_not_followed(
    fixture: &MappedRootsUnderTest,
) {
    let folder = fixture.dir().join("album");
    let file = folder.join("page.jpg");
    fixture.arrange().write_file(&file, b"inside", stamped());
    let folder_relative = mapped("album");
    fixture
        .roots()
        .list_folder(fixture.dir(), Some(&folder_relative))
        .await
        .expect("enumeration succeeds")
        .expect("the folder exists");
    fixture.arrange().plant_other(&file);
    let file_relative = mapped("album/page.jpg");
    assert!(fixture
        .roots()
        .open_source(fixture.dir(), &file_relative)
        .await
        .is_err());
}

fn mapped(path: &str) -> MappedRelativeLocation {
    MappedRelativeLocation::from_entry_path(&EntryPath::parse(path).expect("fixture path"))
}

/// Content that differs along its whole length, so a read that dropped or
/// reordered a stretch of it comes back different rather than the same.
fn filler(len: usize) -> Vec<u8> {
    (0..len)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(7))
        .collect()
}
