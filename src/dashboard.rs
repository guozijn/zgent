use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
};

use crate::state::Store;

pub fn render(store: &Store) -> crate::Result<String> {
    let tasks = store.tasks()?;
    let workers = store.workers()?;
    let mut html = String::from(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>zgent dashboard</title>
  <style>
    body { font-family: ui-sans-serif, system-ui, sans-serif; margin: 32px; color: #202124; }
    table { border-collapse: collapse; width: 100%; margin: 16px 0 32px; }
    th, td { border-bottom: 1px solid #ddd; padding: 8px; text-align: left; }
    th { background: #f6f8fa; }
    .status { font-family: ui-monospace, monospace; }
  </style>
</head>
<body>
  <h1>zgent</h1>
"#,
    );
    html.push_str(
        "<h2>Tasks</h2><table><tr><th>ID</th><th>Status</th><th>Workflow</th><th>Goal</th></tr>",
    );
    for task in tasks {
        html.push_str(&format!(
            "<tr><td>{}</td><td class=\"status\">{}</td><td>{}</td><td>{}</td></tr>",
            escape(&task.id),
            escape(&task.status),
            escape(&task.workflow),
            escape(&task.goal)
        ));
    }
    html.push_str("</table><h2>Workers</h2><table><tr><th>ID</th><th>Status</th><th>Endpoint</th><th>Capabilities</th></tr>");
    for worker in workers {
        html.push_str(&format!(
            "<tr><td>{}</td><td class=\"status\">{}</td><td>{}</td><td>{}</td></tr>",
            escape(&worker.id),
            escape(&worker.status),
            escape(&worker.endpoint),
            escape(&worker.capabilities.join(", "))
        ));
    }
    html.push_str("</table></body></html>\n");
    Ok(html)
}

pub fn export(store: &Store, out: &Path) -> crate::Result<()> {
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, render(store)?)?;
    Ok(())
}

pub fn serve(store: &Store, addr: &str) -> crate::Result<()> {
    let listener = TcpListener::bind(addr)?;
    println!(
        "zgent dashboard listening on http://{}",
        listener.local_addr()?
    );
    for stream in listener.incoming() {
        respond(stream?, &render(store)?)?;
    }
    Ok(())
}

fn respond(mut stream: TcpStream, html: &str) -> crate::Result<()> {
    let mut buffer = [0_u8; 1024];
    let _ = stream.read(&mut buffer)?;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use crate::{home::Home, state::NodeSpec, state::Store};

    #[test]
    fn renders_tasks_and_workers() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home)?;
        store.create_task(
            "dashboard",
            "plan-only",
            vec![NodeSpec::new("plan", "planner")],
        )?;
        store.upsert_worker("worker-1", "local", &["fake".to_string()])?;
        let html = super::render(&store)?;
        assert!(html.contains("dashboard"));
        assert!(html.contains("worker-1"));
        Ok(())
    }
}
