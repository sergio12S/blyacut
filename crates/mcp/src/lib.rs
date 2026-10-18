//! Small stdio MCP adapter, pinned to the 2025-11-25 lifecycle (also accepts 2025-06-18).
mod schema;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
#[derive(Default)]
pub struct Protocol {
    initialized: bool,
    ready: bool,
}
fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
impl Protocol {
    pub fn handle(
        &mut self,
        request: Value,
        mut call: impl FnMut(&str, Value) -> Result<Value, String>,
    ) -> Option<Value> {
        let id = request.get("id").cloned();
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return Some(error(id.unwrap_or(Value::Null), -32600, "Invalid request"));
        };
        if request.get("jsonrpc") != Some(&json!("2.0"))
            || !request.is_object()
            || id
                .as_ref()
                .is_some_and(|v| !v.is_string() && !v.is_i64() && !v.is_u64())
        {
            return Some(error(Value::Null, -32600, "Invalid JSON-RPC envelope"));
        }
        if id.is_none() {
            if method == "notifications/initialized" && self.initialized {
                self.ready = true;
            }
            return None;
        }
        let id = id.unwrap();
        let params = request.get("params").cloned().unwrap_or(json!({}));
        if !params.is_object() {
            return Some(error(id, -32602, "params must be an object"));
        }
        let result = match method {
            "initialize" => {
                if self.initialized {
                    return Some(error(id, -32600, "Already initialized"));
                }
                let Some(version) = params.get("protocolVersion").and_then(Value::as_str) else {
                    return Some(error(id, -32602, "protocolVersion is required"));
                };
                if !params.get("capabilities").is_some_and(Value::is_object)
                    || !params.get("clientInfo").is_some_and(Value::is_object)
                {
                    return Some(error(
                        id,
                        -32602,
                        "clientInfo and capabilities are required",
                    ));
                }
                self.initialized = true;
                json!({"protocolVersion":if ["2025-11-25","2025-06-18"].contains(&version){version}else{"2025-11-25"},"capabilities":{"tools":{}},"serverInfo":{"name":"blyacut","version":env!("CARGO_PKG_VERSION")},"instructions":"Controls the running local BlyaCut session. Read get_project before editing. Only authorized-folder media imports/new exports are allowed. Changes immediately update the desktop timeline."})
            }
            "ping" => json!({}),
            _ if !self.ready => {
                return Some(error(
                    id,
                    -32002,
                    "Complete initialize and notifications/initialized first",
                ))
            }
            "tools/list" => {
                if params.get("cursor").is_some() {
                    return Some(error(id, -32602, "No pagination cursor supported"));
                }
                json!({"tools":schema::tools()})
            }
            "tools/call" => {
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return Some(error(id, -32602, "Tool name is required"));
                };
                if !schema::tools().iter().any(|t| t["name"] == name) {
                    return Some(error(id, -32602, "Unknown tool"));
                }
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
                if !arguments.is_object() {
                    return Some(error(id, -32602, "arguments must be an object"));
                }
                match call(name, arguments) {
                    Ok(value) => {
                        json!({"content":[{"type":"text","text":value.to_string()}],"structuredContent":value,"isError":false})
                    }
                    Err(e) => json!({"content":[{"type":"text","text":e}],"isError":true}),
                }
            }
            _ => return Some(error(id, -32601, "Method not found")),
        };
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }
}
pub fn run(config_path: PathBuf) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::{BufReader, Write};
        let stdin = std::io::stdin();
        let mut input = BufReader::new(stdin.lock());
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        let mut protocol = Protocol::default();
        loop {
            let request = match blyacut_session::ipc::read_json(
                &mut input,
                blyacut_session::ipc::REQUEST_LIMIT,
            ) {
                Ok(Some(value)) => value,
                Ok(None) => break,
                Err(e) => {
                    writeln!(output, "{}", error(Value::Null, -32700, &e))
                        .map_err(|e| e.to_string())?;
                    output.flush().map_err(|e| e.to_string())?;
                    break;
                }
            };
            if let Some(response) =
                protocol.handle(request, |name, args| editor_call(&config_path, name, args))
            {
                writeln!(output, "{response}").map_err(|e| e.to_string())?;
                output.flush().map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = config_path;
        Err("Local MCP bridge currently supports macOS/Linux only".into())
    }
}
fn editor_call(path: &Path, name: &str, args: Value) -> Result<Value, String> {
    #[cfg(unix)]
    {
        blyacut_session::ipc::call(path, name, args)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, name, args);
        Err("Unix IPC required".into())
    }
}
