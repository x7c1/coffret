# Event Logging

Rule prefix: `EL`. What coffret may retain in local diagnostic events, how a
failure and its causes are rendered there, and how that record differs from a
person-facing refusal and the Library's Journal.

Concept background: [Entry Path](../../concepts/entry-path/),
[Library](../../concepts/library/), [Journal](../../concepts/journal/),
[Storage](../../concepts/storage/).

Diagnostic events are plaintext device records used to explain operation and
failure. They are not the [Journal](../../concepts/journal/): the Journal is an
encrypted Storage control-object chain that commits Library state, while an
event is local operational evidence and never controls or reconstructs that
state.

## Rules

- **EL-1.** A diagnostic event must not contain an Entry Path, a local path or
  filename, a device-local Library name, plaintext file content, a
  cryptographic key, a Passphrase, a Recovery Code, or a token or other bearer
  credential. An event is a record an operation leaves behind as a side
  effect: whoever invoked the operation did not ask for it and does not receive
  it, and that record is what this rule binds. A person-facing refusal is a
  response to the operation that person invoked, and naming what was refused is
  part of answering, so it may identify a file, a Library, or a mapping — a
  mapping by the top-level component it stands for (EP-9), or by the local
  folder it names. A rendering written for a response is not reused for an
  event, and a response coffret also writes somewhere that outlives the process
  is a record there, whatever it was first written for. This is enforced by constructing event fields from log-safe facts and
  `Redacted` renderings rather than `Display`. *(Form: prose — absence across
  every event site is a review and construction obligation; regression tests
  cover concrete boundaries.)*
- **EL-2.** `Redacted` renders one typed error link as
  `Vocabulary::Variant`, with permitted facts appended as comma-separated
  `key=value` entries in parentheses. A permitted typed cause follows `: ` and
  applies the same grammar recursively. The rendering describes identity and
  evidence rather than borrowing a person-facing sentence. Two links stand
  beside that grammar — three shapes in all — and both are named here because
  a rule describing one of the three would read as forbidding the other two.
  A link that ends at a foreign cause — one coffret has no vocabulary of
  variants for — stops at the safe summary EL-4 admits, and that summary
  stands where an identity would: an `io::Error` is its kind and nothing more,
  which is all EL-3 lets one contribute. A link is rendered as the message it
  reads rather than as an identity where the sentence is itself the evidence,
  and two vocabularies stand on that footing for different reasons. The
  Storage one keeps what a provider answered, and is safe on the redaction
  EL-5 obliges whoever builds the value to have done, on nothing the rendering
  itself checks. The format one keeps what a reader made of bytes, and is safe
  because its sentences are composed about the shape of what it read rather
  than lifted out of a payload, so nothing anybody chose reaches one (EL-1). A
  vocabulary is admitted to this shape only by stating which of the two its
  own sentences hold to. Neither shape is an identity records can be grouped
  by, which is what each costs. *(Form: prose — the grammar binds every
  vocabulary that renders through `Redacted`, later ones included, so no test
  closes the set; it is honored by construction in each implementation, and
  each vocabulary's own rendering tests sample it.)*
- **EL-3.** An I/O failure is summarized by `io::ErrorKind` and, where useful,
  the fixed operation that failed; its message is not diagnostic evidence
  because a custom error may embed a private path. An Entry Path may contribute
  its byte length where that distinguishes failures without revealing the
  name. Length is only a summary, never permission to retain any component.
  The plaintext size of one file is held to the same line: an exact size can
  match a file somebody else also has, so it is carried where the size is what
  explains the outcome — a provider's ceiling, a Pack boundary, an upload that
  stopped partway — and never as a field an Entry is told apart or looked up
  by. Counts and totals over a run say nothing about any one file, and an
  outcome event carries them freely.
  *(Form: prose — a custom `io::Error`'s message is arbitrary text from outside
  coffret, so no test closes what it may contain; the summaries are honored by
  construction wherever a failure is rendered, with regression tests over
  concrete refusals.)*
- **EL-4.** A diagnostic cause chain crosses only typed boundaries whose
  log-safe rendering is defined. Each coffret error renders its permitted cause
  through `Redacted`; an unknown foreign cause stops at a fixed identity or an
  explicit safe summary such as error kind. An event never walks an arbitrary
  `std::error::Error::source` chain or formats it wholesale. *(Form: prose — an
  open foreign cause chain cannot be exhaustively tested.)*
- **EL-5.** Storage diagnostics may retain operation, status, structured
  reason, counts, ciphertext sizes, and object identifiers generated or derived
  by coffret or minted by a provider — a Container's opaque name and a control
  object's recognizable one alike (FM-12). The location a Library was
  configured into — a bucket, a base prefix, or a provider folder a person
  chose — is that person's own arrangement rather than an identifier coffret or
  a provider minted, so it is not among them and no event field is composed
  from it; the app folder inside it is named after the Library ID, and that
  name stays permitted evidence. Provider messages and bodies may be retained
  only after credential redaction and removal of private request data, that
  configured location included. A provider body is not safe merely because
  ordinary object identifiers are opaque: the provider may echo any part of
  the request. *(Form: prose — providers can add new response text outside
  coffret's control.)*
