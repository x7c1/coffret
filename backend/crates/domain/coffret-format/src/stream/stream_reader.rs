use crate::entry_source::EntrySource;

/// Fills chunk-sized buffers from the entries and the padding tail.
pub(crate) struct StreamReader<'a> {
    entries: &'a [EntrySource<'a>],
    entry_index: usize,
    entry_offset: usize,
    padding_left: u64,
}

impl<'a> StreamReader<'a> {
    pub(crate) fn new(entries: &'a [EntrySource<'a>], pad_len: u64) -> Self {
        Self {
            entries,
            entry_index: 0,
            entry_offset: 0,
            padding_left: pad_len,
        }
    }

    /// Fills `buffer` from the stream, returning how many bytes were written.
    ///
    /// A short return means the stream is exhausted.
    pub(crate) fn read(&mut self, buffer: &mut [u8]) -> usize {
        let mut written = 0;
        while written < buffer.len() {
            match self.entries.get(self.entry_index) {
                Some(entry) => {
                    let remaining = &entry.content[self.entry_offset..];
                    if remaining.is_empty() {
                        self.entry_index += 1;
                        self.entry_offset = 0;
                        continue;
                    }
                    let take = remaining.len().min(buffer.len() - written);
                    buffer[written..written + take].copy_from_slice(&remaining[..take]);
                    self.entry_offset += take;
                    written += take;
                }
                None => {
                    if self.padding_left == 0 {
                        break;
                    }
                    let take = usize::try_from(self.padding_left)
                        .unwrap_or(usize::MAX)
                        .min(buffer.len() - written);
                    buffer[written..written + take].fill(0);
                    self.padding_left -= take as u64;
                    written += take;
                }
            }
        }
        written
    }
}
