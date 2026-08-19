/// A finished control object: the bytes to upload and the name to upload them
/// under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedControlObject {
    bytes: Vec<u8>,
    object_name: String,
}

impl EncodedControlObject {
    pub(super) fn new(bytes: Vec<u8>, object_name: String) -> Self {
        Self { bytes, object_name }
    }

    /// The full object, header first.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The name this object is stored under.
    pub fn object_name(&self) -> &str {
        &self.object_name
    }

    /// Takes the bytes, dropping the name.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
