//! Synchronous JSON-RPC transport over an App Server stdio process.

use std::collections::VecDeque;
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, ChildStdout};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use serde_json::{Value, json};

pub(super) const MAX_PROTOCOL_LINE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub(super) enum ReaderEvent {
    Message(Value),
    InvalidJson,
    LineTooLong,
    IoFailure,
    Eof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RpcFailure {
    detail: String,
}

impl RpcFailure {
    pub(super) fn protocol(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    pub(super) fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for RpcFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

/// Starts the sole stdout reader. It bounds each line before JSON parsing and
/// never waits for the child process, leaving exit observation to the
/// independent supervisor.
pub(super) fn spawn_reader(stdout: ChildStdout) -> Receiver<ReaderEvent> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_bounded_line(&mut reader) {
                Ok(Some(line)) if line.iter().all(u8::is_ascii_whitespace) => {}
                Ok(Some(line)) => match serde_json::from_slice(&line) {
                    Ok(value) => {
                        if sender.send(ReaderEvent::Message(value)).is_err() {
                            return;
                        }
                    }
                    Err(_) => {
                        let _ = sender.send(ReaderEvent::InvalidJson);
                        let _ = sender.send(ReaderEvent::Eof);
                        return;
                    }
                },
                Ok(None) => {
                    let _ = sender.send(ReaderEvent::Eof);
                    return;
                }
                Err(ReadLineFailure::TooLong) => {
                    let _ = sender.send(ReaderEvent::LineTooLong);
                    let _ = sender.send(ReaderEvent::Eof);
                    return;
                }
                Err(ReadLineFailure::Io) => {
                    let _ = sender.send(ReaderEvent::IoFailure);
                    let _ = sender.send(ReaderEvent::Eof);
                    return;
                }
            }
        }
    });
    receiver
}

#[derive(Debug)]
enum ReadLineFailure {
    TooLong,
    Io,
}

fn read_bounded_line<R: BufRead>(reader: &mut R) -> Result<Option<Vec<u8>>, ReadLineFailure> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().map_err(|_| ReadLineFailure::Io)?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }
        if let Some(newline) = available.iter().position(|byte| *byte == b'\n') {
            if line.len().saturating_add(newline) > MAX_PROTOCOL_LINE_BYTES {
                reader.consume(newline + 1);
                return Err(ReadLineFailure::TooLong);
            }
            line.extend_from_slice(&available[..newline]);
            reader.consume(newline + 1);
            return Ok(Some(line));
        }
        if line.len().saturating_add(available.len()) > MAX_PROTOCOL_LINE_BYTES {
            let consumed = available.len();
            reader.consume(consumed);
            return Err(ReadLineFailure::TooLong);
        }
        line.extend_from_slice(available);
        let consumed = available.len();
        reader.consume(consumed);
    }
}

pub(super) type SharedInput = Arc<Mutex<Option<ChildStdin>>>;

/// One sequential JSON-RPC connection. Request IDs are correlated even when
/// notifications or server requests arrive before the matching response.
pub(super) struct RpcProcess {
    input: SharedInput,
    output: Receiver<ReaderEvent>,
    pending: VecDeque<Value>,
    next_id: u64,
    read_only: bool,
}

impl RpcProcess {
    pub(super) fn new(input: SharedInput, output: Receiver<ReaderEvent>, read_only: bool) -> Self {
        Self {
            input,
            output,
            pending: VecDeque::new(),
            next_id: 1,
            read_only,
        }
    }

    fn send(&self, value: &Value) -> Result<(), RpcFailure> {
        let mut bytes = serde_json::to_vec(value)
            .map_err(|_| RpcFailure::protocol("cannot encode an App Server request"))?;
        bytes.push(b'\n');
        let mut input = self
            .input
            .lock()
            .map_err(|_| RpcFailure::protocol("App Server stdin lock is poisoned"))?;
        let input = input
            .as_mut()
            .ok_or_else(|| RpcFailure::protocol("App Server stdin is closed"))?;
        input
            .write_all(&bytes)
            .and_then(|()| input.flush())
            .map_err(|_| RpcFailure::protocol("cannot write an App Server request"))
    }

