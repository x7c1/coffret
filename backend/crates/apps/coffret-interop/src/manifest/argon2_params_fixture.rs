use coffret_format::Argon2Params;
use serde::{Deserialize, Serialize};

/// The Argon2id cost a stored form records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Argon2ParamsFixture {
    /// Memory cost in KiB.
    pub memory_kib: u32,
    /// Number of passes over that memory.
    pub iterations: u32,
    /// How many lanes the passes are spread across.
    pub parallelism: u32,
}

impl From<Argon2Params> for Argon2ParamsFixture {
    fn from(params: Argon2Params) -> Self {
        Self {
            memory_kib: params.memory_kib(),
            iterations: params.iterations(),
            parallelism: params.parallelism(),
        }
    }
}
