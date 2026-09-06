// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Minimal QEMU Machine Protocol (QMP) client for runtime control.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QmpError {
    #[error("QMP I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid QMP JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("QMP server did not send a greeting")]
    MissingGreeting,

    #[error("QMP command failed: {0}")]
    Command(String),

    #[error("unexpected QMP response: {0}")]
    Protocol(String),
}

pub type Result<T> = std::result::Result<T, QmpError>;

pub struct QmpClient {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl QmpClient {
    pub fn connect(path: &Path) -> Result<Self> {
        Self::connect_timeout(path, Duration::from_secs(5))
    }

    pub fn connect_timeout(path: &Path, timeout: Duration) -> Result<Self> {
        let writer = UnixStream::connect(path)?;
        writer.set_read_timeout(Some(timeout))?;
        writer.set_write_timeout(Some(timeout))?;
        let reader_stream = writer.try_clone()?;
        reader_stream.set_read_timeout(Some(timeout))?;
        let mut client = Self {
            reader: BufReader::new(reader_stream),
            writer,
        };

        let greeting = client.read_message()?;
        if greeting.get("QMP").is_none() {
            return Err(QmpError::MissingGreeting);
        }
        client.execute("qmp_capabilities", None)?;
        Ok(client)
    }

    pub fn execute(&mut self, command: &str, arguments: Option<Value>) -> Result<Value> {
        let mut request = json!({ "execute": command });
        if let Some(arguments) = arguments {
            request["arguments"] = arguments;
        }
        serde_json::to_writer(&mut self.writer, &request)?;
        self.writer.write_all(b"\r\n")?;
        self.writer.flush()?;

        loop {
            let response = self.read_message()?;
            if response.get("event").is_some() {
                continue;
            }
            if let Some(value) = response.get("return") {
                return Ok(value.clone());
            }
            if let Some(error) = response.get("error") {
                let class = error
                    .get("class")
                    .and_then(Value::as_str)
                    .unwrap_or("QmpError");
                let description = error
                    .get("desc")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error");
                return Err(QmpError::Command(format!("{class}: {description}")));
            }
            return Err(QmpError::Protocol(response.to_string()));
        }
    }

    pub fn query_status(&mut self) -> Result<String> {
        let value = self.execute("query-status", None)?;
        value
            .get("status")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| QmpError::Protocol(value.to_string()))
    }

    pub fn pause(&mut self) -> Result<()> {
        self.execute("stop", None).map(|_| ())
    }

    pub fn resume(&mut self) -> Result<()> {
        self.execute("cont", None).map(|_| ())
    }

    pub fn powerdown(&mut self) -> Result<()> {
        self.execute("system_powerdown", None).map(|_| ())
    }

    pub fn quit(&mut self) -> Result<()> {
        self.execute("quit", None).map(|_| ())
    }

    pub fn balloon(&mut self, bytes: u64) -> Result<()> {
        self.execute("balloon", Some(json!({ "value": bytes })))
            .map(|_| ())
    }

    fn read_message(&mut self) -> Result<Value> {
        loop {
            let mut line = String::new();
            let read = self.reader.read_line(&mut line)?;
            if read == 0 {
                return Err(QmpError::Protocol("QMP socket closed".into()));
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            return Ok(serde_json::from_str(line)?);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::thread;

    fn read_json(reader: &mut BufReader<UnixStream>) -> Value {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        serde_json::from_str(line.trim()).unwrap()
    }

    #[test]
    fn negotiates_capabilities_and_queries_status() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("qmp.sock");
        let listener = UnixListener::bind(&socket).unwrap();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .write_all(
                    br#"{"QMP":{"version":{"qemu":{"major":9,"minor":0,"micro":0},"package":""},"capabilities":[]}}
"#,
                )
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());

            let capability = read_json(&mut reader);
            assert_eq!(capability["execute"], "qmp_capabilities");
            stream.write_all(b"{\"return\":{}}\r\n").unwrap();

            let query = read_json(&mut reader);
            assert_eq!(query["execute"], "query-status");
            stream
                .write_all(b"{\"event\":\"RESUME\",\"data\":{}}\r\n")
                .unwrap();
            stream
                .write_all(b"{\"return\":{\"status\":\"running\"}}\r\n")
                .unwrap();
        });

        let mut client = QmpClient::connect(&socket).unwrap();
        assert_eq!(client.query_status().unwrap(), "running");
        server.join().unwrap();
    }
}
