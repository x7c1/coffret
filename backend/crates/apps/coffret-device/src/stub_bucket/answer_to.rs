//! What the bucket answers a request with.

use super::request::Request;
use super::{decoded, objects};

/// What the bucket answers `request` with.
pub(super) fn answer_to(request: &Request) -> Vec<u8> {
    let path = request.path.trim_start_matches('/');
    let (bucket, key) = path.split_once('/').unwrap_or((path, ""));
    if key.is_empty() {
        return match request.method.as_str() {
            "GET" if request.query.contains_key("list-type") => {
                let prefix = request.query.get("prefix").map_or("", String::as_str);
                listing(bucket, prefix)
            }
            _ => answer("200 OK", &[], b""),
        };
    }

    let at = format!("{bucket}/{key}");
    let mut objects = objects()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match request.method.as_str() {
        "PUT" => {
            if let Some(source) = request.header("x-amz-copy-source") {
                let source = decoded(source);
                let Some(bytes) = objects.get(source.trim_start_matches('/')).cloned() else {
                    return missing();
                };
                objects.insert(at, bytes);
                let body = concat!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
                    r#"<CopyObjectResult><ETag>"stub"</ETag></CopyObjectResult>"#,
                );
                return answer("200 OK", &[], body.as_bytes());
            }
            if request.header("if-none-match") == Some("*") && objects.contains_key(&at) {
                let body = concat!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
                    "<Error><Code>PreconditionFailed</Code>",
                    "<Message>At least one of the preconditions you specified did not hold",
                    "</Message></Error>",
                );
                return answer("412 Precondition Failed", &[], body.as_bytes());
            }
            let tag = etag(request.body.len());
            objects.insert(at, request.body.clone());
            answer("200 OK", &[("ETag", &tag)], b"")
        }
        "GET" | "HEAD" => {
            let Some(bytes) = objects.get(&at) else {
                return missing();
            };
            let tag = etag(bytes.len());
            let (status, bytes) = match request.header("range").and_then(range) {
                Some((start, end)) => {
                    let end = end.min(bytes.len().saturating_sub(1));
                    ("206 Partial Content", bytes.get(start..=end).unwrap_or(&[]))
                }
                None => ("200 OK", bytes.as_slice()),
            };
            if request.method == "HEAD" {
                return format!(
                    "HTTP/1.1 {status}\r\nETag: {tag}\r\ncontent-length: {}\r\n\r\n",
                    bytes.len()
                )
                .into_bytes();
            }
            answer(status, &[("ETag", &tag)], bytes)
        }
        "DELETE" => {
            objects.remove(&at);
            answer("204 No Content", &[], b"")
        }
        _ => answer("405 Method Not Allowed", &[], b""),
    }
}

/// A `ListObjectsV2` page of every key directly under `prefix`: the listing a
/// delimiter of `/` asks for, whole, in one page.
fn listing(bucket: &str, prefix: &str) -> Vec<u8> {
    let under = format!("{bucket}/{prefix}");
    let objects = objects()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let contents: String = objects
        .range(under.clone()..)
        .take_while(|(at, _)| at.starts_with(&under))
        .filter(|(at, _)| !at[under.len()..].contains('/'))
        .map(|(at, bytes)| {
            format!(
                "<Contents><Key>{}</Key><ETag>{}</ETag><Size>{}</Size></Contents>",
                &at[bucket.len() + 1..],
                etag(bytes.len()),
                bytes.len(),
            )
        })
        .collect();
    let body = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#,
            "<Name>{}</Name><Prefix>{}</Prefix><IsTruncated>false</IsTruncated>",
            "{}</ListBucketResult>",
        ),
        bucket, prefix, contents,
    );
    answer("200 OK", &[], body.as_bytes())
}

/// What a read of a key nothing is stored under is answered with.
fn missing() -> Vec<u8> {
    let body = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        "<Error><Code>NoSuchKey</Code><Message>The specified key does not exist.</Message>",
        "</Error>",
    );
    answer("404 Not Found", &[], body.as_bytes())
}

/// An answer with a status, some headers, and a body.
fn answer(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut head = format!("HTTP/1.1 {status}\r\n");
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!("content-length: {}\r\n\r\n", body.len()));
    let mut answer = head.into_bytes();
    answer.extend_from_slice(body);
    answer
}

/// The entity tag an object is answered with: quoted, as S3 quotes one, and
/// standing for nothing but the object's length.
fn etag(len: usize) -> String {
    format!("\"{len:032x}\"")
}

/// The first and last byte a `Range` header asks for, both inclusive.
fn range(header: &str) -> Option<(usize, usize)> {
    let (start, end) = header.strip_prefix("bytes=")?.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?))
}
