//! Dividing a Container's chunk sequence into parcels (spec: PK-19).

use std::num::NonZeroU64;

use super::testing::{entry_of, filler, key, outline_of, pack, SMALL_CHUNK};
use super::ChunkRunReader;
use crate::parcel_len::ParcelLen;

/// A parcel length of `bytes`, which every case here keeps above zero.
fn parcel_len(bytes: u64) -> ParcelLen {
    ParcelLen::new(NonZeroU64::new(bytes).expect("a case's parcel length is not zero"))
}

// PK-19: parcels are `max(1, S div chunk size)` chunks from the first chunk,
// and the last parcel keeps whatever chunks remain.
#[test]
fn the_chunk_sequence_is_divided_from_its_first_chunk() {
    // 170 bytes in 16-byte chunks: eleven chunks, the last one short.
    let contents = vec![filler(10, 0x10), filler(40, 0x20), filler(120, 0x30)];
    let object = pack(&contents);
    let outline = outline_of(&object);
    let chunks = outline.chunk_count();
    assert!(chunks > 3, "the Container is several parcels long");

    let parcels = outline.parcels(parcel_len(3 * u64::from(SMALL_CHUNK)));
    assert_eq!(parcels.chunks_per_parcel(), 3);
    assert_eq!(parcels.count(), chunks.div_ceil(3));

    let mut next = 0;
    for index in 0..parcels.count() {
        let run = parcels.run(index).expect("every counted parcel has a run");
        assert_eq!(
            run.first(),
            next,
            "parcel {index} starts where the last ended"
        );
        next += run.count();
    }
    assert_eq!(next, chunks, "the parcels cover every chunk once");
    let last = parcels.run(parcels.count() - 1).expect("the last parcel");
    assert_eq!(last.count(), chunks - 3 * (parcels.count() - 1));
    assert!(
        parcels.run(parcels.count()).is_none(),
        "and there is no parcel past it"
    );
}

// PK-19: a parcel length below the chunk size is still one chunk per parcel,
// and a Container no longer than one parcel is one parcel.
#[test]
fn a_parcel_is_at_least_one_chunk_and_a_short_container_is_one_parcel() {
    let object = pack(&[filler(40, 0x10)]);
    let outline = outline_of(&object);

    let tiny = outline.parcels(parcel_len(1));
    assert_eq!(tiny.chunks_per_parcel(), 1);
    assert_eq!(tiny.count(), outline.chunk_count());

    let roomy = outline.parcels(parcel_len(1024 * 1024));
    assert_eq!(roomy.count(), 1);
    assert_eq!(
        roomy.run(0).expect("the one parcel").ciphertext(),
        outline.all_chunks().ciphertext(),
        "one parcel is every chunk of the object",
    );
}

// PK-16: an Entry across a parcel boundary names every parcel it overlaps, and
// the parcels it names decode to its bytes.
#[test]
fn an_entry_across_a_boundary_overlaps_both_parcels() {
    let contents = vec![filler(10, 0x10), filler(40, 0x20), filler(120, 0x30)];
    let object = pack(&contents);
    let outline = outline_of(&object);
    // Two chunks per parcel: the second Entry runs from byte 10 to byte 50,
    // which is chunks 0 to 3 and so parcels 0 and 1.
    let parcels = outline.parcels(parcel_len(2 * u64::from(SMALL_CHUNK)));
    let entry = entry_of(&object, "books/atlas/001.jpg");

    let overlapped = parcels
        .overlapping(&outline, &entry.extent)
        .expect("the extent lies inside the stream");
    assert_eq!(overlapped, 0..2);

    let mut plaintext = Vec::new();
    for index in overlapped.clone() {
        let run = parcels.run(index).expect("an overlapped parcel exists");
        let asked = run.ciphertext();
        let mut reader = ChunkRunReader::begin(&outline, &key(), &run);
        reader
            .read(
                &object[asked.start as usize..asked.end as usize],
                &mut plaintext,
            )
            .expect("the parcel authenticates");
        reader.finish().expect("and arrived whole");
    }
    let from = parcels
        .run(overlapped.start)
        .expect("the first parcel")
        .plaintext();
    assert_eq!(from.start, 0);
    let start = (entry.extent.offset() - from.start) as usize;
    assert_eq!(
        &plaintext[start..start + entry.extent.size() as usize],
        contents[1].as_slice(),
    );
}
