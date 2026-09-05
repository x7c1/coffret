/// The largest unsigned integer the format admits (spec: FM-19).
///
/// Every unsigned integer format v1 carries in 64 bits — a control header's
/// generation, and every CBOR unsigned integer of a meta section or a control
/// payload — is below 2^63, so this is `2^63 - 1`. The bound is what lets an
/// implementation hold what the format says in the widest integer many hosts
/// have, a signed 64-bit one, without reinterpreting a sign; nothing a Library
/// counts comes near it, so the range above it buys nothing.
///
/// The types that carry format integers refuse anything past this rather than
/// each spelling the number: a generation, a Master Key epoch, a ciphertext
/// length claim, and the end of an entry's extent all state the same bound by
/// naming this constant.
pub const MAX_FORMAT_INTEGER: u64 = (1u64 << 63) - 1;

#[cfg(test)]
mod tests {
    use super::*;

    // FM-19: the bound is 2^63, so the largest admitted integer is one below
    // it — and it is exactly what a signed 64-bit integer can hold.
    #[test]
    fn the_largest_admitted_integer_is_one_below_two_to_the_sixty_third() {
        assert_eq!(MAX_FORMAT_INTEGER, 9_223_372_036_854_775_807);
        assert_eq!(i64::MAX.unsigned_abs(), MAX_FORMAT_INTEGER);
    }
}
