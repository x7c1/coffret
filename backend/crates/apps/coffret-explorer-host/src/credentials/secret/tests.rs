use super::*;

// LA-20: 32 bytes, 43 characters of unpadded base64url, and what comes back
// from the page digests to what the host kept.
#[test]
fn a_secret_is_43_characters_of_unpadded_base64url() {
    let ticket = LaunchTicket::draw().unwrap();
    let encoded = ticket.encoded();
    assert_eq!(encoded.len(), ENCODED_LEN);
    assert!(encoded.bytes().all(is_base64url));

    let presented = Digest::of_presented(&encoded).expect("the encoding is valid");
    assert!(presented.same_as(&ticket.digest()));
}

// LA-20: length and alphabet are checked before anything is looked up.
#[test]
fn a_presented_credential_of_another_shape_is_refused() {
    let token = BrowserToken::draw().unwrap();
    let encoded = token.encoded();

    let short = &encoded[..ENCODED_LEN - 1];
    let long = format!("{}A", *encoded);
    let padded = format!("{}=", &encoded[..ENCODED_LEN - 1]);
    let standard_alphabet = format!("+{}", &encoded[1..]);
    let not_ascii = format!("é{}", &encoded[2..]);
    // The 43rd character carries 2 bits past the 32nd byte; set, they
    // spell bytes the encoder never writes.
    let trailing_bits = format!("{}B", &encoded[..ENCODED_LEN - 1]);

    for presented in [
        "",
        short,
        &long,
        &padded,
        &standard_alphabet,
        &not_ascii,
        &trailing_bits,
    ] {
        assert!(
            Digest::of_presented(presented).is_none(),
            "{presented:?} must be refused",
        );
    }
}

#[test]
fn digests_compare_equal_only_when_equal() {
    let a = Digest::of(b"one");
    let b = Digest::of(b"one");
    let c = Digest::of(b"two");
    assert!(a.same_as(&b));
    assert!(!a.same_as(&c));
}

// Neither rendering of any secret type, nor of the digest kept of one,
// contains any part of it: not the encoded form the page presents, and
// not the hex of the raw bytes or of the digest.
#[test]
fn no_rendering_of_a_secret_contains_it() {
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
    fn assert_redacted(rendered: &str, secret: &[&str]) {
        for part in secret {
            // Any eight characters in a row is "a part" of it.
            for window in part.as_bytes().windows(8) {
                let window = std::str::from_utf8(window).unwrap();
                assert!(
                    !rendered.contains(window),
                    "{rendered:?} contains part of a secret",
                );
            }
        }
    }

    let ticket = LaunchTicket::draw().unwrap();
    let ticket_parts = [
        ticket.encoded().to_string(),
        hex(&(ticket.0).0),
        hex(&ticket.digest().0),
    ];
    let ticket_parts: Vec<&str> = ticket_parts.iter().map(String::as_str).collect();
    assert_redacted(&format!("{ticket:?}"), &ticket_parts);
    assert_redacted(&format!("{ticket}"), &ticket_parts);
    assert_redacted(&format!("{:?}", ticket.digest()), &ticket_parts);
    assert_redacted(&format!("{}", ticket.digest()), &ticket_parts);

    let token = BrowserToken::draw().unwrap();
    let token_parts = [
        token.encoded().to_string(),
        hex(&(token.0).0),
        hex(&token.digest().0),
    ];
    let token_parts: Vec<&str> = token_parts.iter().map(String::as_str).collect();
    assert_redacted(&format!("{token:?}"), &token_parts);
    assert_redacted(&format!("{token}"), &token_parts);
    assert_redacted(&format!("{:?}", token.digest()), &token_parts);
    assert_redacted(&format!("{}", token.digest()), &token_parts);

    assert_eq!(format!("{ticket:?}"), "LaunchTicket(<redacted>)");
    assert_eq!(format!("{token}"), "<redacted browser token>");
}

/// Accepts only a type that wipes its bytes when it is dropped.
const fn zeroizes_on_drop<T: ZeroizeOnDrop>() {}

#[test]
fn every_secret_type_wipes_itself() {
    zeroizes_on_drop::<LaunchTicket>();
    zeroizes_on_drop::<BrowserToken>();
    zeroizes_on_drop::<Digest>();

    // The operation the drop runs, checked the way `AccountCacheKey`'s is.
    let mut secret = Secret([0xab; SECRET_BYTES]);
    secret.0.zeroize();
    assert_eq!(secret.0, [0_u8; SECRET_BYTES]);
}
