use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
};

use serde::Serialize;
use serde_json::{Value, json};

use crate::{state::Store, workflows};

#[derive(Debug, Clone, Serialize)]
pub struct GatewayPlan {
    pub id: &'static str,
    pub status: &'static str,
    pub reason: &'static str,
    pub earliest_phase: &'static str,
}

pub fn plans() -> Vec<GatewayPlan> {
    vec![
        GatewayPlan {
            id: "acp",
            status: "available-local",
            reason: "JSON-RPC bridge for local clients to create and inspect zgent tasks",
            earliest_phase: "P2",
        },
        GatewayPlan {
            id: "a2a",
            status: "available-local",
            reason: "HTTP+JSON bridge with agent-card discovery and task submission",
            earliest_phase: "P3",
        },
    ]
}

pub fn acp_stdio(store: &Store) -> crate::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let request: Value = serde_json::from_str(&line?)?;
        let response = acp_handle(store, request)?;
        if !response.is_null() {
            serde_json::to_writer(&mut stdout, &response)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

pub fn acp_handle(store: &Store, request: Value) -> crate::Result<Value> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Ok(rpc_error(id, -32600, "invalid request"));
    };
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "initialize" => json!({
            "protocolVersion": params.get("protocolVersion").cloned().unwrap_or_else(|| json!(1)),
            "agentInfo": { "name": "zgent", "version": env!("CARGO_PKG_VERSION") },
            "agentCapabilities": {
                "loadSession": false,
                "promptCapabilities": { "image": false, "audio": false, "embeddedContext": false },
                "sessionCapabilities": { "list": true }
            },
            "authMethods": []
        }),
        "session/list" => json!({
            "sessions": store.tasks()?.into_iter().map(|task| json!({
                "sessionId": task.id,
                "title": task.goal,
                "cwd": std::env::current_dir().ok().map(|path| path.display().to_string()),
                "updatedAt": task.updated_at
            })).collect::<Vec<_>>(),
            "nextCursor": null
        }),
        "session/cancel" => {
            if let Some(session_id) = params.get("sessionId").and_then(Value::as_str) {
                store.cancel_task(session_id)?;
            }
            if id.is_null() {
                return Ok(Value::Null);
            }
            json!({})
        }
        "_zgent/task/create" => {
            let goal = params
                .get("goal")
                .and_then(Value::as_str)
                .unwrap_or("ACP task");
            let workflow = params
                .get("workflow")
                .and_then(Value::as_str)
                .unwrap_or("plan-only");
            let task_id = create_task(store, goal, workflow)?;
            json!({ "taskId": task_id })
        }
        "_zgent/task/status" => {
            let task_id = params
                .get("taskId")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("taskId is required"))?;
            json!({ "task": store.task(task_id)? })
        }
        _ => return Ok(rpc_error(id, -32601, "method not found")),
    };
    Ok(rpc_result(id, result))
}

pub fn a2a_agent_card(base_url: &str) -> Value {
    json!({
        "name": "zgent",
        "description": "Local-first coordinator for AI coding agents.",
        "version": env!("CARGO_PKG_VERSION"),
        "supportedInterfaces": [{
            "url": format!("{}/a2a/v1", base_url.trim_end_matches('/')),
            "protocolBinding": "JSONRPC",
            "protocolVersion": "1.0"
        }],
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
            "extendedAgentCard": false
        },
        "defaultInputModes": ["text/plain", "application/json"],
        "defaultOutputModes": ["application/json", "text/plain"],
        "skills": [{
            "id": "coordinate-coding-agents",
            "name": "Coordinate Coding Agents",
            "description": "Creates durable zgent tasks for provider-neutral coding-agent workflows.",
            "tags": ["coding", "coordination", "workflow"],
            "inputModes": ["text/plain", "application/json"],
            "outputModes": ["application/json", "text/plain"]
        }]
    })
}

pub fn a2a_jsonrpc(store: &Store, request: Value, base_url: &str) -> crate::Result<Value> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Ok(rpc_error(id, -32600, "invalid request"));
    };
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "message/send" => {
            let goal = a2a_message_text(&params).unwrap_or_else(|| "A2A task".to_string());
            let task_id = create_task(store, &goal, "implement-with-review")?;
            a2a_task(store, &task_id)?
        }
        "tasks/get" => {
            let task_id = params
                .get("id")
                .or_else(|| params.get("taskId"))
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("task id is required"))?;
            a2a_task(store, task_id)?
        }
        "tasks/cancel" => {
            let task_id = params
                .get("id")
                .or_else(|| params.get("taskId"))
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("task id is required"))?;
            store.cancel_task(task_id)?;
            a2a_task(store, task_id)?
        }
        "agent/getCard" => a2a_agent_card(base_url),
        _ => return Ok(rpc_error(id, -32601, "method not found")),
    };
    Ok(rpc_result(id, result))
}

