//! Runs that are refused: a range naming no chunk, a delivery that is not the
//! run that was asked for, and a chunk whose tag does not hold.

use super::testing::{filler, key, outline_of, pack, SMALL_CHUNK};
use super::ChunkRunReader;
use crate::error::Error;

// FM-1, FM-5: a damaged chunk inside the requested run is refused, and no
// plaintext from it reaches the caller.
#[test]
fn a_tampered_chunk_in_the_run_yields_no_plaintext() {
    let contents = vec![filler(200, 0x10)];
    let object = pack(&contents);
    let outline = outline_of(&object);
    let run = outline.all_chunks();
    assert!(run.count() > 2, "the sample needs several chunks");

    // The first chunk of the run, so nothing has been released before it fails.
    let asked = run.ciphertext();
    let mut damaged = object.clone();
    damaged[asked.start as usize] ^= 0x01;

    let mut plaintext = Vec::new();
    let mut reader = ChunkRunReader::begin(&outline, &key(), &run);
    let result = reader.read(
        &damaged[asked.start as usize..asked.end as usize],
        &mut plaintext,
    );
    assert!(
        matches!(result, Err(Error::AuthenticationFailed)),
        "expected a damaged chunk to fail authentication, got {result:?}",
    );
    assert!(
        plaintext.is_empty(),
        "no plaintext from an unauthenticated run reaches the caller (spec: FM-1)",
    );
}

// FM-7: a chunk read under the wrong position, or under the wrong final-chunk
// domain, fails authentication — which is what makes a run a claim about
// exactly those chunks of exactly this Container.
#[test]
fn a_run_read_at_the_wrong_position_fails_authentication() {
    let contents = vec![filler(200, 0x10)];
    let object = pack(&contents);
    let outline = outline_of(&object);

    let first = outline
        .chunks_covering(0..1)
        .expect("the first chunk covers the start of the stream");
    let second = outline
        .chunks_covering(u64::from(SMALL_CHUNK)..u64::from(SMALL_CHUNK) + 1)
        .expect("the second chunk covers the byte after it");
    assert_eq!(second.first(), 1);

    // The second chunk's bytes, offered where the first one's belong.
    let wrong = second.ciphertext();
    let mut plaintext = Vec::new();
    let mut reader = ChunkRunReader::begin(&outline, &key(), &first);
    let result = reader.read(
        &object[wrong.start as usize..wrong.end as usize],
        &mut plaintext,
    );
    assert!(
        matches!(result, Err(Error::AuthenticationFailed)),
        "expected a chunk read at another position to be refused, got {result:?}",
    );
}

// A run cut short is the provider having answered with fewer bytes than were
// asked for, and it is named as that rather than as a Container that will not
// open.
#[test]
fn a_run_that_ends_short_is_refused() {
    let contents = vec![filler(200, 0x10)];
    let object = pack(&contents);
    let outline = outline_of(&object);
    let run = outline.all_chunks();
    let asked = run.ciphertext();

    let mut plaintext = Vec::new();
    let mut reader = ChunkRunReader::begin(&outline, &key(), &run);
    reader
        .read(
            &object[asked.start as usize..asked.end as usize - 4],
            &mut plaintext,
        )
        .expect("the chunks that did arrive whole open");
    let result = reader.finish();
    assert!(
        matches!(result, Err(Error::ChunkRunTruncated { .. })),
        "expected a short delivery to be refused, got {result:?}",
    );
}

// More bytes than the run covers is the same kind of answer from the other
// side, and is refused rather than decoded as a chunk the run never asked for.
#[test]
fn a_run_that_overruns_is_refused() {
    let contents = vec![filler(200, 0x10)];
    let object = pack(&contents);
    let outline = outline_of(&object);
    let run = outline
        .chunks_covering(0..1)
        .expect("the first chunk covers the start of the stream");
    let asked = run.ciphertext();

    let mut plaintext = Vec::new();
    let mut reader = ChunkRunReader::begin(&outline, &key(), &run);
    let result = reader.read(
        &object[asked.start as usize..asked.end as usize + 1],
        &mut plaintext,
    );
    let Err(Error::ChunkRunOverrun { expected, actual }) = result else {
        panic!("expected more ciphertext than the run covers to be refused, got {result:?}");
    };
    // The counts are in the error because they are what separates one byte too
    // many from a provider that ignored the range and sent the whole object.
    assert_eq!(expected, asked.end - asked.start);
    assert_eq!(actual, expected + 1);
}

// A range the object's own stream does not reach names no chunk, and saying so
// is what keeps a catalog describing another state of the Library from aiming a
// read at nothing.
#[test]
fn a_range_past_the_end_of_the_stream_is_refused() {
    let object = pack(&[filler(40, 0x10)]);
    let outline = outline_of(&object);
    let past = outline.plaintext_len() + 1;

    let result = outline.chunks_covering(0..past);
    assert!(
        matches!(result, Err(Error::PlaintextRangeOutOfBounds { .. })),
        "expected a range past the stream to be refused, got {result:?}",
    );
}