    pub(super) fn notify(&self, method: &str, params: Value) -> Result<(), RpcFailure> {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    pub(super) fn request(
        &mut self,
        method: &str,
        params: Value,
        deadline: Instant,
    ) -> Result<Value, RpcFailure> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| RpcFailure::protocol("App Server request ID overflow"))?;
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        }))?;
        loop {
            let event = self.receive(deadline)?;
            let ReaderEvent::Message(message) = event else {
                return Err(reader_failure(event));
            };
            if self.respond_server(&message)? {
                continue;
            }
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    let code = error
                        .get("code")
                        .and_then(Value::as_i64)
                        .map_or_else(|| "unreported".to_owned(), |code| code.to_string());
                    return Err(RpcFailure::protocol(format!(
                        "App Server request '{method}' failed with JSON-RPC code {code}"
                    )));
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            // Preserve notifications and unrelated responses for the event
            // consumer instead of dropping them while this request waits.
            self.pending.push_back(message);
        }
    }

    fn receive(&self, deadline: Instant) -> Result<ReaderEvent, RpcFailure> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(RpcFailure::protocol("App Server request timed out"));
        }
        match self.output.recv_timeout(remaining) {
            Ok(event) => Ok(event),
            Err(RecvTimeoutError::Timeout) => {
                Err(RpcFailure::protocol("App Server request timed out"))
            }
            Err(RecvTimeoutError::Disconnected) => Err(RpcFailure::protocol(
                "App Server output closed before the response",
            )),
        }
    }

    pub(super) fn next(&mut self) -> ReaderEvent {
        if let Some(message) = self.pending.pop_front() {
            ReaderEvent::Message(message)
        } else {
            self.output.recv().unwrap_or(ReaderEvent::Eof)
        }
    }

    pub(super) fn respond_server(&self, message: &Value) -> Result<bool, RpcFailure> {
        if message.get("id").is_none() || message.get("method").is_none() {
            return Ok(false);
        }
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let result = match method {
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
                json!({"decision": if self.read_only { "decline" } else { "accept" }})
            }
            _ => {
                self.send(&json!({
                    "jsonrpc": "2.0",
                    "id": message["id"],
                    "error": {"code": -32601, "message": "Unsupported client request"}
                }))?;
                return Ok(true);
            }
        };
        self.send(&json!({"jsonrpc": "2.0", "id": message["id"], "result": result}))?;
        Ok(true)
    }

    pub(super) fn close_input(&self) {
        if let Ok(mut input) = self.input.lock() {
            input.take();
        }
    }
}

fn reader_failure(event: ReaderEvent) -> RpcFailure {
    let detail = match event {
        ReaderEvent::InvalidJson => "App Server returned invalid JSON",
        ReaderEvent::LineTooLong => "App Server event exceeds 16 MiB",
        ReaderEvent::IoFailure => "cannot read App Server output",
        ReaderEvent::Eof => "App Server exited before completing the request",
        ReaderEvent::Message(_) => "unexpected App Server reader state",
    };
    RpcFailure::protocol(detail)
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use super::{MAX_PROTOCOL_LINE_BYTES, ReadLineFailure, read_bounded_line};

    #[test]
    fn line_reader_accepts_boundary_and_rejects_one_byte_more() {
        let mut boundary = vec![b'x'; MAX_PROTOCOL_LINE_BYTES];
        boundary.push(b'\n');
        assert_eq!(
            read_bounded_line(&mut BufReader::new(Cursor::new(boundary)))
                .unwrap()
                .unwrap()
                .len(),
            MAX_PROTOCOL_LINE_BYTES
        );

        let mut over = vec![b'x'; MAX_PROTOCOL_LINE_BYTES + 1];
        over.push(b'\n');
        assert!(matches!(
            read_bounded_line(&mut BufReader::new(Cursor::new(over))),
            Err(ReadLineFailure::TooLong)
        ));
    }
}
