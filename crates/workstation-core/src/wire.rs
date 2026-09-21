//! Bounded Content-Length framing used by the reviewed Copilot SDK protocol.
//! This is not MCP framing, ACP, a network listener, or a general JSON-RPC API.
use crate::control::Check;
use serde_json::Value;
pub const HEADER_LIMIT: usize = 4096;
pub const FRAME_LIMIT: usize = 256 * 1024;
pub fn encode(v: &Value) -> Check<Vec<u8>> {
    let b = serde_json::to_vec(v).map_err(|_| "WIRE_JSON_ENCODING")?;
    if b.len() > FRAME_LIMIT {
        return Err("WIRE_FRAME_LIMIT");
    }
    let mut out = format!("Content-Length: {}\r\n\r\n", b.len()).into_bytes();
    out.extend_from_slice(&b);
    Ok(out)
}
pub fn decode(bytes: &[u8]) -> Check<Option<(usize, Value)>> {
    let end = bytes.windows(4).position(|v| v == b"\r\n\r\n");
    let Some(end) = end else {
        if bytes.len() > HEADER_LIMIT {
            return Err("WIRE_HEADER_LIMIT");
        }
        return Ok(None);
    };
    if end > HEADER_LIMIT {
        return Err("WIRE_HEADER_LIMIT");
    }
    let header = std::str::from_utf8(&bytes[..end]).map_err(|_| "WIRE_HEADER_ENCODING")?;
    let mut len = None;
    for line in header.split("\r\n") {
        let (k, v) = line.split_once(':').ok_or("WIRE_HEADER_INVALID")?;
        if k.eq_ignore_ascii_case("Content-Length") {
            if len.is_some() || v.trim().is_empty() || !v.trim().bytes().all(|c| c.is_ascii_digit())
            {
                return Err("WIRE_LENGTH_INVALID");
            }
            len = Some(
                v.trim()
                    .parse::<usize>()
                    .map_err(|_| "WIRE_LENGTH_INVALID")?,
            );
        } else if !k.eq_ignore_ascii_case("Content-Type") {
            return Err("WIRE_HEADER_UNSUPPORTED");
        }
    }
    let n = len.ok_or("WIRE_LENGTH_MISSING")?;
    if n == 0 || n > FRAME_LIMIT {
        return Err("WIRE_FRAME_LIMIT");
    }
    let total = end
        .checked_add(4)
        .and_then(|v| v.checked_add(n))
        .ok_or("WIRE_LENGTH_INVALID")?;
    if bytes.len() < total {
        return Ok(None);
    }
    let v: Value =
        serde_json::from_slice(&bytes[end + 4..total]).map_err(|_| "WIRE_JSON_INVALID")?;
    if !v.is_object() || v.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err("WIRE_NOT_JSONRPC2");
    }
    Ok(Some((total, v)))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn frame_roundtrip() {
        let v = json!({"jsonrpc":"2.0","id":1,"result":{"ok":true}});
        let b = encode(&v).unwrap();
        assert_eq!(decode(&b).unwrap(), Some((b.len(), v)));
    }
    #[test]
    fn split_frame_waits() {
        let b = encode(&json!({"jsonrpc":"2.0","id":1})).unwrap();
        assert!(decode(&b[..b.len() - 1]).unwrap().is_none());
    }
    #[test]
    fn duplicate_length_denied() {
        assert!(decode(b"Content-Length: 2\r\ncontent-length: 2\r\n\r\n{}").is_err());
    }
    #[test]
    fn oversized_declared_length_denied() {
        assert!(decode(b"Content-Length: 999999999\r\n\r\n").is_err());
    }
    #[test]
    fn ndjson_not_silently_accepted() {
        assert!(decode(&vec![b'x'; HEADER_LIMIT + 1]).is_err());
    }
    #[test]
    fn frame_does_not_consume_next() {
        let a = encode(&json!({"jsonrpc":"2.0","id":1})).unwrap();
        let mut b = a.clone();
        b.extend_from_slice(&a);
        assert_eq!(decode(&b).unwrap().unwrap().0, a.len());
    }
}
