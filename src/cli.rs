use std::path::PathBuf;

use anyhow::bail;
use clap::{Args, Parser, Subcommand};
use serde_json::json;

use crate::{
    adapters, approvals, collaboration, daemon, dashboard, gateways, home::Home, init, locks,
    marketplace, opencode_http, otel, patches, plugins, runtime, skills, state::Store, tasks,
    verify, workers, workflows, worktrees,
};

#[derive(Debug, Parser)]
#[command(
    name = "zgent",
    version,
    about = "Local-first coordinator for AI coding agents"
)]
struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    home: Option<PathBuf>,
    #[arg(long, global = true)]
    project: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init,
    Doctor,
    Agents {
        #[command(subcommand)]
        command: AgentCommand,
    },
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    Run(RunArgs),
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommand,
    },
    Locks {
        #[command(subcommand)]
        command: LockCommand,
    },
    Approvals {
        #[command(subcommand)]
        command: ApprovalCommand,
    },
    Plugins {
        #[command(subcommand)]
        command: PluginCommand,
    },
    Skills {
        #[command(subcommand)]
        command: SkillCommand,
    },
    Worktrees {
        #[command(subcommand)]
        command: WorktreeCommand,
    },
    Workers {
        #[command(subcommand)]
        command: WorkerCommand,
    },
    Dashboard {
        #[command(subcommand)]
        command: DashboardCommand,
    },
    Daemon {
        #[command(subcommand)]
        command: DaemonClientCommand,
    },
    Export {
        #[command(subcommand)]
        command: ExportCommand,
    },
    Gateways {
        #[command(subcommand)]
        command: GatewayCommand,
    },
    Marketplace {
        #[command(subcommand)]
        command: MarketplaceCommand,
    },
    Collaboration {
        #[command(subcommand)]
        command: CollaborationCommand,
    },
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    Detect,
    List,
    OpencodeServePlan {
        #[arg(long, default_value = "127.0.0.1")]
        hostname: String,
        #[arg(long, default_value_t = 4096)]
        port: u16,
    },
    OpencodeOpenapi {
        #[arg(long, default_value = "http://127.0.0.1:4096")]
        url: String,
    },
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    Create(TaskCreateArgs),
    Lease(LeaseArgs),
    Heartbeat(LeaseArgs),
    RunNext(RunNextArgs),
    RunAll(RunNextArgs),
    RunProviderNext(ProviderRunArgs),
    RunProviderAll(ProviderRunArgs),
    ResumeProviderNext(ProviderResumeArgs),
    ResumeProviderAll(ProviderResumeArgs),
    CapturePatch(CapturePatchArgs),
    Verify(VerifyArgs),
    Complete { node_id: String },
    Fail { node_id: String },
    Retry { node_id: String },
    Cancel { task_id: String },
    Status { task_id: String },
    Events { task_id: String },
}

#[derive(Debug, Args)]
struct TaskCreateArgs {
    goal: Vec<String>,
    #[arg(long, default_value = "plan-only")]
    workflow: String,
}

#[derive(Debug, Args)]
struct LeaseArgs {
    task_or_node_id: String,
    #[arg(long)]
    owner: String,
    #[arg(long, default_value_t = 300)]
    ttl: i64,
}

