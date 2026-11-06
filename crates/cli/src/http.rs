//! A minimal HTTP/1.1 JSON-RPC POST for `tecpan doctor` (`node_getNodeInfo`); plain `http://` only, which is what a
//! local Aztec node serves. Everything else goes through the daemon.

use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use serde_json::{Value, json};

/// `http://host[:port][/path]` → (host, port, path).
pub(crate) fn parse_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "only http:// URLs can be checked here".to_owned())?;
    let (authority, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_owned()), |(a, p)| (a, format!("/{p}")));
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host,
            port.parse().map_err(|_| format!("bad port in {url}"))?,
        ),
        None => (authority, 80),
    };
    if host.is_empty() {
        return Err(format!("no host in {url}"));
    }
    Ok((host.to_owned(), port, path))
}

/// Calls `method` with no params and returns its `result`.
pub(crate) fn json_rpc(url: &str, method: &str, timeout: Duration) -> Result<Value, String> {
    let (host, port, path) = parse_url(url)?;
    let address = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| format!("{host} does not resolve"))?;
    let mut stream = TcpStream::connect_timeout(&address, timeout).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": [] }).to_string();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|e| e.to_string())?;
    parse_response(&response)
}

fn parse_response(response: &[u8]) -> Result<Value, String> {
    let text = String::from_utf8_lossy(response);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or("malformed HTTP response")?;
    let status = head.lines().next().unwrap_or_default();
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(format!("HTTP {status}"));
    }
    let chunked = head.lines().any(|l| {
        l.to_ascii_lowercase().starts_with("transfer-encoding:")
            && l.to_ascii_lowercase().contains("chunked")
    });
    let body = if chunked {
        dechunk(body)
    } else {
        body.to_owned()
    };
    let value: Value = serde_json::from_str(&body).map_err(|e| format!("not JSON-RPC: {e}"))?;
    if let Some(error) = value.get("error") {
        return Err(error.to_string());
    }
    Ok(value.get("result").cloned().unwrap_or(Value::Null))
}

fn dechunk(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while let Some((size, after)) = rest.split_once("\r\n") {
        let Ok(size) = usize::from_str_radix(size.trim(), 16) else {
            break;
        };
        if size == 0 || after.len() < size {
            break;
        }
        out.push_str(&after[..size]);
        rest = after[size..].trim_start_matches("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(
            parse_url("http://localhost:8080").unwrap(),
            ("localhost".into(), 8080, "/".into())
        );
        assert_eq!(
            parse_url("http://node/rpc").unwrap(),
            ("node".into(), 80, "/rpc".into())
        );
        assert!(parse_url("https://node").is_err());
    }

    #[test]
    fn responses() {
        let plain = b"HTTP/1.1 200 OK\r\nContent-Length: 40\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"a\":1}}";
        assert_eq!(parse_response(plain).unwrap(), json!({ "a": 1 }));
        let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\n{\"res\r\n14\r\nult\":2,\"id\":1,\"x\":0}\r\n0\r\n\r\n";
        assert_eq!(parse_response(chunked).unwrap(), json!(2));
        assert!(parse_response(b"HTTP/1.1 404 Not Found\r\n\r\n").is_err());
    }
}
