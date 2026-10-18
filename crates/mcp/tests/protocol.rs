use blyacut_mcp::Protocol;
use serde_json::{json, Value};
fn request(p: &mut Protocol, method: &str, params: Value) -> Value {
    p.handle(
        json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}),
        |_, _| Ok(json!({"ok":true})),
    )
    .unwrap()
}
#[test]
fn lifecycle_schema_and_errors() {
    let mut p = Protocol::default();
    assert_eq!(
        request(&mut p, "tools/list", json!({}))["error"]["code"],
        -32002
    );
    let init = request(
        &mut p,
        "initialize",
        json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}),
    );
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    assert!(p
        .handle(
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            |_, _| unreachable!()
        )
        .is_none());
    let tools = request(&mut p, "tools/list", json!({}));
    let tools = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 12);
    for tool in tools {
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    }
    assert_eq!(
        request(&mut p, "tools/call", json!({"name":"unknown"}))["error"]["code"],
        -32602
    );
    assert_eq!(
        request(
            &mut p,
            "tools/call",
            json!({"name":"get_project","arguments":[]})
        )["error"]["code"],
        -32602
    );
    assert_eq!(
        request(&mut p, "tools/list", Value::Null)["error"]["code"],
        -32602
    );
    let response=p.handle(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_project","arguments":{}}}),|_,_|Err("Editor offline".into())).unwrap();
    assert_eq!(response["result"]["isError"], true);
}
#[cfg(unix)]
#[test]
fn actual_stdio_subprocess_controls_the_shared_session() {
    use blyacut_session::{gateway::Gateway, ipc::Server, Session};
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
        sync::Arc,
    };
    let session = Arc::new(Session::default());
    let server =
        Server::start(Gateway::new(session.clone(), &std::env::temp_dir()).unwrap()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_blyacut-mcp"))
        .arg(&server.info.config_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut send = |message: Value, reply: bool| -> Option<Value> {
        writeln!(input, "{message}").unwrap();
        input.flush().unwrap();
        if reply {
            let mut line = String::new();
            output.read_line(&mut line).unwrap();
            Some(serde_json::from_str(&line).unwrap())
        } else {
            None
        }
    };
    let result=send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"integration","version":"1"}}}),true).unwrap();
    assert_eq!(result["result"]["protocolVersion"], "2025-06-18");
    send(
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        false,
    );
    let result=send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"apply_edits","arguments":{"expected_revision":0,"edits":[{"operation":"add_track","track_id":"external-music","name":"Agent music","kind":"audio"}]}}}),true).unwrap();
    assert_eq!(result["result"]["isError"], false);
    assert!(session
        .snapshot()
        .unwrap()
        .project
        .tracks
        .iter()
        .any(|t| t.id == "external-music"));
    drop(input);
    assert!(child.wait().unwrap().success());
}
