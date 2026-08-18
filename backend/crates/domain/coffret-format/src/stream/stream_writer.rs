use crate::error::{Error, Result};

/// Scatters decrypted chunks into one buffer per Entry.
pub(crate) struct StreamWriter {
    sizes: Vec<u64>,
    contents: Vec<Vec<u8>>,
    entry_index: usize,
    padding_left: u64,
    expected: u64,
    written: u64,
}

impl StreamWriter {
    /// Prepares buffers for entries of the given sizes, followed by `pad_len`
    /// bytes of padding.
    pub(crate) fn new(sizes: Vec<u64>, pad_len: u64, expected: u64) -> Self {
        let contents = sizes
            .iter()
            .map(|size| Vec::with_capacity(usize::try_from(*size).unwrap_or(0)))
            .collect();
        Self {
            sizes,
            contents,
            entry_index: 0,
            padding_left: pad_len,
            expected,
            written: 0,
        }
    }

    /// Appends one chunk's authenticated plaintext to the stream.
    pub(crate) fn write(&mut self, plaintext: &[u8]) -> Result<()> {
        let mut remaining = plaintext;
        while !remaining.is_empty() {
            match self.sizes.get(self.entry_index) {
                Some(size) => {
                    let content = &mut self.contents[self.entry_index];
                    let left = usize::try_from(*size - content.len() as u64).unwrap_or(usize::MAX);
                    if left == 0 {
                        self.entry_index += 1;
                        continue;
                    }
                    let take = left.min(remaining.len());
                    content.extend_from_slice(&remaining[..take]);
                    remaining = &remaining[take..];
                    self.written += take as u64;
                }
                None => {
                    // The padding tail is discarded, but only up to `pad_len`:
                    // anything past it means the stream is longer than the meta
                    // section says it is.
                    let left = usize::try_from(self.padding_left).unwrap_or(usize::MAX);
                    let take = left.min(remaining.len());
                    if take == 0 {
                        return Err(Error::PlaintextLengthMismatch {
                            expected: self.expected,
                            actual: self.written + remaining.len() as u64,
                        });
                    }
                    if remaining[..take].iter().any(|byte| *byte != 0) {
                        return Err(Error::NonZeroPadding);
                    }
                    self.padding_left -= take as u64;
                    remaining = &remaining[take..];
                    self.written += take as u64;
                }
            }
        }
        Ok(())
    }

    /// How many stream bytes have been written so far.
    pub(crate) fn written(&self) -> u64 {
        self.written
    }

    /// The per-Entry plaintext buffers, in entry-table order.
    pub(crate) fn into_contents(self) -> Vec<Vec<u8>> {
        self.contents
    }
}
