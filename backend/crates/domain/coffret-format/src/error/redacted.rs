//! What a diagnostic event may be told of an [`Error`]: the message itself, for
//! every variant but four.

use coffret_model::Redacted;

use super::Error;

impl Redacted for Error {
    /// The message, for every variant but four.
    ///
    /// This is the one vocabulary in the workspace whose messages are safe to
    /// write down as they stand, and it is safe by what it is about rather than
    /// by care taken at each site: every variant here describes the *bytes* of
    /// an object — a magic number, a declared length, a chunk index, which
    /// purpose key a message needed, which schema a payload states — and an
    /// object is the encrypted form, whose whole point is that it names nothing
    /// anybody chose. The `detail` strings are of that same kind, and they have
    /// two provenances: partly an account a CBOR reader gave of bytes that are
    /// not the shape a schema spells, partly sentences this crate composes
    /// about the bytes it read — how many followed a map, which field stood
    /// outside which bound, which shape stood in a field's place. Either way
    /// nothing is lifted out of a payload, and that is the property that makes
    /// them safe to write down.
    ///
    /// [`Model`](Self::Model) is the exception and the reason this is a match
    /// rather than a blanket rendering: that variant carries the domain layer's
    /// own refusal, and two of those do name a path (spec: EL-1, EL-4). That
    /// refusal's own rendering goes underneath, so the rule holds however deep
    /// the chain goes.
    ///
    /// The other three arms are the variants whose `Display` says only which
    /// step refused, leaving what the layer below reported to the cause
    /// `source` hands on. A reader of a diagnostic event has no chain to walk,
    /// so this is where that cause is written out — which is why these three
    /// spell what the blanket arm used to spell for them rather than dropping
    /// to the step alone. Each one's cause holds to the rule.
    /// [`EntropyUnavailable`](Self::EntropyUnavailable)'s:
    /// `getrandom` prints either a sentence of its own about this machine's
    /// random source or the operating system's message for the errno it was
    /// given — built from that code rather than from the custom error EL-3
    /// distrusts a message for — and neither names a path, a filename, or any
    /// content of anybody's. And the two Argon2id ones —
    /// [`InvalidArgon2Params`](Self::InvalidArgon2Params)'s and
    /// [`PassphraseDerivationFailed`](Self::PassphraseDerivationFailed)'s:
    /// `argon2::Error` renders a sentence from a closed set written into that
    /// crate at compile time — "memory cost is too small", "not enough
    /// threads", "salt is too short" — saying which Argon2id parameter or
    /// input it would not take and never the value of one, and the one variant
    /// of it that wraps another error renders a `base64ct` refusal whose text
    /// is fixed in the same way. So no Passphrase, no salt, no path, and
    /// nothing a person typed can reach it (spec: EL-3, EL-4).
    fn redacted(&self) -> String {
        match self {
            Self::Model(error) => format!("Format::Model: {}", error.redacted()),
            Self::InvalidArgon2Params { cause } => {
                format!("Format: invalid Argon2id parameters: {cause}")
            }
            Self::PassphraseDerivationFailed { cause } => {
                format!("Format: could not derive the protection key: {cause}")
            }
            Self::EntropyUnavailable { cause } => {
                format!("Format: could not draw random bytes: {cause}")
            }
            other => format!("Format: {other}"),
        }
    }
}
