# zgent Coordination Model

`zgent` owns persistence and runtime state locally. The interactive entrypoint
uses a managed local daemon as the live coordinator for sessions, locks,
background provider runs, and multi-client status. State remains SQLite plus
JSONL on disk, so the daemon is a runtime coordinator rather than an external
database requirement. The extension boundary is intentionally narrow:

- provider adapters for Codex, Claude Code, Cursor Agent, opencode, and future CLIs
- plugins for installable capability bundles
- skills for scoped instructions
- hooks for trusted local automation
- workflow templates for task DAGs
- gateway surfaces for editor or remote-agent interoperability

## Persistence

By default, `zgent` writes to `~/.zgent`. Project-local mode writes to the
current repository's `.zgent` directory:

```bash
zgent init
zgent --project init
```

Runtime state is SQLite plus JSONL event logs. Project runtime directories such
as `.zgent/state`, `.zgent/tasks`, `.zgent/worktrees`, `.zgent/logs`, and
`.zgent/collaboration` are ignored by default. Declarative project assets such
as workflows, skills, policy, and plugins can be committed deliberately.

## Adapter Registration

Adapters are the provider backend abstraction. Built-in adapters are detected
during initialization and written as static TOML manifests under
`.zgent/adapters/*.toml`. Custom providers can be registered by adding another
manifest with command templates for start and resume.

```toml
id = "cursor"
kind = "provider"
command = "cursor-agent"
start_args = ["-p", "{prompt}", "--output-format", "stream-json"]
resume_args = ["--print", "--output-format", "stream-json", "--resume", "{session_id}", "{prompt}"]
output = "cursor-agent -p --output-format stream-json"
capabilities = ["detect", "start", "resume", "stream", "collect_result"]
permission_modes = ["review-first", "yolo"]
trusted = true
```

## Plugins And Skills

Plugins are local directories with `zgent.plugin.json`. User plugins installed
under `~/.zgent/plugins/installed` are trusted. Project plugins under
`.zgent/plugins` require explicit trust before hooks run.

The marketplace implementation is local-first: `marketplace add-local` indexes a
plugin directory, and `marketplace install` copies it into the installed plugin
directory. A hosted registry can be added later without changing plugin loading.

## Workers

Remote workers are registry rows in the local SQLite store. Registering a worker
records its endpoint and capabilities; dispatching marks the worker assigned and
records an event against the task. Worker `run-next` and `run-all` execute task
nodes under the worker identity through the existing runtime. Remote transport
can be layered behind worker endpoints through plugins or gateways.

```bash
zgent workers register worker-1 --endpoint ssh://worker --capability codex
zgent workers dispatch worker-1 <task-id>
zgent workers run-next worker-1 <task-id> --adapter fake -- /bin/sh -c "echo ok"
```

## Safety And Retry

Write-capable provider runs can require explicit locks with `--require-lock`.
Shell commands that match the local dangerous-command policy move the leased node
to `WAITING_APPROVAL` and create a `dangerous` approval request before execution.
Approving that request releases the node back to `PENDING`; failed or waiting
nodes can also be returned to the runnable queue explicitly:

```bash
zgent approvals approve <approval-id>
zgent task retry <node-id>
```

`review-first` is the default permission mode. `yolo` is an explicit opt-in mode
for a run/session that bypasses local lock and dangerous-command approval gates:

```bash
zgent task run-next <task-id> --permission-mode yolo -- /bin/sh -c "echo ok"
```

## Gateways

`zgent gateways acp-stdio` exposes a JSON-RPC stdio bridge for local clients.
It supports initialization, task creation/status through `_zgent/*` methods, and
session listing/cancellation mapped onto tasks.

`zgent gateways a2a-serve` exposes a small HTTP+JSON gateway:

- `GET /.well-known/agent-card.json`
- `POST /a2a/v1`

Incoming A2A `message/send` requests become ordinary durable `zgent` tasks.

## opencode HTTP

opencode can run as a headless HTTP server and publishes its OpenAPI 3.1
document at `/doc`. `zgent agents opencode-serve-plan` prints the command to
start that server; `zgent agents opencode-openapi` fetches the OpenAPI document
from a running server for SDK generation or inspection.

## Collaboration

Hosted collaboration is represented as local session records first:

```bash
zgent collaboration start --mode hosted --endpoint https://example.invalid
zgent collaboration join <session-id> --participant reviewer
```

This gives the CLI, state model, and audit trail a stable shape before adding
real multi-user networking.
