use std::{fs, path::Path};

use serde_json::{Value, json};

use crate::home::Home;

pub fn export_task(home: &Home, task_id: &str, out: &Path) -> crate::Result<()> {
    let events = read_events(home, task_id)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        out,
        serde_json::to_string_pretty(&json!({
            "resourceSpans": [{
                "resource": {
                    "attributes": [
                        { "key": "service.name", "value": { "stringValue": "zgent" } },
                        { "key": "zgent.task_id", "value": { "stringValue": task_id } }
                    ]
                },
                "scopeSpans": [{
                    "scope": { "name": "zgent.events" },
                    "spans": events
                }]
            }]
        }))?,
    )?;
    Ok(())
}

fn read_events(home: &Home, task_id: &str) -> crate::Result<Vec<Value>> {
    let path = home.task_events(task_id);
    if !path.exists() {
        return Ok(Vec::new());
    }
    fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{home::Home, state::NodeSpec, state::Store};

    #[test]
    fn exports_task_events_as_otel_shaped_json() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home.clone())?;
        let task_id = store.create_task(
            "export",
            "plan-only",
            vec![NodeSpec::new("plan", "planner")],
        )?;
        store.record_event(Some(&task_id), None, "cost.updated", json!({ "tokens": 1 }))?;
        let out = temp.path().join("otel.json");
        super::export_task(&home, &task_id, &out)?;
        let exported: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out)?)?;
        assert_eq!(
            exported["resourceSpans"][0]["scopeSpans"][0]["spans"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        Ok(())
    }
}
