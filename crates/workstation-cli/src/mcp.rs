use serde_json::{json, Value};
use std::{
    io::{BufRead, Write},
    path::Path,
};
use workstation_core::protocol::Protocol;
use workstation_platform::{storage::Store, Error, Result};
const LINE_LIMIT: usize = 128 * 1024;
/// Consume at most one bounded line; no unbounded read_line allocation.
fn read_line(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    loop {
        let part = reader
            .fill_buf()
            .map_err(|_| Error::new("MCP_READ_FAILED"))?;
        if part.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(Error::new("MCP_PARTIAL_MESSAGE"))
            };
        }
        let length = part
            .iter()
            .position(|x| *x == b'\n')
            .map(|x| x + 1)
            .unwrap_or(part.len());
        if bytes.len() + length > LINE_LIMIT {
            return Err(Error::new("MCP_INPUT_LIMIT"));
        }
        let done = part[length - 1] == b'\n';
        bytes.extend_from_slice(&part[..length]);
        reader.consume(length);
        if done {
            return Ok(Some(bytes));
        }
    }
}
pub fn serve(home: &Path, project: &str, environment: Option<&str>) -> Result<()> {
    workstation_core::control::id(project).map_err(Error::new)?;
    let store = Store::open_readonly(home)?;
    store.require_control()?;
    // Fail closed on absent/misspelled scope before accepting requests.
    store.work_items(project)?;
    if let Some(env) = environment {
        store.resources(project, env)?;
    }
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let mut protocol = Protocol::default();
    while let Some(line) = read_line(&mut input)? {
        let reply = match serde_json::from_slice::<Value>(&line) {
            Ok(v) => protocol.handle(v, |name, args| {
                store
                    .read_tool(project, environment, name, args)
                    .map_err(|e| e.code)
            }),
            Err(_) => Some(
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}),
            ),
        };
        if let Some(reply) = reply {
            serde_json::to_writer(&mut output, &reply)
                .map_err(|_| Error::new("MCP_OUTPUT_FAILED"))?;
            output
                .write_all(b"\n")
                .and_then(|_| output.flush())
                .map_err(|_| Error::new("MCP_OUTPUT_FAILED"))?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_line_rejected() {
        let v = vec![b'x'; LINE_LIMIT + 1];
        assert!(read_line(&mut std::io::Cursor::new(v)).is_err());
    }
    #[test]
    fn incomplete_frame_rejected() {
        assert!(read_line(&mut std::io::Cursor::new(b"{}".to_vec())).is_err());
    }
    #[test]
    fn consumes_one_frame() {
        let mut input = std::io::Cursor::new(b"{}\n{}\n".to_vec());
        assert_eq!(read_line(&mut input).unwrap().unwrap(), b"{}\n");
        assert!(read_line(&mut input).unwrap().is_some());
    }
}
