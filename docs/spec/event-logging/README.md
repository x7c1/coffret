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
  credential. A person-facing refusal may identify a file, a Library, or a
  mapping that person owns — a mapping by the top-level component it stands for
  (EP-9), or by the local folder it names; that rendering is not reused for an
  event. This is enforced by constructing event fields from log-safe facts and
  `Redacted` renderings rather than `Display`. *(Form: prose — absence across
  every event site is a review and construction obligation; regression tests
  cover concrete boundaries.)*
- **EL-2.** `Redacted` renders one typed error link as
  `Vocabulary::Variant`, with permitted facts appended as comma-separated
  `key=value` entries in parentheses. A permitted typed cause follows `: ` and
  applies the same grammar recursively. The rendering describes identity and
  evidence rather than borrowing a person-facing sentence. *(Form: prose — the
  grammar binds every vocabulary that renders through `Redacted`, later ones
  included, so no test closes the set; it is honored by construction in each
  implementation, and each vocabulary's own rendering tests sample it.)*
- **EL-3.** An I/O failure is summarized by `io::ErrorKind` and, where useful,
  the fixed operation that failed; its message is not diagnostic evidence
  because a custom error may embed a private path. An Entry Path may contribute
  its byte length where that distinguishes failures without revealing the
  name. Length is only a summary, never permission to retain any component.
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
