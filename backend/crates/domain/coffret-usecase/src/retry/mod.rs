//! Trying a Storage call again, and knowing when to stop.
//!
//! [`Error::is_retryable`](crate::Error::is_retryable) says whether another
//! attempt could succeed; this is what acts on the answer, so that no call site
//! writes a loop of its own and no two of them disagree about how long a worker
//! may sit waiting.

mod retry_policy;
pub use retry_policy::RetryPolicy;

#[cfg(test)]
mod tests;
