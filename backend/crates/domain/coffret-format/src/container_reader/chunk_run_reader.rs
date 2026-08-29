use coffret_model::ContainerKey;

use crate::aead::Cipher;
use crate::container_reader::chunk_layout::ChunkLayout;
use crate::container_reader::chunk_run::ChunkRun;
use crate::container_reader::container_outline::ContainerOutline;
use crate::error::{Error, Result};
use crate::header::Header;
use crate::nonce;

/// Opens the chunks of one run as their ciphertext arrives.
///
/// The read-side counterpart of [`ContainerWriter`](crate::ContainerWriter), and
/// bounded the same way: one chunk of ciphertext and one chunk of plaintext at a
/// time, whatever the object weighs. Bytes are fed in whatever pieces a transfer
/// delivers them in and the plaintext of each chunk is appended to a `Vec` the
/// caller owns and drains, so the memory a reader spends is its own buffer and
/// not the Container.
///
/// A chunk reaches `out` only once its tag has verified against the header the
/// run was cut from (spec: FM-1, FM-5, FM-8), and its nonce carries the position
/// it holds in the *object* — not in the run — so a run of chunks read out of
/// the middle of a Pack is authenticated as exactly those chunks of exactly that
/// Container (spec: FM-7).
///
/// # On error, discard the plaintext
///
/// [`read`](Self::read) appends as it goes. A run that fails part-way has
/// released the chunks that verified before it, which are genuine bytes of the
/// Container but not the answer to what was asked; a caller that keeps them is
/// keeping half an answer. Discard `out`, and the reader with it.
pub struct ChunkRunReader {
    cipher: Cipher,
    header_bytes: [u8; Header::LEN],
    layout: ChunkLayout,
    /// The chunk the next ciphertext byte belongs to.
    index: u64,
    /// One past the run's last chunk.
    end: u64,
    /// The part of the current chunk's message that has arrived.
    message: Vec<u8>,
    /// How many ciphertext bytes the run covers, and how many have arrived.
    expected: u64,
    delivered: u64,
}

impl ChunkRunReader {
    /// Starts a run, ready for the bytes [`ChunkRun::ciphertext`] names.
    pub fn begin(outline: &ContainerOutline, key: &ContainerKey, run: &ChunkRun) -> Self {
        let ciphertext = run.ciphertext();
        Self {
            cipher: Cipher::new(key.as_bytes()),
            header_bytes: outline.header_bytes(),
            layout: run.layout(),
            index: run.first(),
            end: run.first() + run.count(),
            message: Vec::new(),
            expected: ciphertext.end - ciphertext.start,
            delivered: 0,
        }
    }

    /// Feeds the next ciphertext bytes, appending the plaintext of every chunk
    /// they complete to `out`.
    ///
    /// A chunk that arrives in one piece is opened out of the caller's slice
    /// directly; one that arrives split is held until the rest of its message
    /// does. Either way nothing reaches `out` unauthenticated.
    pub fn read(&mut self, ciphertext: &[u8], out: &mut Vec<u8>) -> Result<()> {
        let mut remaining = ciphertext;
        while !remaining.is_empty() {
            if self.index == self.end {
                return Err(Error::ChunkRunOverrun {
                    expected: self.expected,
                    actual: self.delivered + ciphertext.len() as u64,
                });
            }
            let message_len = usize::try_from(self.layout.message_len_of(self.index))
                .map_err(|_| Error::InvalidChunkSize)?;

            if self.message.is_empty() && remaining.len() >= message_len {
                let (message, rest) = remaining.split_at(message_len);
                self.open(message, out)?;
                remaining = rest;
                continue;
            }

            let take = (message_len - self.message.len()).min(remaining.len());
            self.message.extend_from_slice(&remaining[..take]);
            remaining = &remaining[take..];
            if self.message.len() == message_len {
                let message = std::mem::take(&mut self.message);
                self.open(&message, out)?;
            }
        }
        self.delivered += ciphertext.len() as u64;
        Ok(())
    }

    /// Closes the run: every chunk of it has to have arrived whole.
    ///
    /// A run that ends short is the provider having answered with fewer bytes
    /// than were asked for, and it is named as that rather than left to look
    /// like a Container that will not open.
    pub fn finish(self) -> Result<()> {
        if self.index != self.end || !self.message.is_empty() {
            return Err(Error::ChunkRunTruncated {
                expected: self.expected,
                actual: self.delivered,
            });
        }
        Ok(())
    }

    /// Authenticates one whole chunk message and appends its plaintext.
    fn open(&mut self, message: &[u8], out: &mut Vec<u8>) -> Result<()> {
        let is_final = self.index == self.layout.final_index();
        let plaintext = self.cipher.open(
            &nonce::chunk(self.index, is_final),
            &self.header_bytes,
            message,
        )?;
        out.extend_from_slice(&plaintext);
        self.index += 1;
        Ok(())
    }
}
