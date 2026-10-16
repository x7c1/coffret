//! Reading a request off a connection, as far as this stub reads one.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;

use super::decoded;

/// One request, as far as this stub reads one.
pub(super) struct Request {
    pub(super) method: String,
    /// The path, percent-decoded: the bucket, then the key where there is one.
    pub(super) path: String,
    /// The query, percent-decoded, by parameter.
    pub(super) query: BTreeMap<String, String>,
    /// The headers, by lowercased name.
    headers: BTreeMap<String, String>,
    pub(super) body: Vec<u8>,
}

impl Request {
    pub(super) fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

/// Reads one request off a connection, or `None` once it has closed.
pub(super) fn read_request(
    reader: &mut BufReader<TcpStream>,
    writer: &mut TcpStream,
) -> io::Result<Option<Request>> {
    let mut line = String::new();
    // A request is preceded by nothing, but a connection the client keeps open
    // may carry a stray blank line between two of them.
    let (method, target) = loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if let Some(parts) = request_line(&line) {
            break parts;
        }
    };

    let mut headers = BTreeMap::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }

    if headers
        .get("expect")
        .is_some_and(|expect| expect.eq_ignore_ascii_case("100-continue"))
    {
        writer.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
        writer.flush()?;
    }

    let mut body = if headers
        .get("transfer-encoding")
        .is_some_and(|encoding| encoding.contains("chunked"))
    {
        read_chunked(reader)?
    } else {
        let len = headers
            .get("content-length")
            .and_then(|len| len.parse::<usize>().ok())
            .unwrap_or(0);
        let mut body = vec![0; len];
        reader.read_exact(&mut body)?;
        body
    };
    // The SDK sends a body with a checksum in aws-chunked framing, which
    // carries the bytes in chunks and the checksum in a trailer after them.
    let aws_chunked = headers
        .get("content-encoding")
        .is_some_and(|encoding| encoding.contains("aws-chunked"))
        || headers
            .get("x-amz-content-sha256")
            .is_some_and(|sha| sha.starts_with("STREAMING-"));
    if aws_chunked {
        body = read_chunked(&mut io::Cursor::new(body))?;
    }

    let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
    Ok(Some(Request {
        method,
        path: decoded(path),
        query: query
            .split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| {
                let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                (decoded(name), decoded(value))
            })
            .collect(),
        headers,
        body,
    }))
}

/// Reads a body framed in chunks, each a hexadecimal length (with whatever
/// extension follows a `;`), the bytes, and a line break, up to the chunk of
/// length zero and the trailer lines after it.
fn read_chunked(reader: &mut impl BufRead) -> io::Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let size = line.trim_end().split(';').next().unwrap_or_default();
        let size = usize::from_str_radix(size.trim(), 16)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if size == 0 {
            break;
        }
        let start = body.len();
        body.resize(start + size, 0);
        reader.read_exact(&mut body[start..])?;
        line.clear();
        reader.read_line(&mut line)?;
    }
    // The trailer, up to the blank line that ends it, or the end of the body.
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line.trim_end().is_empty() {
            return Ok(body);
        }
    }
}

/// The method and target of a request line, where the line is one.
fn request_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    (version.starts_with("HTTP/") && method.chars().all(|c| c.is_ascii_uppercase()))
        .then(|| (method.to_owned(), target.to_owned()))
}