#[derive(Debug, Args)]
struct RunNextArgs {
    task_id: String,
    #[arg(long, default_value = "zgent-core")]
    owner: String,
    #[arg(long, default_value = "fake")]
    adapter: String,
    #[arg(long = "require-lock")]
    required_locks: Vec<String>,
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

#[derive(Debug, Args)]
struct ProviderRunArgs {
    task_id: String,
    #[arg(long)]
    adapter: String,
    #[arg(long, default_value = "zgent-core")]
    owner: String,
    #[arg(long = "require-lock")]
    required_locks: Vec<String>,
    prompt: Vec<String>,
}

#[derive(Debug, Args)]
struct ProviderResumeArgs {
    task_id: String,
    #[arg(long)]
    adapter: String,
    #[arg(long)]
    session_id: String,
    #[arg(long, default_value = "zgent-core")]
    owner: String,
    #[arg(long = "require-lock")]
    required_locks: Vec<String>,
    prompt: Vec<String>,
}

#[derive(Debug, Args)]
struct CapturePatchArgs {
    task_id: String,
    #[arg(long)]
    node: Option<String>,
    #[arg(long, default_value = ".")]
    repo: PathBuf,
}

#[derive(Debug, Args)]
struct VerifyArgs {
    task_id: String,
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

#[derive(Debug, Args)]
struct RunArgs {
    goal: Vec<String>,
    #[arg(long, default_value = "implement-with-review")]
    workflow: String,
}

#[derive(Debug, Subcommand)]
enum WorkflowCommand {
    List,
    Run {
        name: String,
        #[arg(long)]
        log: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum LockCommand {
    List,
    Acquire {
        resource: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        task: Option<String>,
    },
    Release {
        resource: String,
        #[arg(long)]
        owner: String,
    },
}

#[derive(Debug, Subcommand)]
enum ApprovalCommand {
    List,
    Request {
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        node: Option<String>,
        #[arg(long)]
        level: String,
        #[arg(long)]
        reason: Option<String>,
    },
    Approve {
        approval_id: String,
    },
    Deny {
        approval_id: String,
    },
}

#[derive(Debug, Subcommand)]
enum PluginCommand {
    List,
    Trust {
        plugin_id: String,
    },
    RunHook {
        plugin_id: String,
        hook: String,
        #[arg(last = true)]
        args: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum SkillCommand {
    List,
}

#[derive(Debug, Subcommand)]
enum WorktreeCommand {
    Create {
        task_id: String,
        #[arg(long)]
        agent: String,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum WorkerCommand {
    Register {
        id: String,
        #[arg(long)]
        endpoint: String,
        #[arg(long = "capability")]
        capabilities: Vec<String>,
    },
    List,
    Dispatch {
        worker_id: String,
        task_id: String,
    },
    RunNext(WorkerRunArgs),
    RunAll(WorkerRunArgs),
}

#[derive(Debug, Args)]
struct WorkerRunArgs {
    worker_id: String,
    task_id: String,
    #[arg(long, default_value = "fake")]
    adapter: String,
    #[arg(long = "require-lock")]
    required_locks: Vec<String>,
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum DashboardCommand {
    Export {
        #[arg(long)]
        out: PathBuf,
    },
    Serve {
        #[arg(long, default_value = "127.0.0.1:8765")]
        addr: String,
    },
}

#[derive(Debug, Subcommand)]
enum DaemonClientCommand {
    Health {
        #[arg(long)]
        socket: Option<PathBuf>,
    },
    TaskStatus {
        task_id: String,
        #[arg(long)]
        socket: Option<PathBuf>,
    },
    Locks {
        #[arg(long)]
        socket: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum ExportCommand {
    Otel {
        task_id: String,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum GatewayCommand {
    List,
    AcpStdio,
    A2aCard {
        #[arg(long, default_value = "http://127.0.0.1:8766")]
        base_url: String,
    },
    A2aServe {
        #[arg(long, default_value = "127.0.0.1:8766")]
        addr: String,
    },
}

#[derive(Debug, Subcommand)]
enum MarketplaceCommand {
    List,
    AddLocal { path: PathBuf },
    Install { plugin_id: String },
}

#[derive(Debug, Subcommand)]
enum CollaborationCommand {
    Start {
        #[arg(long, default_value = "hosted")]
        mode: String,
        #[arg(long)]
        endpoint: Option<String>,
    },
    Join {
        session_id: String,
        #[arg(long, default_value = "local")]
        participant: String,
    },
    List,
}

pub fn run() -> crate::Result<()> {
    let cli = Cli::parse();
    let home = Home::resolve(cli.home, cli.project)?;
    match cli.command {
        Commands::Init => init::run(home),
        Commands::Doctor => doctor(home),
        Commands::Agents { command } => agents(home, command),
        Commands::Task { command } => task(home, command),
        Commands::Run(args) => run_task(home, args),
        Commands::Workflow { command } => workflow(home, command),
        Commands::Locks { command } => lock(home, command),
        Commands::Approvals { command } => approval(home, command),
        Commands::Plugins { command } => plugin(home, command),
        Commands::Skills { command } => skill(home, command),
        Commands::Worktrees { command } => worktree(home, command),
        Commands::Workers { command } => worker(home, command),
        Commands::Dashboard { command } => dashboard_cmd(home, command),
        Commands::Daemon { command } => daemon_client(home, command),
        Commands::Export { command } => export(home, command),
        Commands::Gateways { command } => gateway(home, command),
        Commands::Marketplace { command } => marketplace_cmd(home, command),
        Commands::Collaboration { command } => collaboration_cmd(home, command),
    }
}

fn doctor(home: Home) -> crate::Result<()> {
    println!("home: {}", home.root().display());
    println!(
        "initialized: {}",
        home.config().exists() && home.db().exists()
    );
    let detected = adapters::detect_all();
    let installed = detected.iter().filter(|adapter| adapter.installed).count();
    println!("detected agents: {installed}/{}", detected.len());
    for adapter in detected {
        println!(
            "  {}: {} {}",
            adapter.id,
            if adapter.installed {
                "installed"
            } else {
                "missing"
            },
            adapter.version.unwrap_or_default()
        );
    }
    Ok(())
}

fn agents(home: Home, command: AgentCommand) -> crate::Result<()> {
    match command {
        AgentCommand::Detect => {
            home.require_initialized()?;
            let store = Store::open(home.clone())?;
            let detected = adapters::detect_all();
            std::fs::write(
                home.adapters_file(),
                serde_json::to_string_pretty(&detected)?,
            )?;
            for adapter in &detected {
                store.upsert_adapter(adapter)?;
            }
            print_adapters(&detected);
            Ok(())
        }
        AgentCommand::List => {
            home.require_initialized()?;
            let store = Store::open(home)?;
            print_adapters(&store.list_adapters()?);
            Ok(())
        }
        AgentCommand::OpencodeServePlan { hostname, port } => {
            let plan = opencode_http::serve_plan(&hostname, port);
            println!("{} {}", plan.program, plan.args.join(" "));
            println!(
                "openapi: {}",
                opencode_http::openapi_url(&format!("http://{hostname}:{port}"))
            );
            Ok(())
        }
        AgentCommand::OpencodeOpenapi { url } => {
            print!("{}", opencode_http::fetch_openapi(&url)?);
            Ok(())
        }
    }
}

fn task(home: Home, command: TaskCommand) -> crate::Result<()> {
    home.require_initialized()?;
    let store = Store::open(home.clone())?;
    match command {
        TaskCommand::Create(args) => {
            let goal = join_goal(args.goal)?;
            let nodes = workflows::nodes_for_home(&home, &args.workflow)?;
            let task_id = store.create_task(&goal, &args.workflow, nodes)?;
            println!("{task_id}");
            Ok(())
        }
        TaskCommand::Lease(args) => {
            if let Some(node) =
                store.lease_next_node(&args.task_or_node_id, &args.owner, args.ttl)?
            {
                println!("{} {}", node.id, node.name);
            } else {
                println!("no-runnable-node");
            }
            Ok(())
        }
        TaskCommand::Heartbeat(args) => {
            if store.heartbeat_node(&args.task_or_node_id, &args.owner, args.ttl)? {
                println!("heartbeat {}", args.task_or_node_id);
            } else {
                println!("not-running {}", args.task_or_node_id);
            }
            Ok(())
        }
        TaskCommand::RunNext(args) => {
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
        TaskCommand::RunAll(args) => {
            let count = runtime::run_all_command(
                &store,
                &args.task_id,
                &args.owner,
                &args.adapter,
                &args.command,
                runtime::RunOptions {
                    required_locks: args.required_locks,
                },
            )?;
            println!("ran {count} node(s)");
            Ok(())
        }
        TaskCommand::RunProviderNext(args) => {
            let command = provider_command(&store, &args)?;
            if let Some(node) = runtime::run_next_command(
                &store,
                &args.task_id,
                &args.owner,
                &args.adapter,
                &command,
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
        TaskCommand::RunProviderAll(args) => {
            let command = provider_command(&store, &args)?;
            let count = runtime::run_all_command(
                &store,
                &args.task_id,
                &args.owner,
                &args.adapter,
                &command,
                runtime::RunOptions {
                    required_locks: args.required_locks,
                },
            )?;
            println!("ran {count} node(s)");
            Ok(())
        }
        TaskCommand::ResumeProviderNext(args) => {
            let command = provider_resume_command(&store, &args)?;
            if let Some(node) = runtime::run_next_command(
                &store,
                &args.task_id,
                &args.owner,
                &args.adapter,
                &command,
                runtime::RunOptions {
                    required_locks: args.required_locks,
                },
            )? {
                println!("resumed {} {}", node.id, node.name);
            } else {
                println!("no-runnable-node");
            }
            Ok(())
        }
        TaskCommand::ResumeProviderAll(args) => {
            let command = provider_resume_command(&store, &args)?;
            let count = runtime::run_all_command(
                &store,
                &args.task_id,
                &args.owner,
                &args.adapter,
                &command,
                runtime::RunOptions {
                    required_locks: args.required_locks,
                },
            )?;
            println!("resumed {count} node(s)");
            Ok(())
        }
        TaskCommand::CapturePatch(args) => {
            let patch_id =
                patches::capture_git_diff(&store, &args.task_id, args.node.as_deref(), &args.repo)?;
            println!("{patch_id}");
            Ok(())
        }
        TaskCommand::Verify(args) => {
            if verify::run_verification(&store, &args.task_id, &args.command)? {
                println!("verification passed");
            } else {
                println!("verification failed");
            }
            Ok(())
        }
        TaskCommand::Complete { node_id } => {
            if store.finish_node(&node_id, "COMPLETED")? {
                println!("completed {node_id}");
            } else {
                println!("not-found {node_id}");
            }
            Ok(())
        }
        TaskCommand::Fail { node_id } => {
            if store.finish_node(&node_id, "FAILED")? {
                println!("failed {node_id}");
            } else {
                println!("not-found {node_id}");
            }
            Ok(())
        }
        TaskCommand::Retry { node_id } => {
            if store.retry_node(&node_id)? {
                println!("retrying {node_id}");
            } else {
                println!("not-retryable {node_id}");
            }
            Ok(())
        }
        TaskCommand::Cancel { task_id } => {
            if store.cancel_task(&task_id)? {
                println!("cancelled {task_id}");
            } else {
                println!("not-found {task_id}");
            }
            Ok(())
        }
        TaskCommand::Status { task_id } => tasks::print_status(&store, &task_id),
        TaskCommand::Events { task_id } => tasks::print_events(&store, &task_id),
    }
}

fn provider_command(store: &Store, args: &ProviderRunArgs) -> crate::Result<Vec<String>> {
    let prompt = if args.prompt.is_empty() {
        store
            .task(&args.task_id)?
            .map(|task| task.goal)
            .ok_or_else(|| anyhow::anyhow!("task not found: {}", args.task_id))?
    } else {
        args.prompt.join(" ")
    };
    let adapter = adapters::builtin_adapter(&args.adapter)
        .ok_or_else(|| anyhow::anyhow!("unknown adapter: {}", args.adapter))?;
    let plan = adapters::AgentAdapter::start(&adapter, &prompt);
    let mut command = vec![plan.program];
    command.extend(plan.args);
    Ok(command)
}

fn provider_resume_command(store: &Store, args: &ProviderResumeArgs) -> crate::Result<Vec<String>> {
    let prompt = if args.prompt.is_empty() {
        store
            .task(&args.task_id)?
            .map(|task| task.goal)
            .ok_or_else(|| anyhow::anyhow!("task not found: {}", args.task_id))?
    } else {
        args.prompt.join(" ")
    };
    let adapter = adapters::builtin_adapter(&args.adapter)
        .ok_or_else(|| anyhow::anyhow!("unknown adapter: {}", args.adapter))?;
    let plan = adapters::AgentAdapter::resume(&adapter, &args.session_id, &prompt)
        .ok_or_else(|| anyhow::anyhow!("adapter does not support resume: {}", args.adapter))?;
    let mut command = vec![plan.program];
    command.extend(plan.args);
    Ok(command)
}

fn run_task(home: Home, args: RunArgs) -> crate::Result<()> {
    home.require_initialized()?;
    let store = Store::open(home.clone())?;
    let goal = join_goal(args.goal)?;
    let nodes = workflows::nodes_for_home(&home, &args.workflow)?;
    let task_id = store.create_task(&goal, &args.workflow, nodes)?;
    store.record_event(
        Some(&task_id),
        None,
        "run.queued",
        json!({ "workflow": args.workflow, "execution": "manual lease pending" }),
    )?;
    println!("queued {task_id}");
    Ok(())
}

fn workflow(home: Home, command: WorkflowCommand) -> crate::Result<()> {
    match command {
        WorkflowCommand::List => {
            for name in workflows::list_names(&home)? {
                println!("{name}");
            }
            Ok(())
        }
        WorkflowCommand::Run { name, log } => {
            home.require_initialized()?;
            let store = Store::open(home.clone())?;
            let mut goal = format!("run workflow {name}");
            if let Some(log) = log {
                goal.push_str(&format!(" with log {}", log.display()));
            }
            let nodes = workflows::nodes_for_home(&home, &name)?;
            let task_id = store.create_task(&goal, &name, nodes)?;
            println!("{task_id}");
            Ok(())
        }
    }
}

fn lock(home: Home, command: LockCommand) -> crate::Result<()> {
    home.require_initialized()?;
    let store = Store::open(home)?;
    match command {
        LockCommand::List => {
            for (resource, owner, task_id, acquired_at) in store.locks()? {
                println!(
                    "{}",
                    locks::format_lock(&resource, &owner, task_id.as_deref(), acquired_at)
                );
            }
            Ok(())
        }
        LockCommand::Acquire {
            resource,
            owner,
            task,
        } => {
            if store.acquire_lock(&resource, &owner, task.as_deref())? {
                println!("acquired {resource}");
            } else {
                println!("busy {resource}");
            }
            Ok(())
        }
        LockCommand::Release { resource, owner } => {
            if store.release_lock(&resource, &owner)? {
                println!("released {resource}");
            } else {
                println!("not-held {resource}");
            }
            Ok(())
        }
    }
}

fn approval(home: Home, command: ApprovalCommand) -> crate::Result<()> {
    home.require_initialized()?;
    let store = Store::open(home)?;
    match command {
        ApprovalCommand::List => {
            for approval in store.approvals()? {
                println!("{}", approvals::format_approval(&approval));
            }
            Ok(())
        }
        ApprovalCommand::Request {
            task,
            node,
            level,
            reason,
        } => {
            approvals::validate_level(&level)?;
            let approval_id = store.request_approval(
                task.as_deref(),
                node.as_deref(),
                &level,
                reason.as_deref(),
            )?;
            println!("{approval_id}");
            Ok(())
        }
        ApprovalCommand::Approve { approval_id } => {
            if store.decide_approval(&approval_id, "APPROVED")? {
                println!("approved {approval_id}");
            } else {
                println!("not-found {approval_id}");
            }
            Ok(())
        }
        ApprovalCommand::Deny { approval_id } => {
            if store.decide_approval(&approval_id, "DENIED")? {
                println!("denied {approval_id}");
            } else {
                println!("not-found {approval_id}");
            }
            Ok(())
        }
    }
}

fn plugin(home: Home, command: PluginCommand) -> crate::Result<()> {
    match command {
        PluginCommand::List => {
            for plugin in plugins::list(&home)? {
                println!(
                    "{} {} trusted={} adapters={} skills={} workflows={} hooks={} path={}",
                    plugin.id,
                    plugin.version,
                    plugin.trusted,
                    plugin.capabilities.adapters.len(),
                    plugin.capabilities.skills.len(),
                    plugin.capabilities.workflows.len(),
                    plugin.capabilities.hooks.len(),
                    plugin.path.display()
                );
            }
            Ok(())
        }
        PluginCommand::Trust { plugin_id } => {
            home.require_initialized()?;
            plugins::trust(&home, &plugin_id)?;
            println!("trusted {plugin_id}");
            Ok(())
        }
        PluginCommand::RunHook {
            plugin_id,
            hook,
            args,
        } => {
            home.require_initialized()?;
            let output =
                plugins::run_hook(&home, std::path::Path::new("."), &plugin_id, &hook, &args)?;
            print!("{}", String::from_utf8_lossy(&output.stdout));
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            if !output.status.success() {
                bail!("hook exited with {}", output.status);
            }
            Ok(())
        }
    }
}

fn skill(home: Home, command: SkillCommand) -> crate::Result<()> {
    match command {
        SkillCommand::List => {
            let mut all = skills::list(&home)?;
            all.extend(skills::list_project()?);
            for skill in all {
                println!(
                    "{} {} path={}",
                    skill.id,
                    skill.version,
                    skill.path.display()
                );
            }
            Ok(())
        }
    }
}

fn worktree(home: Home, command: WorktreeCommand) -> crate::Result<()> {
    home.require_initialized()?;
    match command {
        WorktreeCommand::Create {
            task_id,
            agent,
            repo,
        } => {
            let path = worktrees::create(&home, &repo, &task_id, &agent)?;
            println!("{}", path.display());
            Ok(())
        }
    }
}

fn worker(home: Home, command: WorkerCommand) -> crate::Result<()> {
    home.require_initialized()?;
    let store = Store::open(home)?;
    match command {
        WorkerCommand::Register {
            id,
            endpoint,
            capabilities,
        } => {
            store.upsert_worker(&id, &endpoint, &capabilities)?;
            println!("registered {id}");
            Ok(())
        }
        WorkerCommand::List => workers::print_workers(&store),
        WorkerCommand::Dispatch { worker_id, task_id } => {
            if store.dispatch_worker(&worker_id, &task_id)? {
                println!("dispatched {task_id} to {worker_id}");
            } else {
                println!("not-found");
            }
            Ok(())
        }
        WorkerCommand::RunNext(args) => {
            if !store.dispatch_worker(&args.worker_id, &args.task_id)? {
                println!("not-found");
                return Ok(());
            }
            if let Some(node) = runtime::run_next_command(
                &store,
                &args.task_id,
                &args.worker_id,
                &args.adapter,
                &args.command,
                runtime::RunOptions {
                    required_locks: args.required_locks,
                },
            )? {
                println!("worker {} ran {} {}", args.worker_id, node.id, node.name);
            } else {
                println!("no-runnable-node");
            }
            Ok(())
        }
        WorkerCommand::RunAll(args) => {
            if !store.dispatch_worker(&args.worker_id, &args.task_id)? {
                println!("not-found");
                return Ok(());
            }
            let count = runtime::run_all_command(
                &store,
                &args.task_id,
                &args.worker_id,
                &args.adapter,
                &args.command,
                runtime::RunOptions {
                    required_locks: args.required_locks,
                },
            )?;
            println!("worker {} ran {count} node(s)", args.worker_id);
            Ok(())
        }
    }
}

fn dashboard_cmd(home: Home, command: DashboardCommand) -> crate::Result<()> {
    home.require_initialized()?;
    let store = Store::open(home)?;
    match command {
        DashboardCommand::Export { out } => {
            dashboard::export(&store, &out)?;
            println!("{}", out.display());
            Ok(())
        }
        DashboardCommand::Serve { addr } => dashboard::serve(&store, &addr),
    }
}

fn daemon_client(home: Home, command: DaemonClientCommand) -> crate::Result<()> {
    let (socket, request) = match command {
        DaemonClientCommand::Health { socket } => (
            daemon::socket_path(&home, socket),
            json!({ "command": "health" }),
        ),
        DaemonClientCommand::TaskStatus { task_id, socket } => (
            daemon::socket_path(&home, socket),
            json!({ "command": "task_status", "task_id": task_id }),
        ),
        DaemonClientCommand::Locks { socket } => (
            daemon::socket_path(&home, socket),
            json!({ "command": "locks" }),
        ),
    };
    let response = daemon::send_request(socket, request)?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

fn export(home: Home, command: ExportCommand) -> crate::Result<()> {
    home.require_initialized()?;
    match command {
        ExportCommand::Otel { task_id, out } => {
            otel::export_task(&home, &task_id, &out)?;
            println!("{}", out.display());
            Ok(())
        }
    }
}

fn gateway(home: Home, command: GatewayCommand) -> crate::Result<()> {
    match command {
        GatewayCommand::List => {
            for plan in gateways::plans() {
                println!(
                    "{}\t{}\tphase={}\treason={}",
                    plan.id, plan.status, plan.earliest_phase, plan.reason
                );
            }
            Ok(())
        }
        GatewayCommand::AcpStdio => {
            home.require_initialized()?;
            let store = Store::open(home)?;
            gateways::acp_stdio(&store)
        }
        GatewayCommand::A2aCard { base_url } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&gateways::a2a_agent_card(&base_url))?
            );
            Ok(())
        }
        GatewayCommand::A2aServe { addr } => {
            home.require_initialized()?;
            let store = Store::open(home)?;
            gateways::serve_a2a(&store, &addr)
        }
    }
}

fn marketplace_cmd(home: Home, command: MarketplaceCommand) -> crate::Result<()> {
    match command {
        MarketplaceCommand::List => {
            for entry in marketplace::list(&home)? {
                println!(
                    "{} {} {} {}",
                    entry.id, entry.version, entry.name, entry.source
                );
            }
            Ok(())
        }
        MarketplaceCommand::AddLocal { path } => {
            let entry = marketplace::add_local(&home, &path)?;
            println!("added {}", entry.id);
            Ok(())
        }
        MarketplaceCommand::Install { plugin_id } => {
            home.require_initialized()?;
            let path = marketplace::install(&home, &plugin_id)?;
            println!("{}", path.display());
            Ok(())
        }
    }
}

fn collaboration_cmd(home: Home, command: CollaborationCommand) -> crate::Result<()> {
    home.require_initialized()?;
    match command {
        CollaborationCommand::Start { mode, endpoint } => {
            let session = collaboration::start(&home, &mode, endpoint)?;
            println!("{}", session.id);
            Ok(())
        }
        CollaborationCommand::Join {
            session_id,
            participant,
        } => {
            let session = collaboration::join(&home, &session_id, &participant)?;
            println!("joined {}", session.id);
            Ok(())
        }
        CollaborationCommand::List => {
            for session in collaboration::list(&home)? {
                println!(
                    "{}\t{}\t{}\t{}",
                    session.id,
                    session.mode,
                    session.endpoint.unwrap_or_else(|| "-".to_string()),
                    session.participants.join(",")
                );
            }
            Ok(())
        }
    }
}

fn print_adapters(adapters: &[adapters::AdapterInfo]) {
    for adapter in adapters {
        println!(
            "{}\t{}\t{}\t{}",
            adapter.id,
            if adapter.installed {
                "installed"
            } else {
                "missing"
            },
            adapter.version.as_deref().unwrap_or("-"),
            adapter.control_surface
        );
    }
}

fn join_goal(parts: Vec<String>) -> crate::Result<String> {
    let goal = parts.join(" ");
    if goal.trim().is_empty() {
        bail!("goal is required");
    }
    Ok(goal)
}
