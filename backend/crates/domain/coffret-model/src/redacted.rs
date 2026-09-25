/// What one failure may say in a diagnostic event (spec: EL-1, EL-2).
///
/// [`Display`](std::fmt::Display) is written for the person a refusal is shown
/// to. A refusal is a response to the operation that person invoked, and saying
/// what was refused is part of answering: it names the Entry Path that was
/// refused, the folder standing in the way, the Library whose grant ran out.
/// An event is not a response. It is a record the operation leaves behind as a
/// side effect, which nobody asked for and nobody receives, and none of that
/// may be written into one: a record outlives the files it names and
/// travels where they do not — it stays behind once the volume is unmounted,
/// and it is the thing that gets attached to a bug report. So no event may
/// carry an Entry Path, a local file or folder name, or any name a user chose.
///
/// A diagnostic event therefore never renders a failure with `Display`. It
/// renders this: each error in a chain says *which* error it is and what may
/// be said about it, and its cause says the same underneath. Nothing about an
/// event then depends on a message staying free of a path — which is not a
/// property a message written for a person can be held to, since naming the
/// path is what makes it useful to them.
///
/// # The shape
///
/// One link is `Vocabulary::Variant`, followed where there is anything to add
/// by log-safe facts in parentheses as `key=value`, and a cause is appended
/// after `": "`:
///
/// ```text
/// Device::Fetch: Fetch::UnmaterializablePath(path_len=21, descent=blocked)
/// Sync::Storage: Storage::RateLimited(retry_after=3s)
/// ```
///
/// Identities and not sentences, because the questions a log file answers are
/// aggregate ones — which refusal arrives, how often, under which operation —
/// and those are answered by grouping records rather than by reading prose. A
/// path's *length* is the one thing about it that survives: it is what
/// separates "the same Entry every time" from "a different one each run"
/// without saying which.
///
/// Two shapes stand beside the grammar, and EL-2 admits both. One is a link
/// rendered as the message it reads, where the sentence is itself the
/// evidence: the format layer's, whose sentences are composed about the shape
/// of what it read, and the Storage port's for the failures it composes out of
/// opaque values a gateway hands it — an object name coffret or the provider
/// minted, a count, a digest — which are safe on the gateway having handed it
/// nothing else. What a provider or a transport said in words is not among
/// them, and neither is any free-text account a failure carries, whoever
/// composed it. A provider may echo any part of the request, so the Storage
/// port renders every such failure as an identity with its structured facts,
/// like the second link above, and leaves the words to the event the gateway
/// records where it read them (spec: EL-2, EL-5).
///
/// The other is a link that ends at a foreign cause there is no coffret
/// vocabulary for. It stops at a safe summary in place of an identity — the
/// Storage port's own `Io` is written `Io(kind=…)` and says nothing further,
/// which is all an `io::Error` may contribute (spec: EL-2, EL-3, EL-4). Neither
/// shape can be grouped by the way an identity can, which is what each costs.
///
/// # What counts as a log-safe fact
///
/// Identifiers generated or derived by coffret or minted by a provider,
/// Container IDs, generations, replica positions, statuses, counts, sizes,
/// ceilings, hashes, and an `io::Error`'s [`kind`](std::io::ErrorKind) are
/// permitted. Entry Paths may contribute only their byte length. The complete
/// boundary and cause-chain grammar live in EL-1 through EL-5.
pub trait Redacted {
    /// This error's identity, the facts about it a log may carry, and its
    /// cause's under it.
    fn redacted(&self) -> String;
}