pub fn serve_a2a(store: &Store, addr: &str) -> crate::Result<()> {
    let listener = TcpListener::bind(addr)?;
    println!(
        "zgent A2A gateway listening on http://{}",
        listener.local_addr()?
    );
    let base_url = format!("http://{}", listener.local_addr()?);
    for stream in listener.incoming() {
        respond_a2a(store, stream?, &base_url)?;
    }
    Ok(())
}

fn respond_a2a(store: &Store, mut stream: TcpStream, base_url: &str) -> crate::Result<()> {
    let request = read_http(&mut stream)?;
    let response = match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/.well-known/agent-card.json") => http_json(200, a2a_agent_card(base_url)),
        ("POST", "/a2a/v1") => {
            let body: Value = serde_json::from_slice(&request.body)?;
            http_json(200, a2a_jsonrpc(store, body, base_url)?)
        }
        _ => http_json(404, json!({ "error": "not found" })),
    };
    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn create_task(store: &Store, goal: &str, workflow: &str) -> crate::Result<String> {
    let nodes = workflows::nodes_for_home(store.home(), workflow)?;
    store.create_task(goal, workflow, nodes)
}

fn a2a_task(store: &Store, task_id: &str) -> crate::Result<Value> {
    let task = store
        .task(task_id)?
        .ok_or_else(|| anyhow::anyhow!("task not found: {task_id}"))?;
    Ok(json!({
        "id": task.id,
        "contextId": task.workflow,
        "status": { "state": a2a_state(&task.status) },
        "artifacts": [],
        "metadata": { "goal": task.goal }
    }))
}

fn a2a_state(status: &str) -> &'static str {
    match status {
        "PENDING" => "submitted",
        "RUNNING" => "working",
        "COMPLETED" => "completed",
        "FAILED" => "failed",
        "CANCELLED" => "canceled",
        _ => "unknown",
    }
}

fn a2a_message_text(params: &Value) -> Option<String> {
    let parts = params.get("message")?.get("parts")?.as_array()?;
    let text = parts
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    (!text.trim().is_empty()).then_some(text)
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_http(stream: &mut TcpStream) -> crate::Result<HttpRequest> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut first = String::new();
    reader.read_line(&mut first)?;
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let mut content_length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().unwrap_or(0);
        }
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;
    Ok(HttpRequest { method, path, body })
}

fn http_json(status: u16, body: Value) -> String {
    let text = serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string());
    let reason = if status == 200 { "OK" } else { "Not Found" };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        text.len(),
        text
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{home::Home, state::Store};

    #[test]
    fn handles_acp_create_and_status() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let store = Store::open(Home::from_path(temp.path().join(".zgent")))?;
        let created = super::acp_handle(
            &store,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "_zgent/task/create",
                "params": { "goal": "bridge task" }
            }),
        )?;
        let task_id = created["result"]["taskId"].as_str().unwrap();
        let status = super::acp_handle(
            &store,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "_zgent/task/status",
                "params": { "taskId": task_id }
            }),
        )?;
        assert_eq!(status["result"]["task"]["goal"], "bridge task");
        Ok(())
    }

    #[test]
    fn handles_a2a_message_send() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let store = Store::open(Home::from_path(temp.path().join(".zgent")))?;
        let response = super::a2a_jsonrpc(
            &store,
            json!({
                "jsonrpc": "2.0",
                "id": "a",
                "method": "message/send",
                "params": {
                    "message": {
                        "parts": [{ "text": "coordinate this change" }]
                    }
                }
            }),
            "http://127.0.0.1:8766",
        )?;
        assert_eq!(response["result"]["status"]["state"], "submitted");
        assert_eq!(
            response["result"]["metadata"]["goal"],
            "coordinate this change"
        );
        assert_eq!(
            super::a2a_agent_card("http://127.0.0.1:8766")["name"],
            "zgent"
        );
        Ok(())
    }
}
