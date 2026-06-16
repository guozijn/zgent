use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{adapters::AdapterInfo, events::Event, home::Home};

pub struct Store {
    home: Home,
    conn: Connection,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskRow {
    pub id: String,
    pub goal: String,
    pub workflow: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeRow {
    pub id: String,
    pub task_id: String,
    pub name: String,
    pub role: String,
    pub state: String,
    pub depends_on: Vec<String>,
    pub agent: Option<String>,
    pub skill: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApprovalRow {
    pub id: String,
    pub task_id: Option<String>,
    pub node_id: Option<String>,
    pub level: String,
    pub status: String,
    pub reason: Option<String>,
    pub created_at: i64,
    pub decided_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerRow {
    pub id: String,
    pub endpoint: String,
    pub status: String,
    pub capabilities: Vec<String>,
    pub registered_at: i64,
    pub updated_at: i64,
}

pub type LockRow = (String, String, Option<String>, i64);

#[derive(Debug, Clone)]
pub struct NodeSpec {
    pub name: String,
    pub role: String,
    pub depends_on: Vec<String>,
    pub agent: Option<String>,
    pub skill: Option<String>,
}

impl NodeSpec {
    pub fn new(name: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            role: role.into(),
            depends_on: Vec::new(),
            agent: None,
            skill: None,
        }
    }

    pub fn depends_on(mut self, deps: &[&str]) -> Self {
        self.depends_on = deps.iter().map(|dep| (*dep).to_string()).collect();
        self
    }

    pub fn agent(mut self, agent: impl Into<String>) -> Self {
        self.agent = Some(agent.into());
        self
    }

    pub fn skill(mut self, skill: impl Into<String>) -> Self {
        self.skill = Some(skill.into());
        self
    }
}

impl Store {
    pub fn open(home: Home) -> crate::Result<Self> {
        if let Some(parent) = home.db().parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(home.db())?;
        let store = Self { home, conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn home(&self) -> &Home {
        &self.home
    }

    pub fn init_schema(&self) -> crate::Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                goal TEXT NOT NULL,
                workflow TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS task_nodes (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                role TEXT NOT NULL,
                state TEXT NOT NULL,
                depends_on TEXT NOT NULL,
                agent TEXT,
                skill TEXT,
                lease_owner TEXT,
                leased_until INTEGER,
                attempts INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                task_id TEXT,
                node_id TEXT,
                kind TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS agent_profiles (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                adapter TEXT,
                config TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS adapter_capabilities (
                id TEXT PRIMARY KEY,
                command TEXT NOT NULL,
                installed INTEGER NOT NULL,
                binary TEXT,
                version TEXT,
                auth_status TEXT NOT NULL,
                control_surface TEXT NOT NULL,
                priority TEXT NOT NULL,
                modes TEXT NOT NULL,
                detected_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS agent_sessions (
                id TEXT PRIMARY KEY,
                adapter TEXT NOT NULL,
                task_id TEXT,
                node_id TEXT,
                provider_session_id TEXT,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS resource_locks (
                resource TEXT PRIMARY KEY,
                owner TEXT NOT NULL,
                task_id TEXT,
                acquired_at INTEGER NOT NULL,
                expires_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS artifacts (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                node_id TEXT,
                kind TEXT NOT NULL,
                path TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS patches (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                node_id TEXT,
                path TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS approvals (
                id TEXT PRIMARY KEY,
                task_id TEXT,
                node_id TEXT,
                level TEXT NOT NULL,
                status TEXT NOT NULL,
                reason TEXT,
                created_at INTEGER NOT NULL,
                decided_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS plugins (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                path TEXT NOT NULL,
                trusted INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                version TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS runs (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                workflow TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS remote_workers (
                id TEXT PRIMARY KEY,
                endpoint TEXT NOT NULL,
                status TEXT NOT NULL,
                capabilities TEXT NOT NULL,
                registered_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            ",
        )?;
        self.ensure_column("task_nodes", "skill", "TEXT")?;
        Ok(())
    }

    fn ensure_column(&self, table: &str, column: &str, ty: &str) -> crate::Result<()> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for name in rows {
            if name? == column {
                return Ok(());
            }
        }
        self.conn
            .execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {ty}"), [])?;
        Ok(())
    }

    pub fn upsert_adapter(&self, adapter: &AdapterInfo) -> crate::Result<()> {
        let now = crate::events::now();
        self.conn.execute(
            "
            INSERT INTO adapter_capabilities
                (id, command, installed, binary, version, auth_status, control_surface, priority, modes, detected_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                command = excluded.command,
                installed = excluded.installed,
                binary = excluded.binary,
                version = excluded.version,
                auth_status = excluded.auth_status,
                control_surface = excluded.control_surface,
                priority = excluded.priority,
                modes = excluded.modes,
                detected_at = excluded.detected_at
            ",
            params![
                adapter.id,
                adapter.command,
                adapter.installed as i64,
                adapter.binary,
                adapter.version,
                adapter.auth_status,
                adapter.control_surface,
                adapter.priority,
                serde_json::to_string(&adapter.modes)?,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn list_adapters(&self) -> crate::Result<Vec<AdapterInfo>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, command, installed, binary, version, auth_status, control_surface, priority, modes
            FROM adapter_capabilities
            ORDER BY priority, id
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            let modes: String = row.get(8)?;
            Ok(AdapterInfo {
                id: row.get(0)?,
                command: row.get(1)?,
                installed: row.get::<_, i64>(2)? == 1,
                binary: row.get(3)?,
                version: row.get(4)?,
                auth_status: row.get(5)?,
                control_surface: row.get(6)?,
                priority: row.get(7)?,
                modes: serde_json::from_str(&modes).unwrap_or_default(),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn upsert_agent_profile(
        &self,
        id: &str,
        kind: &str,
        adapter: Option<&str>,
        config: Value,
    ) -> crate::Result<()> {
        self.conn.execute(
            "
            INSERT INTO agent_profiles (id, kind, adapter, config, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, adapter = excluded.adapter, config = excluded.config
            ",
            params![
                id,
                kind,
                adapter,
                serde_json::to_string_pretty(&config)?,
                crate::events::now()
            ],
        )?;
        Ok(())
    }

    pub fn create_task(
        &self,
        goal: &str,
        workflow: &str,
        nodes: Vec<NodeSpec>,
    ) -> crate::Result<String> {
        let now = crate::events::now();
        let task_id = format!("task-{}", Uuid::new_v4());
        fs::create_dir_all(self.home.task_dir(&task_id).join("artifacts"))?;
        fs::create_dir_all(self.home.task_dir(&task_id).join("patches"))?;
        fs::create_dir_all(self.home.task_dir(&task_id).join("transcripts"))?;
        fs::write(
            self.home.task_dir(&task_id).join("task.toml"),
            format!(
                "id = \"{task_id}\"\nworkflow = \"{workflow}\"\ngoal = \"{}\"\n",
                escape_toml(goal)
            ),
        )?;
        self.conn.execute(
            "INSERT INTO tasks (id, goal, workflow, status, created_at, updated_at) VALUES (?1, ?2, ?3, 'PENDING', ?4, ?4)",
            params![task_id, goal, workflow, now],
        )?;
        for node in nodes {
            let node_id = format!("node-{}", Uuid::new_v4());
            self.conn.execute(
                "
                INSERT INTO task_nodes (id, task_id, name, role, state, depends_on, agent, skill)
                VALUES (?1, ?2, ?3, ?4, 'PENDING', ?5, ?6, ?7)
                ",
                params![
                    node_id,
                    task_id,
                    node.name,
                    node.role,
                    serde_json::to_string(&node.depends_on)?,
                    node.agent,
                    node.skill,
                ],
            )?;
        }
        self.record_event(
            Some(&task_id),
            None,
            "task.created",
            json!({ "goal": goal, "workflow": workflow }),
        )?;
        Ok(task_id)
    }

    pub fn task(&self, task_id: &str) -> crate::Result<Option<TaskRow>> {
        self.conn
            .query_row(
                "SELECT id, goal, workflow, status, created_at, updated_at FROM tasks WHERE id = ?1",
                [task_id],
                |row| {
                    Ok(TaskRow {
                        id: row.get(0)?,
                        goal: row.get(1)?,
                        workflow: row.get(2)?,
                        status: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn tasks(&self) -> crate::Result<Vec<TaskRow>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, goal, workflow, status, created_at, updated_at
            FROM tasks
            ORDER BY created_at DESC, id
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TaskRow {
                id: row.get(0)?,
                goal: row.get(1)?,
                workflow: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn nodes(&self, task_id: &str) -> crate::Result<Vec<NodeRow>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, task_id, name, role, state, depends_on, agent, skill
            FROM task_nodes
            WHERE task_id = ?1
            ORDER BY rowid
            ",
        )?;
        let rows = stmt.query_map([task_id], |row| {
            let depends_on: String = row.get(5)?;
            Ok(NodeRow {
                id: row.get(0)?,
                task_id: row.get(1)?,
                name: row.get(2)?,
                role: row.get(3)?,
                state: row.get(4)?,
                depends_on: serde_json::from_str(&depends_on).unwrap_or_default(),
                agent: row.get(6)?,
                skill: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn node(&self, node_id: &str) -> crate::Result<Option<NodeRow>> {
        self.conn
            .query_row(
                "
                SELECT id, task_id, name, role, state, depends_on, agent, skill
                FROM task_nodes
                WHERE id = ?1
                ",
                [node_id],
                |row| {
                    let depends_on: String = row.get(5)?;
                    Ok(NodeRow {
                        id: row.get(0)?,
                        task_id: row.get(1)?,
                        name: row.get(2)?,
                        role: row.get(3)?,
                        state: row.get(4)?,
                        depends_on: serde_json::from_str(&depends_on).unwrap_or_default(),
                        agent: row.get(6)?,
                        skill: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn lease_next_node(
        &self,
        task_id: &str,
        owner: &str,
        ttl_secs: i64,
    ) -> crate::Result<Option<NodeRow>> {
        let nodes = self.nodes(task_id)?;
        let completed: HashSet<_> = nodes
            .iter()
            .filter(|node| node.state == "COMPLETED")
            .map(|node| node.name.as_str())
            .collect();
        let Some(node) = nodes.iter().find(|node| {
            node.state == "PENDING"
                && node
                    .depends_on
                    .iter()
                    .all(|dep| completed.contains(dep.as_str()))
        }) else {
            return Ok(None);
        };
        self.conn.execute(
            "
            UPDATE task_nodes
            SET state = 'RUNNING', lease_owner = ?1, leased_until = ?2, attempts = attempts + 1
            WHERE id = ?3 AND state = 'PENDING'
            ",
            params![owner, crate::events::now() + ttl_secs, node.id],
        )?;
        self.conn.execute(
            "UPDATE tasks SET status = 'RUNNING', updated_at = ?1 WHERE id = ?2 AND status = 'PENDING'",
            params![crate::events::now(), task_id],
        )?;
        self.record_event(
            Some(task_id),
            Some(&node.id),
            "node.leased",
            json!({ "owner": owner, "ttl_secs": ttl_secs, "node": node.name }),
        )?;
        self.node(&node.id)
    }

    pub fn heartbeat_node(&self, node_id: &str, owner: &str, ttl_secs: i64) -> crate::Result<bool> {
        let changed = self.conn.execute(
            "
            UPDATE task_nodes
            SET leased_until = ?1
            WHERE id = ?2 AND lease_owner = ?3 AND state = 'RUNNING'
            ",
            params![crate::events::now() + ttl_secs, node_id, owner],
        )?;
        if changed == 1 {
            let node = self.node(node_id)?.expect("updated node should exist");
            self.record_event(
                Some(&node.task_id),
                Some(node_id),
                "node.heartbeat",
                json!({ "owner": owner, "ttl_secs": ttl_secs }),
            )?;
        }
        Ok(changed == 1)
    }

    pub fn finish_node(&self, node_id: &str, state: &str) -> crate::Result<bool> {
        let Some(node) = self.node(node_id)? else {
            return Ok(false);
        };
        let changed = self.conn.execute(
            "UPDATE task_nodes SET state = ?1, lease_owner = NULL, leased_until = NULL WHERE id = ?2",
            params![state, node_id],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        self.record_event(
            Some(&node.task_id),
            Some(node_id),
            if state == "COMPLETED" {
                "node.completed"
            } else {
                "node.failed"
            },
            json!({ "node": node.name, "state": state }),
        )?;
        self.refresh_task_state(&node.task_id)?;
        Ok(true)
    }

    pub fn wait_for_approval(&self, node_id: &str, reason: &str) -> crate::Result<bool> {
        let Some(node) = self.node(node_id)? else {
            return Ok(false);
        };
        let changed = self.conn.execute(
            "
            UPDATE task_nodes
            SET state = 'WAITING_APPROVAL', lease_owner = NULL, leased_until = NULL
            WHERE id = ?1 AND state = 'RUNNING'
            ",
            [node_id],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        self.record_event(
            Some(&node.task_id),
            Some(node_id),
            "node.waiting_approval",
            json!({ "node": node.name, "reason": reason }),
        )?;
        self.refresh_task_state(&node.task_id)?;
        Ok(true)
    }

    pub fn retry_node(&self, node_id: &str) -> crate::Result<bool> {
        let Some(node) = self.node(node_id)? else {
            return Ok(false);
        };
        let changed = self.conn.execute(
            "
            UPDATE task_nodes
            SET state = 'PENDING', lease_owner = NULL, leased_until = NULL
            WHERE id = ?1 AND state IN ('FAILED', 'BLOCKED', 'WAITING_APPROVAL', 'CANCELLED')
            ",
            [node_id],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        self.record_event(
            Some(&node.task_id),
            Some(node_id),
            "node.retried",
            json!({ "node": node.name }),
        )?;
        self.refresh_task_state(&node.task_id)?;
        Ok(true)
    }

    pub fn cancel_task(&self, task_id: &str) -> crate::Result<bool> {
        if self.task(task_id)?.is_none() {
            return Ok(false);
        }
        self.conn.execute(
            "
            UPDATE task_nodes
            SET state = 'CANCELLED', lease_owner = NULL, leased_until = NULL
            WHERE task_id = ?1 AND state IN ('PENDING', 'RUNNING', 'BLOCKED', 'WAITING_APPROVAL')
            ",
            [task_id],
        )?;
        self.conn.execute(
            "UPDATE tasks SET status = 'CANCELLED', updated_at = ?1 WHERE id = ?2",
            params![crate::events::now(), task_id],
        )?;
        self.record_event(Some(task_id), None, "task.cancelled", json!({}))?;
        Ok(true)
    }

    pub fn record_event(
        &self,
        task_id: Option<&str>,
        node_id: Option<&str>,
        kind: &str,
        payload: Value,
    ) -> crate::Result<Event> {
        let event = Event::new(task_id, node_id, kind, crate::redact::redact_value(payload));
        self.conn.execute(
            "INSERT INTO events (id, task_id, node_id, kind, payload, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.id,
                event.task_id,
                event.node_id,
                event.kind,
                serde_json::to_string(&event.payload)?,
                event.created_at
            ],
        )?;
        append_jsonl(&self.home.global_events(), &event)?;
        if let Some(task_id) = task_id {
            fs::create_dir_all(self.home.task_dir(task_id))?;
            append_jsonl(&self.home.task_events(task_id), &event)?;
        }
        Ok(event)
    }

    pub fn task_event_count(&self, task_id: &str) -> crate::Result<i64> {
        self.conn
            .query_row(
                "SELECT count(*) FROM events WHERE task_id = ?1",
                [task_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn refresh_task_state(&self, task_id: &str) -> crate::Result<()> {
        let nodes = self.nodes(task_id)?;
        let state = if nodes.iter().any(|node| node.state == "FAILED") {
            "FAILED"
        } else if !nodes.is_empty() && nodes.iter().all(|node| node.state == "COMPLETED") {
            "COMPLETED"
        } else if nodes.iter().any(|node| node.state == "RUNNING") {
            "RUNNING"
        } else if nodes.iter().any(|node| node.state == "WAITING_APPROVAL") {
            "WAITING_APPROVAL"
        } else if nodes.iter().any(|node| node.state == "BLOCKED") {
            "BLOCKED"
        } else {
            "PENDING"
        };
        self.conn.execute(
            "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![state, crate::events::now(), task_id],
        )?;
        if matches!(state, "COMPLETED" | "FAILED") {
            self.record_event(
                Some(task_id),
                None,
                if state == "COMPLETED" {
                    "task.completed"
                } else {
                    "task.failed"
                },
                json!({ "status": state }),
            )?;
        }
        Ok(())
    }

    pub fn acquire_lock(
        &self,
        resource: &str,
        owner: &str,
        task_id: Option<&str>,
    ) -> crate::Result<bool> {
        let changed = self.conn.execute(
            "
            INSERT OR IGNORE INTO resource_locks (resource, owner, task_id, acquired_at)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![resource, owner, task_id, crate::events::now()],
        )?;
        if changed == 1 {
            self.record_event(
                task_id,
                None,
                "lock.acquired",
                json!({ "resource": resource, "owner": owner }),
            )?;
        }
        Ok(changed == 1)
    }

    pub fn release_lock(&self, resource: &str, owner: &str) -> crate::Result<bool> {
        let task_id: Option<String> = self
            .conn
            .query_row(
                "SELECT task_id FROM resource_locks WHERE resource = ?1 AND owner = ?2",
                params![resource, owner],
                |row| row.get(0),
            )
            .optional()?;
        let changed = self.conn.execute(
            "DELETE FROM resource_locks WHERE resource = ?1 AND owner = ?2",
            params![resource, owner],
        )?;
        if changed == 1 {
            self.record_event(
                task_id.as_deref(),
                None,
                "lock.released",
                json!({ "resource": resource, "owner": owner }),
            )?;
        }
        Ok(changed == 1)
    }

    pub fn locks(&self) -> crate::Result<Vec<LockRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT resource, owner, task_id, acquired_at FROM resource_locks ORDER BY resource",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn lock_held(&self, resource: &str, owner: &str) -> crate::Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM resource_locks WHERE resource = ?1 AND owner = ?2",
            params![resource, owner],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn sync_skill_index(&self, skills: &[crate::skills::SkillInfo]) -> crate::Result<()> {
        let now = crate::events::now();
        for skill in skills {
            self.conn.execute(
                "
                INSERT INTO skills (id, name, path, version, created_at)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(id) DO UPDATE SET name = excluded.name, path = excluded.path, version = excluded.version
                ",
                params![skill.id, skill.name, skill.path.display().to_string(), skill.version, now],
            )?;
        }
        Ok(())
    }

    pub fn record_artifact(
        &self,
        task_id: &str,
        node_id: Option<&str>,
        kind: &str,
        path: &std::path::Path,
    ) -> crate::Result<String> {
        let id = format!("artifact-{}", Uuid::new_v4());
        self.conn.execute(
            "
            INSERT INTO artifacts (id, task_id, node_id, kind, path, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                id,
                task_id,
                node_id,
                kind,
                path.display().to_string(),
                crate::events::now()
            ],
        )?;
        self.record_event(
            Some(task_id),
            node_id,
            "artifact.created",
            json!({ "id": id, "kind": kind, "path": path }),
        )?;
        Ok(id)
    }

    pub fn create_session(
        &self,
        adapter: &str,
        task_id: Option<&str>,
        node_id: Option<&str>,
        provider_session_id: Option<&str>,
    ) -> crate::Result<String> {
        let id = format!("session-{}", Uuid::new_v4());
        let now = crate::events::now();
        self.conn.execute(
            "
            INSERT INTO agent_sessions
                (id, adapter, task_id, node_id, provider_session_id, status, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, 'RUNNING', ?6, ?6)
            ",
            params![id, adapter, task_id, node_id, provider_session_id, now],
        )?;
        self.record_event(
            task_id,
            node_id,
            "session.started",
            json!({ "id": id, "adapter": adapter, "provider_session_id": provider_session_id }),
        )?;
        Ok(id)
    }

    pub fn update_session(&self, session_id: &str, status: &str) -> crate::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE agent_sessions SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, crate::events::now(), session_id],
        )?;
        Ok(changed == 1)
    }

    pub fn update_session_provider_id(
        &self,
        session_id: &str,
        provider_session_id: &str,
    ) -> crate::Result<bool> {
        let changed = self.conn.execute(
            "
            UPDATE agent_sessions
            SET provider_session_id = ?1, updated_at = ?2
            WHERE id = ?3 AND provider_session_id IS NULL
            ",
            params![provider_session_id, crate::events::now(), session_id],
        )?;
        Ok(changed == 1)
    }

    pub fn session_provider_ids(&self, task_id: &str) -> crate::Result<Vec<Option<String>>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT provider_session_id
            FROM agent_sessions
            WHERE task_id = ?1
            ORDER BY created_at, id
            ",
        )?;
        let rows = stmt.query_map([task_id], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn upsert_worker(
        &self,
        id: &str,
        endpoint: &str,
        capabilities: &[String],
    ) -> crate::Result<()> {
        let now = crate::events::now();
        self.conn.execute(
            "
            INSERT INTO remote_workers
                (id, endpoint, status, capabilities, registered_at, updated_at)
            VALUES (?1, ?2, 'available', ?3, ?4, ?4)
            ON CONFLICT(id) DO UPDATE SET
                endpoint = excluded.endpoint,
                status = 'available',
                capabilities = excluded.capabilities,
                updated_at = excluded.updated_at
            ",
            params![id, endpoint, serde_json::to_string(capabilities)?, now],
        )?;
        self.record_event(
            None,
            None,
            "worker.registered",
            json!({ "id": id, "endpoint": endpoint, "capabilities": capabilities }),
        )?;
        Ok(())
    }

    pub fn workers(&self) -> crate::Result<Vec<WorkerRow>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, endpoint, status, capabilities, registered_at, updated_at
            FROM remote_workers
            ORDER BY id
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            let capabilities: String = row.get(3)?;
            Ok(WorkerRow {
                id: row.get(0)?,
                endpoint: row.get(1)?,
                status: row.get(2)?,
                capabilities: serde_json::from_str(&capabilities).unwrap_or_default(),
                registered_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn dispatch_worker(&self, worker_id: &str, task_id: &str) -> crate::Result<bool> {
        let worker_exists: i64 = self.conn.query_row(
            "SELECT count(*) FROM remote_workers WHERE id = ?1",
            [worker_id],
            |row| row.get(0),
        )?;
        if worker_exists == 0 || self.task(task_id)?.is_none() {
            return Ok(false);
        }
        self.conn.execute(
            "UPDATE remote_workers SET status = 'assigned', updated_at = ?1 WHERE id = ?2",
            params![crate::events::now(), worker_id],
        )?;
        self.record_event(
            Some(task_id),
            None,
            "worker.dispatched",
            json!({ "worker_id": worker_id }),
        )?;
        Ok(true)
    }

    pub fn record_patch(
        &self,
        task_id: &str,
        node_id: Option<&str>,
        path: &std::path::Path,
        status: &str,
    ) -> crate::Result<String> {
        let id = format!("patch-{}", Uuid::new_v4());
        self.conn.execute(
            "
            INSERT INTO patches (id, task_id, node_id, path, status, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                id,
                task_id,
                node_id,
                path.display().to_string(),
                status,
                crate::events::now()
            ],
        )?;
        self.record_event(
            Some(task_id),
            node_id,
            "patch.created",
            json!({ "id": id, "path": path, "status": status }),
        )?;
        Ok(id)
    }

    pub fn request_approval(
        &self,
        task_id: Option<&str>,
        node_id: Option<&str>,
        level: &str,
        reason: Option<&str>,
    ) -> crate::Result<String> {
        let id = format!("approval-{}", Uuid::new_v4());
        self.conn.execute(
            "
            INSERT INTO approvals (id, task_id, node_id, level, status, reason, created_at)
            VALUES (?1, ?2, ?3, ?4, 'PENDING', ?5, ?6)
            ",
            params![id, task_id, node_id, level, reason, crate::events::now()],
        )?;
        self.record_event(
            task_id,
            node_id,
            "approval.requested",
            json!({ "id": id, "level": level, "reason": reason }),
        )?;
        Ok(id)
    }

    pub fn decide_approval(&self, approval_id: &str, status: &str) -> crate::Result<bool> {
        let approval = self.approval(approval_id)?;
        let Some(approval) = approval else {
            return Ok(false);
        };
        self.conn.execute(
            "UPDATE approvals SET status = ?1, decided_at = ?2 WHERE id = ?3",
            params![status, crate::events::now(), approval_id],
        )?;
        self.record_event(
            approval.task_id.as_deref(),
            approval.node_id.as_deref(),
            if status == "APPROVED" {
                "approval.granted"
            } else {
                "approval.denied"
            },
            json!({ "id": approval_id, "level": approval.level }),
        )?;
        if status == "APPROVED"
            && let Some(node_id) = approval.node_id.as_deref()
        {
            let _ = self.retry_node(node_id)?;
        }
        Ok(true)
    }

    pub fn has_approved_approval(
        &self,
        task_id: &str,
        node_id: Option<&str>,
        level: &str,
    ) -> crate::Result<bool> {
        let count: i64 = if let Some(node_id) = node_id {
            self.conn.query_row(
                "
                SELECT count(*)
                FROM approvals
                WHERE status = 'APPROVED'
                  AND level = ?1
                  AND task_id = ?2
                  AND (node_id = ?3 OR node_id IS NULL)
                ",
                params![level, task_id, node_id],
                |row| row.get(0),
            )?
        } else {
            self.conn.query_row(
                "
                SELECT count(*)
                FROM approvals
                WHERE status = 'APPROVED'
                  AND level = ?1
                  AND task_id = ?2
                  AND node_id IS NULL
                ",
                params![level, task_id],
                |row| row.get(0),
            )?
        };
        Ok(count > 0)
    }

    pub fn approval(&self, approval_id: &str) -> crate::Result<Option<ApprovalRow>> {
        self.conn
            .query_row(
                "
                SELECT id, task_id, node_id, level, status, reason, created_at, decided_at
                FROM approvals
                WHERE id = ?1
                ",
                [approval_id],
                approval_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn approvals(&self) -> crate::Result<Vec<ApprovalRow>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, task_id, node_id, level, status, reason, created_at, decided_at
            FROM approvals
            ORDER BY created_at DESC, id
            ",
        )?;
        let rows = stmt.query_map([], approval_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn approval_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ApprovalRow> {
    Ok(ApprovalRow {
        id: row.get(0)?,
        task_id: row.get(1)?,
        node_id: row.get(2)?,
        level: row.get(3)?,
        status: row.get(4)?,
        reason: row.get(5)?,
        created_at: row.get(6)?,
        decided_at: row.get(7)?,
    })
}

fn append_jsonl(path: &std::path::Path, value: &impl Serialize) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn escape_toml(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{home::Home, state::NodeSpec};

    #[test]
    fn creates_task_nodes_events_and_locks() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = super::Store::open(home.clone()).unwrap();
        let task_id = store
            .create_task(
                "test task",
                "plan-only",
                vec![NodeSpec::new("plan", "planner")],
            )
            .unwrap();
        store
            .record_event(Some(&task_id), None, "test.event", json!({ "ok": true }))
            .unwrap();
        assert_eq!(store.nodes(&task_id).unwrap().len(), 1);
        assert_eq!(store.task_event_count(&task_id).unwrap(), 2);
        assert!(
            store
                .acquire_lock("file:src/lib.rs", "tester", Some(&task_id))
                .unwrap()
        );
        assert!(
            !store
                .acquire_lock("file:src/lib.rs", "other", Some(&task_id))
                .unwrap()
        );
        assert!(store.release_lock("file:src/lib.rs", "tester").unwrap());
    }

    #[test]
    fn leases_and_completes_runnable_nodes() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = super::Store::open(home).unwrap();
        let task_id = store
            .create_task(
                "test task",
                "two-step",
                vec![
                    NodeSpec::new("first", "worker"),
                    NodeSpec::new("second", "worker").depends_on(&["first"]),
                ],
            )
            .unwrap();
        let first = store
            .lease_next_node(&task_id, "tester", 30)
            .unwrap()
            .expect("first node");
        assert_eq!(first.name, "first");
        assert!(store.heartbeat_node(&first.id, "tester", 30).unwrap());
        assert!(store.finish_node(&first.id, "COMPLETED").unwrap());
        let second = store
            .lease_next_node(&task_id, "tester", 30)
            .unwrap()
            .expect("second node");
        assert_eq!(second.name, "second");
        assert!(store.finish_node(&second.id, "COMPLETED").unwrap());
        assert_eq!(store.task(&task_id).unwrap().unwrap().status, "COMPLETED");
    }

    #[test]
    fn records_approval_decisions() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = super::Store::open(home).unwrap();
        let approval_id = store
            .request_approval(None, None, "dangerous", Some("test"))
            .unwrap();
        assert_eq!(
            store.approval(&approval_id).unwrap().unwrap().status,
            "PENDING"
        );
        assert!(store.decide_approval(&approval_id, "APPROVED").unwrap());
        assert_eq!(
            store.approval(&approval_id).unwrap().unwrap().status,
            "APPROVED"
        );
    }

    #[test]
    fn records_agent_session_lifecycle() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = super::Store::open(home).unwrap();
        let session_id = store
            .create_session("fake", None, None, Some("provider-1"))
            .unwrap();
        assert!(session_id.starts_with("session-"));
        assert!(store.update_session(&session_id, "COMPLETED").unwrap());
        assert!(
            !store
                .update_session_provider_id(&session_id, "provider-2")
                .unwrap()
        );
    }

    #[test]
    fn cancels_task_and_open_nodes() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = super::Store::open(home).unwrap();
        let task_id = store
            .create_task(
                "cancel",
                "two-step",
                vec![
                    NodeSpec::new("first", "worker"),
                    NodeSpec::new("second", "worker").depends_on(&["first"]),
                ],
            )
            .unwrap();
        assert!(store.cancel_task(&task_id).unwrap());
        assert_eq!(store.task(&task_id).unwrap().unwrap().status, "CANCELLED");
        assert!(
            store
                .nodes(&task_id)
                .unwrap()
                .iter()
                .all(|node| node.state == "CANCELLED")
        );
    }

    #[test]
    fn registers_and_dispatches_remote_worker() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        let store = super::Store::open(home).unwrap();
        let task_id = store
            .create_task(
                "remote",
                "plan-only",
                vec![NodeSpec::new("plan", "planner")],
            )
            .unwrap();
        store
            .upsert_worker("worker-1", "ssh://worker", &["codex".to_string()])
            .unwrap();
        assert_eq!(store.workers().unwrap().len(), 1);
        assert!(store.dispatch_worker("worker-1", &task_id).unwrap());
        assert_eq!(store.workers().unwrap()[0].status, "assigned");
    }
}
