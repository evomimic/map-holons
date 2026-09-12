//! Minimal JSON-RPC transport for source-only TDL editor services.
//!
//! The server intentionally implements only editor operations backed by the
//! source workspace. It never invokes Holon Loading or resolves a loader key.

use crate::editor_service::{EditorDiagnostic, EditorSymbol, EditorWorkspace, SourcePosition};
use serde_json::{json, Value};
use std::{
    io::{self, BufRead, BufReader, Write},
    path::PathBuf,
};

#[derive(Default)]
pub struct TdlLanguageServer {
    workspace: EditorWorkspace,
}

impl TdlLanguageServer {
    pub fn handle(&mut self, message: Value) -> Vec<Value> {
        let method = message.get("method").and_then(Value::as_str);
        match method {
            Some("initialize") => {
                if let Some(root) = message
                    .pointer("/params/rootUri")
                    .and_then(Value::as_str)
                    .and_then(path_from_uri)
                {
                    let source_root = root.join("schema-src");
                    if source_root.is_dir() {
                        let _ = self.workspace.load_source_root(&source_root);
                    }
                }
                response(
                    &message,
                    json!({
                        "capabilities": {
                            "textDocumentSync": 1,
                            "documentSymbolProvider": true,
                            "definitionProvider": true,
                            "referencesProvider": true,
                            "hoverProvider": true
                        },
                        "serverInfo": { "name": "map-schema TDL", "version": env!("CARGO_PKG_VERSION") }
                    }),
                )
                .into_iter()
                .collect()
            }
            Some("textDocument/didOpen") | Some("textDocument/didChange") => {
                let Some((uri, text)) = document_text(&message) else { return Vec::new() };
                let document = self.workspace.upsert_document(uri.clone(), &text);
                vec![publish_diagnostics(&uri, document.diagnostics())]
            }
            Some("textDocument/documentSymbol") => {
                let Some(uri) = message.pointer("/params/textDocument/uri").and_then(Value::as_str)
                else {
                    return response(&message, Value::Array(Vec::new())).into_iter().collect();
                };
                let symbols = self
                    .workspace
                    .document(uri)
                    .map(|document| document.symbols().iter().map(document_symbol).collect())
                    .unwrap_or_default();
                response(&message, Value::Array(symbols)).into_iter().collect()
            }
            Some("textDocument/definition") => self.locations_for_key_at(&message, false),
            Some("textDocument/references") => self.locations_for_key_at(&message, true),
            Some("textDocument/hover") => self.hover(&message),
            _ if message.get("id").is_some() => {
                response(&message, Value::Null).into_iter().collect()
            }
            _ => Vec::new(),
        }
    }

    fn locations_for_key_at(&self, message: &Value, usages: bool) -> Vec<Value> {
        let Some((uri, position)) = request_location(message) else {
            return response(message, Value::Array(Vec::new())).into_iter().collect();
        };
        let Some(key) =
            self.workspace.document(&uri).and_then(|document| document.key_at(position))
        else {
            return response(message, Value::Array(Vec::new())).into_iter().collect();
        };
        let locations = if usages {
            let mut locations = self
                .workspace
                .definitions(key)
                .into_iter()
                .map(location_for_symbol)
                .collect::<Vec<_>>();
            locations.extend(
                self.workspace
                    .usages(key)
                    .into_iter()
                    .map(|reference| json!({ "uri": reference.uri, "range": reference.range })),
            );
            locations
        } else {
            self.workspace.definitions(key).into_iter().map(location_for_symbol).collect()
        };
        response(message, Value::Array(locations)).into_iter().collect()
    }

    fn hover(&self, message: &Value) -> Vec<Value> {
        let Some((uri, position)) = request_location(message) else {
            return response(message, Value::Null).into_iter().collect();
        };
        let Some(key) =
            self.workspace.document(&uri).and_then(|document| document.key_at(position))
        else {
            return response(message, Value::Null).into_iter().collect();
        };
        let definitions = self.workspace.definitions(key);
        let detail = if definitions.is_empty() {
            format!("Authored loader key `{key}`. No declaration is available in the configured source roots; this may resolve against a previously saved holon.")
        } else {
            format!(
                "Authored loader key `{key}`. {} source declaration(s) available.",
                definitions.len()
            )
        };
        response(message, json!({ "contents": { "kind": "markdown", "value": detail } }))
            .into_iter()
            .collect()
    }
}

