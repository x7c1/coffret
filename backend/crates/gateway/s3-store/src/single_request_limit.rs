use coffret_usecase::{Error, Result};

/// The most bytes S3 accepts in one `PutObject`.
///
/// S3 publishes the cap as "5 GB", without saying whether that is the decimal
/// or the binary figure. The decimal one is the smaller, and erring low costs
/// nothing: the difference is 74 MiB at the very top of a range no normal
/// Pack reaches.
pub const SINGLE_REQUEST_MAX_BYTES: u64 = 5_000_000_000;

/// Refuses a body this adapter cannot send as one request.
///
/// Every write here goes out as a single `PutObject`, so the cap above is the
/// adapter's ceiling as much as S3's. The pack policy's size target keeps
/// normal Packs far below it — 1 GiB and 2 GiB are its candidates (spec:
/// PK-5) — but an Entry larger than the target forms an oversized singleton
/// Pack (spec: PK-3), so one very large file, a raw video among them, reaches
/// the ceiling on its own.
///
/// This is therefore a limitation of the adapter today rather than a rule about
/// what a Library may hold: multipart upload is what lifts it, and until that
/// lands the honest thing is to refuse early. The length travels with the
/// [`ByteStream`](coffret_usecase::ByteStream), so the refusal costs nothing —
/// the alternative is streaming several gigabytes to S3 and being told at the
/// end.
pub fn refuse_oversized(len: u64) -> Result<()> {
    if len <= SINGLE_REQUEST_MAX_BYTES {
        return Ok(());
    }
    Err(Error::Unsupported {
        detail: format!(
            "{len} bytes is past the {SINGLE_REQUEST_MAX_BYTES} this store sends in one request; \
             an object this large needs a multipart upload, which it does not do yet"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_object_at_the_cap_is_one_request_worth_of_bytes() {
        assert!(refuse_oversized(SINGLE_REQUEST_MAX_BYTES).is_ok());
        assert!(refuse_oversized(0).is_ok());
    }

    #[test]
    fn a_single_byte_past_the_cap_is_refused_before_anything_is_sent() {
        let error = refuse_oversized(SINGLE_REQUEST_MAX_BYTES + 1)
            .expect_err("one byte past the cap is past the cap");

        let Error::Unsupported { detail } = &error else {
            panic!("an object too large for one request is a request this store cannot serve: {error:?}");
        };
        // Naming multipart is the point: the refusal has to say what would
        // carry the object, or it reads as a rule about the object itself.
        assert!(detail.contains("multipart"), "{detail}");
    }
}
