use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{adapters, approvals, home::Home, runtime, state::Store, workflows};

pub fn socket_path(home: &Home, socket: Option<PathBuf>) -> PathBuf {
    socket.unwrap_or_else(|| home.root().join("zgentd.sock"))
}

pub fn send_request(socket: PathBuf, request: Value) -> crate::Result<Value> {
    let mut stream = UnixStream::connect(socket)?;
    serde_json::to_writer(&mut stream, &request)?;
    stream.write_all(b"\n")?;
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    Ok(serde_json::from_str(&response)?)
}

#[derive(Debug, Parser)]
#[command(name = "zgentd", version, about = "Local zgent scheduler daemon")]
struct DaemonCli {
    #[arg(long, global = true, value_name = "PATH")]
    home: Option<PathBuf>,
    #[arg(long, global = true)]
    project: bool,
    #[command(subcommand)]
    command: Option<DaemonCommand>,
}

#[derive(Debug, Subcommand)]
enum DaemonCommand {
    Once(OnceArgs),
    Serve {
        #[arg(long)]
        socket: Option<PathBuf>,
    },
}

#[derive(Debug, Args)]
struct OnceArgs {
    task_id: String,
    #[arg(long, default_value = "zgentd")]
    owner: String,
    #[arg(long, default_value = "fake")]
    adapter: String,
    #[arg(long = "require-lock")]
    required_locks: Vec<String>,
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct IpcRequest {
    command: String,
    task_id: Option<String>,
    adapter: Option<String>,
    prompt: Option<String>,
    owner: Option<String>,
    permission_mode: Option<String>,
}

pub fn run() -> crate::Result<()> {
    let cli = DaemonCli::parse();
    let home = Home::resolve(cli.home, cli.project)?;
    match cli.command {
        Some(DaemonCommand::Once(args)) => once(home, args),
        Some(DaemonCommand::Serve { socket }) => serve(home, socket),
        None => {
            println!("zgentd: use `once` or `serve`");
            Ok(())
        }
    }
}

fn once(home: Home, args: OnceArgs) -> crate::Result<()> {
    home.require_initialized()?;
    let store = Store::open(home)?;
    if let Some(node) = runtime::run_next_command(
        &store,
        &args.task_id,
        &args.owner,
        &args.adapter,
        &args.command,
        runtime::RunOptions {
            required_locks: args.required_locks,
            ..Default::default()
        },
    )? {
        println!("ran {} {}", node.id, node.name);
    } else {
        println!("no-runnable-node");
    }
    Ok(())
}

pub fn serve(home: Home, socket: Option<PathBuf>) -> crate::Result<()> {
    home.require_initialized()?;
    let socket = socket_path(&home, socket);
    if socket.exists() {
        fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)?;
    println!("zgentd listening on {}", socket.display());
    for stream in listener.incoming() {
        handle_stream(&home, &Store::open(home.clone())?, stream?)?;
    }
    Ok(())
}

fn handle_stream(home: &Home, store: &Store, stream: UnixStream) -> crate::Result<()> {
    let reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<IpcRequest>(&line) {
            Ok(request) => handle_request(home, store, request),
            Err(error) => json!({ "ok": false, "error": error.to_string() }),
        };
        serde_json::to_writer(&mut writer, &response)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

fn handle_request(home: &Home, store: &Store, request: IpcRequest) -> Value {
    match request.command.as_str() {
        "health" => json!({ "ok": true, "service": "zgentd" }),
        "adapters" => match adapters::registered_infos(home) {
            Ok(adapters) => json!({ "ok": true, "adapters": adapters }),
            Err(error) => json!({ "ok": false, "error": error.to_string() }),
        },
        "tasks" => match store.tasks() {
            Ok(tasks) => json!({ "ok": true, "tasks": tasks }),
            Err(error) => json!({ "ok": false, "error": error.to_string() }),
        },
        "task_status" => match request
            .task_id
            .as_deref()
            .and_then(|id| store.task(id).ok().flatten())
        {
            Some(task) => json!({ "ok": true, "task": task }),
            None => json!({ "ok": false, "error": "task not found" }),
        },
        "locks" => match store.locks() {
            Ok(locks) => json!({ "ok": true, "locks": locks }),
            Err(error) => json!({ "ok": false, "error": error.to_string() }),
        },
        "submit_prompt" => match submit_prompt(home, store, request) {
            Ok(task_id) => json!({ "ok": true, "task_id": task_id }),
            Err(error) => json!({ "ok": false, "error": error.to_string() }),
        },
        other => json!({ "ok": false, "error": format!("unknown command: {other}") }),
    }
}

fn submit_prompt(home: &Home, store: &Store, request: IpcRequest) -> crate::Result<String> {
    let adapter = request.adapter.as_deref().unwrap_or("cursor");
    let prompt = request
        .prompt
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("prompt is required"))?;
    let owner = request.owner.as_deref().unwrap_or("zgent-daemon");
    let permission_mode = approvals::parse_permission_mode(request.permission_mode.as_deref())?;
    let task_id = store.create_task(
        prompt,
        "plan-only",
        workflows::nodes_for_home(home, "plan-only")?,
    )?;
    store.record_event(
        Some(&task_id),
        None,
        "user.message",
        json!({ "text": prompt, "adapter": adapter, "permission_mode": permission_mode.as_str() }),
    )?;
    let plan = adapters::plan_start(home, adapter, prompt)?;
    let command = adapters::command_from_plan(plan);
    runtime::run_next_command(
        store,
        &task_id,
        owner,
        adapter,
        &command,
        runtime::RunOptions {
            permission_mode,
            ..Default::default()
        },
    )?;
    Ok(task_id)
}

#[cfg(test)]
mod tests {
    use crate::{home::Home, state::NodeSpec, state::Store};

    #[test]
    fn handles_health_and_task_status_requests() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home).unwrap();
        let task_id = store
            .create_task(
                "status",
                "plan-only",
                vec![NodeSpec::new("plan", "planner")],
            )
            .unwrap();
        let health = super::handle_request(
            store.home(),
            &store,
            super::IpcRequest {
                command: "health".to_string(),
                task_id: None,
                adapter: None,
                prompt: None,
                owner: None,
                permission_mode: None,
            },
        );
        assert_eq!(health["ok"], true);
        let status = super::handle_request(
            store.home(),
            &store,
            super::IpcRequest {
                command: "task_status".to_string(),
                task_id: Some(task_id),
                adapter: None,
                prompt: None,
                owner: None,
                permission_mode: None,
            },
        );
        assert_eq!(status["ok"], true);
        assert_eq!(status["task"]["status"], "PENDING");
    }

    #[test]
    fn handles_adapter_listing() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home.clone()).unwrap();
        let adapters = super::handle_request(
            &home,
            &store,
            super::IpcRequest {
                command: "adapters".to_string(),
                task_id: None,
                adapter: None,
                prompt: None,
                owner: None,
                permission_mode: None,
            },
        );
        assert_eq!(adapters["ok"], true);
        assert!(adapters["adapters"].as_array().unwrap().len() >= 4);
    }
}
