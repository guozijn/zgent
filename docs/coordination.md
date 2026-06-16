# zgent Coordination Model

`zgent` owns persistence and runtime state directly. There is no external state
daemon requirement. The extension boundary is intentionally narrow:

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
records an event against the task. Execution transport remains a plugin or
gateway concern.

```bash
zgent workers register worker-1 --endpoint ssh://worker --capability codex
zgent workers dispatch worker-1 <task-id>
```

## Gateways

`zgent gateways acp-stdio` exposes a JSON-RPC stdio bridge for local clients.
It supports initialization, task creation/status through `_zgent/*` methods, and
session listing/cancellation mapped onto tasks.

`zgent gateways a2a-serve` exposes a small HTTP+JSON gateway:

- `GET /.well-known/agent-card.json`
- `POST /a2a/v1`

Incoming A2A `message/send` requests become ordinary durable `zgent` tasks.

## Collaboration

Hosted collaboration is represented as local session records first:

```bash
zgent collaboration start --mode hosted --endpoint https://example.invalid
zgent collaboration join <session-id> --participant reviewer
```

This gives the CLI, state model, and audit trail a stable shape before adding
real multi-user networking.
