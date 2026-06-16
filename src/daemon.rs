use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{home::Home, runtime, state::Store};

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
        },
    )? {
        println!("ran {} {}", node.id, node.name);
    } else {
        println!("no-runnable-node");
    }
    Ok(())
}

fn serve(home: Home, socket: Option<PathBuf>) -> crate::Result<()> {
    home.require_initialized()?;
    let socket = socket_path(&home, socket);
    if socket.exists() {
        fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)?;
    println!("zgentd listening on {}", socket.display());
    for stream in listener.incoming() {
        handle_stream(&Store::open(home.clone())?, stream?)?;
    }
    Ok(())
}

fn handle_stream(store: &Store, stream: UnixStream) -> crate::Result<()> {
    let reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<IpcRequest>(&line) {
            Ok(request) => handle_request(store, request),
            Err(error) => json!({ "ok": false, "error": error.to_string() }),
        };
        serde_json::to_writer(&mut writer, &response)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

fn handle_request(store: &Store, request: IpcRequest) -> Value {
    match request.command.as_str() {
        "health" => json!({ "ok": true, "service": "zgentd" }),
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
        other => json!({ "ok": false, "error": format!("unknown command: {other}") }),
    }
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
            &store,
            super::IpcRequest {
                command: "health".to_string(),
                task_id: None,
            },
        );
        assert_eq!(health["ok"], true);
        let status = super::handle_request(
            &store,
            super::IpcRequest {
                command: "task_status".to_string(),
                task_id: Some(task_id),
            },
        );
        assert_eq!(status["ok"], true);
        assert_eq!(status["task"]["status"], "PENDING");
    }
}
