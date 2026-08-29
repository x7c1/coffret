//! Reading a Container in pieces, without ever holding the whole of it.
//!
//! [`decode`](crate::decode()) answers about an object already in hand, which is
//! what a Container the size of one photograph affords. A Pack is sized in
//! gigabytes (spec: PK-5), and a reader that wants one page out of one must
//! neither wait for the rest nor buffer it — so this is the read side laid out
//! the way [`ContainerWriter`](crate::ContainerWriter) lays out the write side:
//! one chunk at a time, from bytes the caller supplies.
//!
//! It works because a Container tells a reader where everything is before any of
//! it arrives. The header gives the chunk size and the meta section's length
//! (spec: FM-2, FM-6), the meta section gives the entry table and the padding
//! tail (spec: FM-4, FM-9), and between them chunk `k`'s message sits at
//! `Header::LEN + meta_len + k * (chunk_size + tag)` and authenticates on its own
//! (spec: FM-5, FM-7, FM-8). So the three steps here are: read the front
//! ([`ContainerOutline`]), turn an Entry's extent into the chunks that cover it
//! ([`ChunkRun`]), and open exactly those ([`ChunkRunReader`]).
//!
//! Rounding an Entry out to chunk boundaries is not a convenience — a chunk is
//! the smallest thing that authenticates, and coffret never releases plaintext
//! it has not authenticated (spec: FM-1). What a range read cannot do is check
//! the object's own hash, which is a claim about bytes it deliberately did not
//! ask for; per-chunk authentication is the integrity gate for the bytes that do
//! arrive, and whatever the caller's catalog says about the Entry is the gate
//! after that (spec: PK-16).

mod chunk_layout;

mod chunk_run;
pub use chunk_run::ChunkRun;

mod chunk_run_reader;
pub use chunk_run_reader::ChunkRunReader;

mod container_outline;
pub use container_outline::ContainerOutline;

#[cfg(test)]
mod rejection_tests;
#[cfg(test)]
mod round_trip_tests;

#[cfg(test)]
mod testing;
