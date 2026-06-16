use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct NormalizedEvent {
    pub kind: String,
    pub payload: Value,
    pub provider_session_id: Option<String>,
}

pub fn normalize_output(stream: &str, text: &str) -> Vec<NormalizedEvent> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| normalize_line(stream, line))
        .collect()
}

fn normalize_line(stream: &str, line: &str) -> NormalizedEvent {
    match serde_json::from_str::<Value>(line) {
        Ok(raw) => normalize_json(stream, raw),
        Err(_) => NormalizedEvent {
            kind: "agent.message".to_string(),
            payload: json!({ "stream": stream, "text": line }),
            provider_session_id: None,
        },
    }
}

fn normalize_json(stream: &str, raw: Value) -> NormalizedEvent {
    let tag = raw
        .get("type")
        .or_else(|| raw.get("event"))
        .or_else(|| raw.get("kind"))
        .or_else(|| raw.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("provider.event")
        .to_ascii_lowercase();
    let kind = if tag.contains("reason") {
        "agent.reasoning"
    } else if tag.contains("message") || tag.contains("assistant") || tag.contains("content") {
        "agent.message"
    } else if tag.contains("tool") && (tag.contains("start") || tag.contains("call")) {
        "tool.started"
    } else if tag.contains("tool")
        && (tag.contains("complete") || tag.contains("result") || tag.contains("end"))
    {
        "tool.completed"
    } else if tag.contains("tool") && (tag.contains("fail") || tag.contains("error")) {
        "tool.failed"
    } else if tag.contains("patch") {
        "patch.created"
    } else if tag.contains("cost") || tag.contains("usage") || tag.contains("token") {
        "cost.updated"
    } else if tag.contains("fail") || tag.contains("error") {
        "run.failed"
    } else {
        "provider.event"
    };
    let provider_session_id = raw
        .get("session_id")
        .or_else(|| raw.get("conversation_id"))
        .or_else(|| raw.get("thread_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            raw.get("session")
                .and_then(|session| session.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    NormalizedEvent {
        kind: kind.to_string(),
        payload: json!({ "stream": stream, "raw": raw }),
        provider_session_id,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn normalizes_jsonl_and_text_output() {
        let events = super::normalize_output(
            "stdout",
            "{\"type\":\"assistant_message\",\"session_id\":\"abc\",\"content\":\"hi\"}\nplain",
        );
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, "agent.message");
        assert_eq!(events[0].provider_session_id.as_deref(), Some("abc"));
        assert_eq!(events[1].kind, "agent.message");
    }

    #[test]
    fn maps_tool_and_usage_events() {
        let events = super::normalize_output(
            "stdout",
            "{\"event\":\"tool_start\",\"name\":\"read\"}\n{\"type\":\"usage\",\"tokens\":10}",
        );
        assert_eq!(events[0].kind, "tool.started");
        assert_eq!(events[1].kind, "cost.updated");
    }
}
