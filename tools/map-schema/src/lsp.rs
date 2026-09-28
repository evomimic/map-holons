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
                let configured = message
                    .pointer("/params/initializationOptions/sourceRoots")
                    .and_then(Value::as_array);
                let roots: Vec<PathBuf> = if let Some(roots) = configured {
                    roots.iter().filter_map(Value::as_str).filter_map(path_from_uri).collect()
                } else if let Some(folders) =
                    message.pointer("/params/workspaceFolders").and_then(Value::as_array)
                {
                    folders
                        .iter()
                        .filter_map(|folder| folder.get("uri"))
                        .filter_map(Value::as_str)
                        .filter_map(path_from_uri)
                        .map(|root| root.join("schema-src"))
                        .collect()
                } else {
                    message
                        .pointer("/params/rootUri")
                        .and_then(Value::as_str)
                        .and_then(path_from_uri)
                        .map(|root| vec![root.join("schema-src")])
                        .unwrap_or_default()
                };
                for root in roots {
                    if root.is_dir() {
                        let _ = self.workspace.load_source_root(&root);
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
                            "hoverProvider": true,
                            "workspaceSymbolProvider": true,
                            "experimental": { "tdlRelationshipGraph": { "version": 1, "method": "tdl/relationshipGraph" } }
                        },
                        "serverInfo": { "name": "map-schema TDL", "version": env!("CARGO_PKG_VERSION") }
                    }),
                )
                .into_iter()
                .collect()
            }
            Some("textDocument/didOpen") | Some("textDocument/didChange") => {
                let Some((uri, text)) = document_text(&message) else { return Vec::new() };
                self.workspace.upsert_document(uri, &text);
                self.workspace
                    .documents()
                    .map(|document| publish_diagnostics(document.uri(), document.diagnostics()))
                    .collect()
            }
            Some("textDocument/didClose") => {
                let Some(uri) = message.pointer("/params/textDocument/uri").and_then(Value::as_str)
                else {
                    return Vec::new();
                };
                self.workspace.close_document(uri);
                let mut diagnostics: Vec<_> = self
                    .workspace
                    .documents()
                    .map(|d| publish_diagnostics(d.uri(), d.diagnostics()))
                    .collect();
                if self.workspace.document(uri).is_none() {
                    diagnostics.push(publish_diagnostics(uri, &[]));
                }
                diagnostics
            }
            Some("workspace/symbol") => {
                let query = message.pointer("/params/query").and_then(Value::as_str).unwrap_or("");
                let symbols: Vec<_> = self
                    .workspace
                    .symbols(query)
                    .into_iter()
                    .map(|symbol| {
                        json!({
                            "name": symbol.key, "kind": 13, "location": location_for_symbol(symbol)
                        })
                    })
                    .collect();
                response(&message, json!(symbols)).into_iter().collect()
            }
            Some("tdl/relationshipGraph") => {
                response(&message, json!(self.workspace.relationship_graph())).into_iter().collect()
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
            let mut locations = if message
                .pointer("/params/context/includeDeclaration")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                self.workspace
                    .definitions(key)
                    .into_iter()
                    .map(location_for_symbol)
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
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
        "params": { "uri": uri, "diagnostics": diagnostics.iter().map(|d| json!({
            "message": d.message, "code": d.code, "range": d.range, "source": "map-schema",
            "severity": match d.severity { crate::editor_service::EditorDiagnosticSeverity::Error => 1, crate::editor_service::EditorDiagnosticSeverity::Warning => 2 }
        })).collect::<Vec<_>>() }
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
    let raw = uri.strip_prefix("file://")?;
    let mut bytes = Vec::new();
    let mut iter = raw.bytes();
    while let Some(byte) = iter.next() {
        if byte == b'%' {
            let high = (iter.next()? as char).to_digit(16)?;
            let low = (iter.next()? as char).to_digit(16)?;
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok().map(PathBuf::from)
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
    #[test]
    fn protocol_exposes_graph_workspace_symbols_hover_and_cross_file_usages() {
        let mut server = TdlLanguageServer::default();
        let initialized =
            server.handle(json!({"jsonrpc":"2.0", "id":0, "method":"initialize", "params":{}}));
        assert_eq!(
            initialized[0]["result"]["capabilities"]["experimental"]["tdlRelationshipGraph"]
                ["version"],
            1
        );
        for (uri, source) in [
            ("file:///a.tdl", "schema S\nholon A {\n type T\n}\n"),
            ("file:///b.tdl", "schema S\ninstance B {\n type A\n}\n"),
        ] {
            let diagnostics = server.handle(json!({"jsonrpc":"2.0", "method":"textDocument/didOpen", "params":{"textDocument":{"uri":uri, "text":source}}}));
            assert!(diagnostics.iter().all(|d| d["params"]["diagnostics"] == json!([])));
        }
        let params = json!({"textDocument":{"uri":"file:///b.tdl"}, "position":{"line":2,"character":6}, "context":{"includeDeclaration":false}});
        for method in ["textDocument/definition", "textDocument/references", "textDocument/hover"] {
            let result =
                server.handle(json!({"jsonrpc":"2.0", "id":1, "method":method, "params":params}));
            if method.ends_with("hover") {
                assert!(result[0]["result"]["contents"]["value"]
                    .as_str()
                    .unwrap()
                    .contains("1 source declaration"));
            } else {
                let locations = result[0]["result"].as_array().unwrap();
                assert_eq!(locations.len(), 1);
                assert_eq!(
                    locations[0]["uri"],
                    if method.ends_with("definition") { "file:///a.tdl" } else { "file:///b.tdl" }
                );
            }
        }
        let symbols = server.handle(
            json!({"jsonrpc":"2.0", "id":2, "method":"workspace/symbol", "params":{"query":"B"}}),
        );
        assert_eq!(symbols[0]["result"].as_array().unwrap().len(), 1);
        assert_eq!(symbols[0]["result"][0]["name"], "B");
        let graph =
            server.handle(json!({"jsonrpc":"2.0", "id":3, "method":"tdl/relationshipGraph"}));
        assert!(graph[0]["result"]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["source"] == "B"
                && edge["name"] == "DescribedBy"
                && edge["target"] == "A"));
    }

    #[test]
    fn protocol_updates_diagnostics_across_documents_and_clears_on_close() {
        let mut server = TdlLanguageServer::default();
        for uri in ["file:///a.tdl", "file:///b.tdl"] {
            server.handle(json!({"jsonrpc":"2.0", "method":"textDocument/didOpen", "params":{"textDocument":{"uri":uri,"text":"schema S\nholon A {\n type T\n}\n"}}}));
        }
        let messages = server.handle(json!({"jsonrpc":"2.0", "method":"textDocument/didChange", "params":{"textDocument":{"uri":"file:///b.tdl"},"contentChanges":[{"text":"schema S\nholon A {\n type T\n}\n"}]}}));
        assert_eq!(messages.len(), 2);
        for message in messages {
            let diagnostic = &message["params"]["diagnostics"][0];
            assert_eq!(diagnostic["severity"], 1);
            assert_eq!(diagnostic["code"], "TDL_DUPLICATE_KEY");
            assert_eq!(diagnostic["range"]["start"]["line"], 1);
        }
        let messages = server.handle(json!({"jsonrpc":"2.0", "method":"textDocument/didClose", "params":{"textDocument":{"uri":"file:///b.tdl"}}}));
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|m| m["params"]["diagnostics"] == json!([])));
    }

    #[test]
    fn configured_roots_and_open_buffers_share_encoded_document_identities() {
        let directory = std::env::temp_dir().join(format!("tdl roots {}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("schema 🌍.tdl");
        std::fs::write(&path, "schema S\nholon Disk {\n type T\n}\n").unwrap();
        let uri = crate::editor_service::file_uri(&path);
        assert_eq!(path_from_uri(&uri), Some(path.clone()));
        let mut server = TdlLanguageServer::default();
        server.handle(json!({"jsonrpc":"2.0", "id":0, "method":"initialize", "params":{"initializationOptions":{"sourceRoots":[crate::editor_service::file_uri(&directory)]}}}));
        assert_eq!(server.workspace.definitions("Disk").len(), 1);
        server.handle(json!({"jsonrpc":"2.0", "method":"textDocument/didOpen", "params":{"textDocument":{"uri":uri,"text":"schema S\nholon Buffer {\n type T\n}\n"}}}));
        assert!(server.workspace.definitions("Disk").is_empty());
        assert_eq!(server.workspace.definitions("Buffer").len(), 1);
        server.handle(json!({"jsonrpc":"2.0", "method":"textDocument/didClose", "params":{"textDocument":{"uri":uri}}}));
        assert_eq!(server.workspace.definitions("Disk").len(), 1);
        assert!(server.workspace.definitions("Buffer").is_empty());
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn definition_at_selected_relationship_key_end_finds_the_corpus_declaration() {
        let mut server = TdlLanguageServer::default();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let source_path = root.join("schema-src/core/root.tdl");
        let source = std::fs::read_to_string(&source_path).unwrap();
        let uri = crate::editor_service::file_uri(&source_path);
        let key = "(HolonType.TypeDescriptor)-[DescribedBy]->(TypeDescriptor)";
        let (line, text) = source.lines().enumerate().find(|(_, text)| text.contains(key)).unwrap();
        let start = text[..text.find(key).unwrap()].encode_utf16().count();
        let end = start + key.encode_utf16().count();
        server.handle(json!({"jsonrpc":"2.0", "id":0, "method":"initialize", "params":{"rootUri":crate::editor_service::file_uri(&root)}}));
        server.handle(json!({"jsonrpc":"2.0", "method":"textDocument/didOpen", "params":{"textDocument":{"uri":uri,"text":source}}}));
        for character in [start, start + 5, end - 1, end] {
            let response = server.handle(json!({"jsonrpc":"2.0", "id":1, "method":"textDocument/definition", "params":{"textDocument":{"uri":uri},"position":{"line":line,"character":character}}}));
            let locations = response[0]["result"].as_array().unwrap();
            assert_eq!(locations.len(), 1, "caret at {character}");
            assert_eq!(
                locations[0]["uri"],
                crate::editor_service::file_uri(
                    &root.join("schema-src/core/relationship-types.tdl")
                )
            );
        }
        // The comma is not part of the authored key: a caret past it has no declaration.
        let response = server.handle(json!({"jsonrpc":"2.0", "id":2, "method":"textDocument/definition", "params":{"textDocument":{"uri":uri},"position":{"line":line,"character":end + 1}}}));
        assert_eq!(response[0]["result"], json!([]));
    }
}
