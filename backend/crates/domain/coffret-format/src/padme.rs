/// Rounds a plaintext stream length up to its Padmé bucket boundary.
///
/// Padmé (from the PURBs work) rounds an unpadded length `L` up to the next
/// multiple of `2^(E-S)`, where `E = floor(log2 L)` and `S = floor(log2 E) + 1`.
/// A stream short enough that `E - S <= 0` is stored unpadded. Overhead is
/// bounded at about 12% and is typically a few percent.
///
/// Padding this way blunts fingerprinting of known content by its exact stored
/// size, which is one of the few things a storage provider can still observe
/// about an otherwise opaque object.
///
/// Lengths so large that the bucket boundary above them is not representable in
/// a `u64` are returned unpadded; no real stream reaches that size.
pub fn padded_len(unpadded: u64) -> u64 {
    // log2 is undefined at 0, and E would be 0 at 1 — both are below the
    // regime where padding applies.
    if unpadded < 2 {
        return unpadded;
    }
    let e = floor_log2(unpadded);
    let s = floor_log2(u64::from(e)) + 1;
    if e <= s {
        return unpadded;
    }
    let mask = (1u64 << (e - s)) - 1;
    match unpadded.checked_add(mask) {
        Some(rounded) => rounded & !mask,
        None => unpadded,
    }
}

/// `floor(log2(value))`, for `value > 0`.
fn floor_log2(value: u64) -> u32 {
    debug_assert!(value > 0, "log2 is undefined at zero");
    u64::BITS - 1 - value.leading_zeros()
}

#[cfg(test)]
mod tests {
    use super::*;

    // FM-4: Padmé rounds an unpadded length L up to the next multiple of
    // 2^(E-S), with E = floor(log2 L) and S = floor(log2 E) + 1.
    #[test]
    fn pads_to_the_expected_bucket() {
        //                     L,     padded
        let cases: &[(u64, u64)] = &[
            (8, 8),                 // E=3, S=2, bucket 2, already aligned
            (9, 10),                // E=3, S=2, bucket 2
            (100, 104),             // E=6, S=3, bucket 8
            (1_000, 1_024),         // E=9, S=4, bucket 32
            (1_048_576, 1_048_576), // E=20, S=5, bucket 32768, already aligned
            (1_048_577, 1_081_344), // E=20, S=5, bucket 32768
        ];
        for (unpadded, expected) in cases {
            assert_eq!(padded_len(*unpadded), *expected, "L = {unpadded}");
        }
    }

    // FM-4: a stream short enough that E - S <= 0 is stored unpadded.
    #[test]
    fn short_streams_are_unpadded() {
        for unpadded in 0..=7u64 {
            assert_eq!(padded_len(unpadded), unpadded, "L = {unpadded}");
        }
    }

    // FM-4: overhead is bounded at about 12%.
    #[test]
    fn overhead_stays_under_twelve_percent() {
        for unpadded in 1..=100_000u64 {
            let padded = padded_len(unpadded);
            assert!(padded >= unpadded, "L = {unpadded}");
            assert!(
                padded * 100 <= unpadded * 112,
                "L = {unpadded} padded to {padded}"
            );
        }
    }

    #[test]
    fn padding_is_idempotent() {
        for unpadded in 0..=10_000u64 {
            let padded = padded_len(unpadded);
            assert_eq!(padded_len(padded), padded, "L = {unpadded}");
        }
    }
}
