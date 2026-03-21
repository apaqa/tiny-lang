//! tiny-lang 最小 LSP 伺服器。

use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};

use serde_json::{Value, json};

use crate::error::{ErrorKind, TinyLangError};
use crate::typechecker::TypeChecker;

/// 中文註解：啟動最小 JSON-RPC / LSP 事件迴圈。
pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = LspServer::new(stdin.lock(), stdout.lock());
    server.run()
}

struct LspServer<R: BufRead, W: Write> {
    reader: R,
    writer: W,
    documents: HashMap<String, String>,
    shutting_down: bool,
}

impl<R: BufRead, W: Write> LspServer<R, W> {
    fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            documents: HashMap::new(),
            shutting_down: false,
        }
    }

    fn run(&mut self) -> io::Result<()> {
        while let Some(message) = self.read_message()? {
            self.handle_message(message)?;
            if self.shutting_down {
                break;
            }
        }
        Ok(())
    }

    fn handle_message(&mut self, message: Value) -> io::Result<()> {
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        match method {
            "initialize" => {
                let id = message.get("id").cloned().unwrap_or(Value::Null);
                let result = json!({
                    "capabilities": {
                        "textDocumentSync": 2
                    },
                    "serverInfo": {
                        "name": "tiny-lsp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                });
                self.write_message(&json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": result
                }))?;
            }
            "initialized" => {}
            "shutdown" => {
                self.shutting_down = true;
                let id = message.get("id").cloned().unwrap_or(Value::Null);
                self.write_message(&json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": Value::Null
                }))?;
            }
            "exit" => {
                self.shutting_down = true;
            }
            "textDocument/didOpen" => {
                let doc = &message["params"]["textDocument"];
                let uri = doc["uri"].as_str().unwrap_or("").to_string();
                let text = doc["text"].as_str().unwrap_or("").to_string();
                self.documents.insert(uri.clone(), text.clone());
                self.publish_diagnostics(&uri, &text)?;
            }
            "textDocument/didChange" => {
                let uri = message["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let text = message["params"]["contentChanges"]
                    .as_array()
                    .and_then(|changes| changes.last())
                    .and_then(|change| change.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                self.documents.insert(uri.clone(), text.clone());
                self.publish_diagnostics(&uri, &text)?;
            }
            _ => {
                if message.get("id").is_some() {
                    self.write_message(&json!({
                        "jsonrpc": "2.0",
                        "id": message["id"].clone(),
                        "result": Value::Null
                    }))?;
                }
            }
        }
        Ok(())
    }

    fn publish_diagnostics(&mut self, uri: &str, source: &str) -> io::Result<()> {
        let diagnostics = collect_diagnostics(source);
        self.write_message(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": diagnostics
            }
        }))
    }

    fn read_message(&mut self) -> io::Result<Option<Value>> {
        let mut content_length = None;
        let mut line = String::new();

        loop {
            line.clear();
            let bytes = self.reader.read_line(&mut line)?;
            if bytes == 0 {
                return Ok(None);
            }
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some((name, value)) = trimmed.split_once(':') {
                if name.eq_ignore_ascii_case("Content-Length") {
                    content_length = value.trim().parse::<usize>().ok();
                }
            }
        }

        let Some(content_length) = content_length else {
            return Ok(None);
        };

        let mut body = vec![0_u8; content_length];
        self.reader.read_exact(&mut body)?;
        let value = serde_json::from_slice::<Value>(&body)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        Ok(Some(value))
    }

    fn write_message(&mut self, value: &Value) -> io::Result<()> {
        let body = serde_json::to_vec(value)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        write!(self.writer, "Content-Length: {}\r\n\r\n", body.len())?;
        self.writer.write_all(&body)?;
        self.writer.flush()
    }
}

fn collect_diagnostics(source: &str) -> Vec<Value> {
    match crate::parse_source(source) {
        Ok(program) => {
            let mut checker = TypeChecker::new();
            checker.check_program(&program);
            checker.errors.iter().map(to_diagnostic).collect()
        }
        Err(err) => vec![to_diagnostic(&err)],
    }
}

fn to_diagnostic(error: &TinyLangError) -> Value {
    let (line, column) = match error.span {
        Some(span) => (span.line.saturating_sub(1), span.column.saturating_sub(1)),
        None => (0, 0),
    };
    json!({
        "range": {
            "start": { "line": line, "character": column },
            "end": { "line": line, "character": column + 1 }
        },
        "severity": severity(error.kind),
        "source": "tiny-lsp",
        "message": error.to_string()
    })
}

fn severity(kind: ErrorKind) -> i32 {
    match kind {
        ErrorKind::Lex | ErrorKind::Parse | ErrorKind::Runtime | ErrorKind::Io | ErrorKind::TypeCheck => 1,
    }
}
