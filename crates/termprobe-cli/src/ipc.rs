//! cli ↔ daemon transport over an `interprocess` local socket.
//! One JSON request line per connection, one JSON response line back.

use std::io::{BufRead, BufReader, Read, Write};
use std::time::{Duration, Instant};

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericFilePath, GenericNamespaced, ListenerOptions};

pub use interprocess::local_socket::Stream;

use crate::protocol::{Request, Response};

pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const IO_POLL_INTERVAL: Duration = Duration::from_millis(5);

fn to_name(raw: &str) -> std::io::Result<interprocess::local_socket::Name<'_>> {
    if cfg!(windows) {
        raw.to_ns_name::<GenericNamespaced>()
    } else {
        raw.to_fs_name::<GenericFilePath>()
    }
}

/// Bound control-plane probes, including probes of an older, blocked daemon.
pub fn send_with_timeout(
    socket: &str,
    req: &Request,
    timeout: Duration,
) -> anyhow::Result<Response> {
    exchange_with_timeout(connect(socket)?, req, timeout)
}

pub fn exchange_with_timeout(
    mut conn: Stream,
    req: &Request,
    timeout: Duration,
) -> anyhow::Result<Response> {
    let deadline = Instant::now() + timeout;
    let mut line = serde_json::to_vec(req)?;
    line.push(b'\n');
    if line.len() > MAX_REQUEST_BYTES {
        anyhow::bail!("request exceeds {MAX_REQUEST_BYTES} bytes");
    }
    conn.set_nonblocking(true)?;
    write_until(&mut conn, &line, deadline)?;
    let mut reader = BufReader::new(conn);
    let line = read_line_until(&mut NonblockingReader(&mut reader), Some(deadline))?;
    Ok(serde_json::from_slice(&line)?)
}

pub fn exchange(conn: Stream, req: &Request) -> anyhow::Result<Response> {
    let mut reader = BufReader::new(conn);
    let mut line = serde_json::to_string(req)?;
    line.push('\n');
    if line.len() > MAX_REQUEST_BYTES {
        anyhow::bail!("request exceeds {MAX_REQUEST_BYTES} bytes");
    }
    reader.get_mut().write_all(line.as_bytes())?;
    reader.get_mut().flush()?;

    let mut response = String::new();
    reader.read_line(&mut response)?;
    let resp: Response = serde_json::from_str(response.trim())?;
    Ok(resp)
}

/// Is a daemon currently accepting connections on this socket?
pub fn is_running(socket: &str) -> bool {
    match to_name(socket) {
        Ok(name) => Stream::connect(name).is_ok(),
