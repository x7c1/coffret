use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;

/// The browser token in `Authorization: Bearer <token>`, given once
/// (spec: LA-14).
///
/// `None` for a header that is missing, given more than once, or not of that
/// shape. What follows the scheme is handed over as it is: whether it is a
/// token at all is the credentials' to say, as one answer with a token that is
/// not held (spec: LA-20).
pub(crate) fn bearer(headers: &HeaderMap) -> Option<&str> {
    credential(headers, "Bearer")
}

/// The launch ticket in `Authorization: Launch <ticket>`, given once
/// (spec: LA-16).
pub(crate) fn launch_ticket(headers: &HeaderMap) -> Option<&str> {
    credential(headers, "Launch")
}

fn credential<'a>(headers: &'a HeaderMap, scheme: &str) -> Option<&'a str> {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    let (given, credential) = value.to_str().ok()?.split_once(' ')?;
    given.eq_ignore_ascii_case(scheme).then_some(credential)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(values: &[&'static str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append(AUTHORIZATION, HeaderValue::from_static(value));
        }
        headers
    }

    // LA-14: one header, of the scheme asked for, and nothing else.
    #[test]
    fn a_credential_is_one_header_of_its_own_scheme() {
        assert_eq!(bearer(&headers(&["Bearer abc"])), Some("abc"));
        assert_eq!(bearer(&headers(&["bearer abc"])), Some("abc"));
        assert_eq!(launch_ticket(&headers(&["Launch abc"])), Some("abc"));

        assert_eq!(bearer(&headers(&[])), None);
        assert_eq!(bearer(&headers(&["Bearer abc", "Bearer abc"])), None);
        assert_eq!(bearer(&headers(&["Launch abc"])), None);
        assert_eq!(launch_ticket(&headers(&["Bearer abc"])), None);
        assert_eq!(bearer(&headers(&["Bearerabc"])), None);
        assert_eq!(bearer(&headers(&["Basic YTpi"])), None);
    }
}
