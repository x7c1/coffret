//! Moving a Container's plaintext stream past the chunk boundary, one chunk at
//! a time.
//!
//! The stream is every Entry's plaintext back to back in entry-table order,
//! followed by the zero padding the meta section's `pad_len` records. Neither
//! side of the format ever materializes that whole stream: the reader fills one
//! chunk buffer at a time, and the writer scatters one decrypted chunk into the
//! entries it overlaps.

mod stream_reader;
pub(crate) use stream_reader::StreamReader;

mod stream_writer;
pub(crate) use stream_writer::StreamWriter;
