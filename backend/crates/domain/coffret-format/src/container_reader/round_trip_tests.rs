//! Reading one Entry out of a Container without reading the Container.

use coffret_model::ContainerKind;

use super::testing::{
    container_id, entry_of, filler, key, outline_of, pack, read_entry, SMALL_CHUNK,
};
use super::ChunkRunReader;

// FM-2, FM-9: a Container's shape is settled by its header and meta section, so
// the front of the object is enough to place every Entry and every chunk.
#[test]
fn an_outline_is_read_from_the_front_of_the_object() {
    let contents = vec![filler(40, 0x10), filler(70, 0x20)];
    let object = pack(&contents);
    let outline = outline_of(&object);

    assert_eq!(outline.container_id(), container_id());
    assert_eq!(outline.chunk_size().get(), SMALL_CHUNK);
    assert_eq!(outline.kind(), ContainerKind::Pack);
    assert_eq!(outline.entries().len(), 2);
    assert_eq!(
        outline.object_len(),
        object.len() as u64,
        "the outline says how long the object it came from is",
    );
    assert_eq!(
        outline.entries()[1].extent.offset(),
        contents[0].len() as u64,
        "the entry table tiles the stream from offset zero (spec: FM-9)",
    );
}

// FM-5, PK-16: an Entry inside one chunk costs that one chunk, and its bytes
// come back exactly.
#[test]
fn an_entry_inside_one_chunk_is_read_from_that_chunk() {
    // The first Entry is shorter than a chunk, so it never leaves chunk 0.
    let contents = vec![filler(10, 0x10), filler(200, 0x20)];
    let object = pack(&contents);

    let entry = entry_of(&object, "books/atlas/000.jpg");
    let outline = outline_of(&object);
    let run = outline
        .chunks_covering(entry.extent.range())
        .expect("the extent lies inside the stream");
    assert_eq!(run.count(), 1, "one chunk covers the Entry");

    let (content, asked) = read_entry(&object, "books/atlas/000.jpg");
    assert_eq!(content, contents[0]);
    assert!(
        asked < object.len() as u64,
        "reading one Entry asked for {asked} of {} bytes",
        object.len(),
    );
}

// FM-5: an Entry that straddles a chunk boundary is read from every chunk it
// touches, and the bytes in front of it inside the first chunk are the caller's
// to skip.
#[test]
fn an_entry_spanning_a_chunk_boundary_is_read_whole() {
    // 10 bytes in, then 40 bytes across chunks 0, 1, 2 and 3.
    let contents = vec![filler(10, 0x10), filler(40, 0x20), filler(120, 0x30)];
    let object = pack(&contents);

    let entry = entry_of(&object, "books/atlas/001.jpg");
    let outline = outline_of(&object);
    let run = outline
        .chunks_covering(entry.extent.range())
        .expect("the extent lies inside the stream");
    assert!(run.count() > 1, "the Entry spans a chunk boundary");
    assert_eq!(run.first(), 0);
    assert_ne!(
        run.plaintext_start(),
        entry.extent.offset(),
        "the run starts at a chunk boundary, before the Entry does",
    );

    let (content, asked) = read_entry(&object, "books/atlas/001.jpg");
    assert_eq!(content, contents[1]);
    assert!(asked < object.len() as u64);
}

// FM-4, FM-7: the last Entry ends in the final chunk, whose message is short
// and whose nonce carries the final-chunk domain, and whatever padding follows
// it inside that chunk is discarded rather than delivered.
#[test]
fn an_entry_ending_in_the_final_chunk_is_read_whole() {
    let contents = vec![filler(50, 0x10), filler(37, 0x20)];
    let object = pack(&contents);

    let outline = outline_of(&object);
    let entry = entry_of(&object, "books/atlas/001.jpg");
    let run = outline
        .chunks_covering(entry.extent.range())
        .expect("the extent lies inside the stream");
    assert!(
        outline.pad_len() > 0,
        "the case wants a padding tail behind the last Entry (spec: FM-4)",
    );
    assert_eq!(
        run.ciphertext().end,
        object.len() as u64,
        "the run reaches the final chunk, so it ends at the end of the object",
    );

    let (content, _) = read_entry(&object, "books/atlas/001.jpg");
    assert_eq!(content, contents[1]);
}

// FM-4: an Entry of length zero still names the chunk it stands at, so a run is
// never empty and a reader never has to special-case one.
#[test]
fn an_empty_entry_names_the_chunk_it_stands_at() {
    let contents = vec![filler(48, 0x10), Vec::new()];
    let object = pack(&contents);

    let (content, _) = read_entry(&object, "books/atlas/001.jpg");
    assert!(content.is_empty());
}

// Feeding the run in the pieces a transfer happens to deliver them in is the
// same answer as feeding it in one, which is the whole point of a reader that
// takes bytes as they arrive.
#[test]
fn plaintext_is_the_same_however_the_ciphertext_is_split() {
    let contents = vec![filler(200, 0x10)];
    let object = pack(&contents);
    let outline = outline_of(&object);
    let run = outline.all_chunks();
    let asked = run.ciphertext();
    let ciphertext = &object[asked.start as usize..asked.end as usize];

    let mut whole = Vec::new();
    let mut reader = ChunkRunReader::begin(&outline, &key(), &run);
    reader.read(ciphertext, &mut whole).expect("the run opens");
    reader.finish().expect("the whole run arrived");

    for piece in [1usize, 7, 13] {
        let mut split = Vec::new();
        let mut reader = ChunkRunReader::begin(&outline, &key(), &run);
        for part in ciphertext.chunks(piece) {
            reader.read(part, &mut split).expect("the run opens");
        }
        reader.finish().expect("the whole run arrived");
        assert_eq!(split, whole, "delivered {piece} bytes at a time");
    }

    assert_eq!(
        whole.len() as u64,
        outline.plaintext_len(),
        "every chunk of the object carries the whole padded stream (spec: FM-4)",
    );
    assert_eq!(&whole[..contents[0].len()], &contents[0][..]);
}
