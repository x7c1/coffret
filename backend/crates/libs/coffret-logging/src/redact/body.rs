use super::text::text;

/// Reads a response body into something an event may carry.
pub fn body(bytes: &[u8]) -> String {
    text(&String::from_utf8_lossy(bytes))
}

#[cfg(test)]
mod tests {
    use super::super::text::ELIDED;
    use super::super::MAX_BODY_BYTES;
    use super::*;

    #[test]
    fn a_refusal_from_the_token_endpoint_is_kept_as_it_arrived() {
        let refusal = br#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#;

        assert_eq!(
            body(refusal),
            r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#,
        );
    }

    #[test]
    fn a_token_in_a_json_answer_never_reaches_the_event() {
        let answer = br#"{"access_token":"ya29.a0-secret","expires_in":3599,"refresh_token":"1//0e-secret"}"#;
        let recorded = body(answer);

        assert!(!recorded.contains("ya29.a0-secret"), "{recorded}");
        assert!(!recorded.contains("1//0e-secret"), "{recorded}");
        // What is left still says what the answer was made of.
        assert!(recorded.contains("expires_in"), "{recorded}");
        assert!(recorded.contains("3599"), "{recorded}");
    }

    #[test]
    fn a_token_in_a_form_never_reaches_the_event() {
        let form = b"grant_type=refresh_token&refresh_token=1//0e-secret&client_id=an-app";
        let recorded = body(form);

        assert!(!recorded.contains("1//0e-secret"), "{recorded}");
        assert!(recorded.contains("client_id=an-app"), "{recorded}");
    }

    #[test]
    fn a_quoted_authorization_header_never_reaches_the_event() {
        let quoted = br#"{"error":{"message":"Invalid Credentials for Bearer ya29.a0-secret"}}"#;
        let recorded = body(quoted);

        assert!(!recorded.contains("ya29.a0-secret"), "{recorded}");
        assert!(recorded.contains("Invalid Credentials"), "{recorded}");
    }

    #[test]
    fn a_field_merely_named_in_a_message_is_left_alone() {
        let message = br#"{"error_description":"the refresh_token is not valid"}"#;

        assert_eq!(
            body(message),
            r#"{"error_description":"the refresh_token is not valid"}"#,
        );
    }

    #[test]
    fn a_body_longer_than_one_event_may_carry_is_cut_short() {
        let long = vec![b'x'; MAX_BODY_BYTES * 4];
        let recorded = body(&long);

        assert_eq!(recorded.len(), MAX_BODY_BYTES);
        assert!(recorded.ends_with(ELIDED));
    }

    #[test]
    fn a_body_written_over_several_lines_becomes_one_event() {
        let xml = b"<?xml version=\"1.0\"?>\n<Error><Code>AccessDenied</Code></Error>";
        let recorded = body(xml);

        assert!(!recorded.contains('\n'), "{recorded}");
        assert!(recorded.contains("AccessDenied"), "{recorded}");
    }

    #[test]
    fn a_body_that_is_not_text_is_still_recorded() {
        let bytes = [0xff, 0xfe, b'o', b'k'];

        assert!(body(&bytes).contains("ok"));
    }
}