/// Runs the server using standard LSP `Content-Length` framing.
pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let mut server = TdlLanguageServer::default();
    while let Some(message) = read_message(&mut reader)? {
        for response in server.handle(message) {
            write_message(&mut writer, &response)?;
        }
    }
    Ok(())
}

fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let Some(length) = length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "LSP message has no Content-Length",
        ));
    };
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload)?;
    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn write_message(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let payload = serde_json::to_vec(message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write!(writer, "Content-Length: {}\r\n\r\n", payload.len())?;
    writer.write_all(&payload)?;
    writer.flush()
}

fn document_text(message: &Value) -> Option<(String, String)> {
    let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str)?.to_string();
    let text = message
        .pointer("/params/textDocument/text")
        .or_else(|| message.pointer("/params/contentChanges/0/text"))
        .and_then(Value::as_str)?
        .to_string();
    Some((uri, text))
}

fn request_location(message: &Value) -> Option<(String, SourcePosition)> {
    Some((
        message.pointer("/params/textDocument/uri").and_then(Value::as_str)?.to_string(),
        SourcePosition {
            line: message.pointer("/params/position/line").and_then(Value::as_u64)? as u32,
            character: message.pointer("/params/position/character").and_then(Value::as_u64)?
                as u32,
        },
    ))
}

fn response(message: &Value, result: Value) -> Option<Value> {
    message.get("id").cloned().map(|id| json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn publish_diagnostics(uri: &str, diagnostics: &[EditorDiagnostic]) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri, "diagnostics": diagnostics }
    })
}

/// Returns the hierarchical `DocumentSymbol` form rather than legacy
/// `SymbolInformation`: RustRover uses this shape to populate its Structure
/// tool window for files backed only by an LSP client.
fn document_symbol(symbol: &EditorSymbol) -> Value {
    json!({
        "name": symbol.key,
        "kind": 13,
        "detail": symbol.kind,
        "range": symbol.range,
        "selectionRange": symbol.range
    })
}

fn location_for_symbol(symbol: &EditorSymbol) -> Value {
    json!({ "uri": symbol.uri, "range": symbol.range })
}

fn path_from_uri(uri: &str) -> Option<PathBuf> {
    uri.strip_prefix("file://").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_source_definitions_without_resolving_saved_keys() {
        let mut server = TdlLanguageServer::default();
        server.handle(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": "file:///schema.tdl", "text": "schema Example\n\nholon Book.HolonType {\n  type PreviouslySaved.Type\n}\n" } }
        }));
        let response = server.handle(json!({
            "jsonrpc": "2.0", "id": 1, "method": "textDocument/definition",
            "params": { "textDocument": { "uri": "file:///schema.tdl" }, "position": { "line": 3, "character": 8 } }
        }));
        assert_eq!(response[0]["result"], json!([]));
    }

    #[test]
    fn serves_definition_from_the_available_source_corpus() {
        let mut server = TdlLanguageServer::default();
        server.handle(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": "file:///schema.tdl", "text": "schema Example\n\nholon Example.Type {\n  type HolonType.TypeDescriptor\n}\n\nholon Example.Instance {\n  type Example.Type\n}\n" } }
        }));
        let response = server.handle(json!({
            "jsonrpc": "2.0", "id": 1, "method": "textDocument/definition",
            "params": { "textDocument": { "uri": "file:///schema.tdl" }, "position": { "line": 7, "character": 8 } }
        }));
        assert_eq!(response[0]["result"][0]["range"]["start"]["line"], json!(2));
    }

    #[test]
    fn returns_document_symbols_for_property_declarations() {
        let mut server = TdlLanguageServer::default();
        let source = include_str!("../../../schema-src/core/root.tdl");
        server.handle(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": "file:///root.tdl", "text": source } }
        }));
        let response = server.handle(json!({
            "jsonrpc": "2.0", "id": 1, "method": "textDocument/documentSymbol",
            "params": { "textDocument": { "uri": "file:///root.tdl" } }
        }));
        let symbols = response[0]["result"].as_array().expect("document symbols");
        assert!(symbols.iter().any(|symbol| symbol["name"] == "ConstraintName.PropertyType"));
        assert!(symbols.iter().all(|symbol| symbol.get("selectionRange").is_some()));
    }
}
