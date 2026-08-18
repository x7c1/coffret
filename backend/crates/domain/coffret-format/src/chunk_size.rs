use crate::error::{Error, Result};

/// How many plaintext bytes go into one Container chunk.
///
/// The chunk size is a per-Container parameter recorded in the header, not a
/// format constant: a new Container may adopt a different size without a format
/// version change, and a reader always honors the value it finds in the header
/// rather than assuming the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkSize(u32);

impl ChunkSize {
    /// The size new Containers are written with: 1 MiB.
    pub const DEFAULT: Self = Self(1024 * 1024);

    /// Wraps a chunk size, which must be greater than zero.
    pub fn new(bytes: u32) -> Result<Self> {
        if bytes == 0 {
            return Err(Error::InvalidChunkSize);
        }
        Ok(Self(bytes))
    }

    /// The size in bytes.
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Default for ChunkSize {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FM-6: the initial chunk size is 1 MiB.
    #[test]
    fn default_is_one_mebibyte() {
        assert_eq!(ChunkSize::default().get(), 1024 * 1024);
    }

    #[test]
    fn zero_is_rejected() {
        assert_eq!(ChunkSize::new(0), Err(Error::InvalidChunkSize));
    }
}
