use coffret_model::MAX_FORMAT_INTEGER;

use crate::error::{Error, Result};

/// One unsigned integer of a serde-deserialized wire map, held to the bound the
/// format puts on every integer it carries (spec: FM-19).
///
/// The meta section's maps and the entry maps a control payload carries are
/// deserialized as whole structs rather than read field by field, so their
/// `u64` fields take the full 64-bit range whatever the rule says. Each one
/// passes through here as the struct becomes a domain value — `schema`,
/// `pad_len`, `offset`, `size` — which states the bound once for all of them
/// and reports a number past it the way the payload readers already do: as the
/// malformed map it makes, naming the key and the number. Both are the format's
/// own arithmetic and neither says anything about the Library's content.
///
/// `malformed` is what the carrier calls such a map: a meta section and the two
/// control payloads each have their own variant, and the same entry map read
/// out of a different object is that object's refusal rather than one shared
/// spelling for all three.
///
/// The bound is not checked again on the way out, because a writer never has a
/// larger number to write: an entry's `offset` and `size` come out of an
/// `EntryExtent`, which refused anything whose end lies past the bound, and a
/// schema is this build's own constant. What is left is a padding length, which
/// is arithmetic rather than a type, and the encoders hold that one to the same
/// bound where they write it.
pub(crate) fn bounded_uint(key: &str, value: u64, malformed: fn(String) -> Error) -> Result<u64> {
    if value > MAX_FORMAT_INTEGER {
        return Err(malformed(format!(
            "{key} is an unsigned integer below 2^63, found {value}"
        )));
    }
    Ok(value)
}
