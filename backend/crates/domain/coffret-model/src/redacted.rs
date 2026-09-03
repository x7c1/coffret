/// What one failure may say in a log line.
///
/// [`Display`](std::fmt::Display) is written for the person a refusal is shown
/// to, and that person owns the Library: it names the Entry Path that was
/// refused, the folder standing in the way, the Library whose grant ran out.
/// None of that may be written down. Coffret hides the shape of a person's
/// files from Storage behind opaque Container names, and a plaintext log on
/// the same disk that accumulated Entry Paths would be an unencrypted copy of
/// exactly what the format keeps from the provider — outside the reach of
/// whole-disk encryption and outliving every request that made it. So no event
/// may carry an Entry Path, a local file or folder name, or any name a user
/// chose.
///
/// A log line therefore never renders a failure with `Display`. It renders
/// this: each error in a chain says *which* error it is and what may be said
/// about it, and its cause says the same underneath. Nothing about a log line
/// then depends on a message staying free of a path — which is not a property
/// a message written for a person can be held to, since naming the path is what
/// makes it useful to them.
///
/// # The shape
///
/// One link is `Vocabulary::Variant`, followed where there is anything to add
/// by log-safe facts in parentheses as `key=value`, and a cause is appended
/// after `": "`:
///
/// ```text
/// Device::Fetch: Fetch::UnmaterializablePath(path_len=21, descent=blocked)
/// Sync::Storage: Storage is rate limiting, retry in 3s: userRateLimitExceeded
/// ```
///
/// Identities and not sentences, because the questions a log file answers are
/// aggregate ones — which refusal arrives, how often, under which operation —
/// and those are answered by grouping records rather than by reading prose. A
/// path's *length* is the one thing about it that survives: it is what
/// separates "the same Entry every time" from "a different one each run"
/// without saying which.
///
/// The second link above is a message rather than an identity, and it is the
/// one deliberate exception: what a provider answered is what the log file is
/// kept for in the first place, so the Storage port's vocabulary is rendered as
/// it reads. Everything in one of those was minted by the Library or stated by
/// the provider — an object name, a provider reason string, a body a gateway
/// has already taken the credentials out of.
///
/// # What counts as a log-safe fact
///
/// Anything the Library minted or a provider stated, and nothing a person did:
/// object names, Container IDs, generations, replica positions, statuses,
/// counts, sizes, ceilings, hashes, and an `io::Error`'s
/// [`kind`](std::io::ErrorKind). Never a path, a filename, a Library name, a
/// bucket, a mapping prefix, or — the exception above aside — a free-text
/// message that could have one embedded in it.
pub trait Redacted {
    /// This error's identity, the facts about it a log may carry, and its
    /// cause's under it.
    fn redacted(&self) -> String;
}
