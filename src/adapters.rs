use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    pub id: String,
    pub command: String,
    pub binary: Option<String>,
    pub installed: bool,
    pub version: Option<String>,
    pub auth_status: String,
    pub control_surface: String,
    pub priority: String,
    pub modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandPlan {
    pub adapter: String,
    pub program: String,
    pub args: Vec<String>,
}

pub trait AgentAdapter {
    fn detect(&self) -> AdapterInfo;
    fn start(&self, prompt: &str) -> CommandPlan;
    fn resume(&self, _session_id: &str, _prompt: &str) -> Option<CommandPlan> {
        None
    }
    fn cancel(&self, _session_id: &str) -> Option<CommandPlan> {
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct AdapterSpec {
    id: &'static str,
    command: &'static str,
    control_surface: &'static str,
    priority: &'static str,
    auth_file: Option<&'static str>,
}

const SPECS: &[AdapterSpec] = &[
    AdapterSpec {
        id: "codex",
        command: "codex",
        control_surface: "codex exec --json",
        priority: "P0",
        auth_file: Some(".codex/auth.json"),
    },
    AdapterSpec {
        id: "claude",
        command: "claude",
        control_surface: "claude -p --output-format stream-json",
        priority: "P0",
        auth_file: Some(".claude.json"),
    },
    AdapterSpec {
        id: "opencode",
        command: "opencode",
        control_surface: "opencode run --format json",
        priority: "P0",
        auth_file: Some(".local/share/opencode/auth.json"),
    },
    AdapterSpec {
        id: "cursor",
        command: "cursor-agent",
        control_surface: "cursor-agent -p --output-format stream-json",
        priority: "P1",
        auth_file: None,
    },
];

#[derive(Debug, Clone, Copy)]
pub struct BuiltinAdapter {
    spec: AdapterSpec,
}

impl AgentAdapter for BuiltinAdapter {
    fn detect(&self) -> AdapterInfo {
        detect(&self.spec)
    }

    fn start(&self, prompt: &str) -> CommandPlan {
        let args = match self.spec.id {
            "codex" => vec!["exec", "--json", prompt],
            "claude" => vec!["-p", prompt, "--output-format", "stream-json"],
            "opencode" => vec!["run", "--format", "json", prompt],
            "cursor" => vec!["-p", prompt, "--output-format", "stream-json"],
            _ => vec![prompt],
        };
        CommandPlan {
            adapter: self.spec.id.to_string(),
            program: self.spec.command.to_string(),
            args: args.into_iter().map(str::to_string).collect(),
        }
    }

    fn resume(&self, session_id: &str, prompt: &str) -> Option<CommandPlan> {
        let args = match self.spec.id {
            "codex" => vec!["exec", "resume", "--json", session_id, prompt],
            "claude" => vec![
                "--resume",
                session_id,
                "-p",
                prompt,
                "--output-format",
                "stream-json",
            ],
            "opencode" => vec!["run", "--format", "json", "--session", session_id, prompt],
            "cursor" => vec![
                "--print",
                "--output-format",
                "stream-json",
                "--resume",
                session_id,
                prompt,
            ],
            _ => return None,
        };
        Some(CommandPlan {
            adapter: self.spec.id.to_string(),
            program: self.spec.command.to_string(),
            args: args.into_iter().map(str::to_string).collect(),
        })
    }
}

impl BuiltinAdapter {
    pub fn id(&self) -> &'static str {
        self.spec.id
    }
}

pub fn builtin_adapters() -> Vec<BuiltinAdapter> {
    SPECS
        .iter()
        .copied()
        .map(|spec| BuiltinAdapter { spec })
        .collect()
}

pub fn builtin_adapter(id: &str) -> Option<BuiltinAdapter> {
    builtin_adapters()
        .into_iter()
        .find(|adapter| adapter.spec.id == id)
}

pub fn detect_all() -> Vec<AdapterInfo> {
    builtin_adapters()
        .iter()
        .map(AgentAdapter::detect)
        .collect()
}

fn detect(spec: &AdapterSpec) -> AdapterInfo {
    let binary = find_binary(spec.command);
    let version = binary.as_deref().and_then(command_version);
    AdapterInfo {
        id: spec.id.to_string(),
        command: spec.command.to_string(),
        installed: binary.is_some(),
        binary: binary.map(|path| path.display().to_string()),
        version,
        auth_status: auth_status(spec.auth_file),
        control_surface: spec.control_surface.to_string(),
        priority: spec.priority.to_string(),
        modes: vec![
            "detect".to_string(),
            "start".to_string(),
            "resume".to_string(),
            "cancel".to_string(),
            "stream".to_string(),
            "collect_result".to_string(),
        ],
    }
}

fn command_version(binary: &Path) -> Option<String> {
    let output = Command::new(binary).arg("--version").output().ok()?;
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr)
    } else {
        String::from_utf8_lossy(&output.stdout)
    };
    let version = text.trim();
    (!version.is_empty()).then(|| version.to_string())
}

fn auth_status(relative_auth_file: Option<&str>) -> String {
    let Some(relative) = relative_auth_file else {
        return "unknown".to_string();
    };
    let Some(home) = env::var_os("HOME") else {
        return "unknown".to_string();
    };
    if PathBuf::from(home).join(relative).exists() {
        "configured".to_string()
    } else {
        "not-detected".to_string()
    }
}

fn find_binary(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(command))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

#[cfg(test)]
mod tests {
    #[test]
    fn detects_known_adapter_specs() {
        let adapters = super::detect_all();
        assert_eq!(adapters.len(), 4);
        assert!(adapters.iter().any(|adapter| adapter.id == "codex"));
        assert!(adapters.iter().any(|adapter| adapter.id == "claude"));
        assert!(adapters.iter().any(|adapter| adapter.id == "opencode"));
        assert!(adapters.iter().any(|adapter| adapter.id == "cursor"));
    }

    #[test]
    fn builds_provider_start_command_plans() {
        let adapters = super::builtin_adapters();
        let codex = adapters
            .iter()
            .find(|adapter| adapter.spec.id == "codex")
            .unwrap();
        let plan = super::AgentAdapter::start(codex, "hello");
        assert_eq!(plan.program, "codex");
        assert_eq!(plan.args, ["exec", "--json", "hello"]);
    }

    #[test]
    fn builds_provider_resume_command_plans() {
        let adapters = super::builtin_adapters();
        let opencode = adapters
            .iter()
            .find(|adapter| adapter.spec.id == "opencode")
            .unwrap();
        let plan =
            super::AgentAdapter::resume(opencode, "session-1", "continue").expect("resume plan");
        assert_eq!(plan.program, "opencode");
        assert_eq!(
            plan.args,
            [
                "run",
                "--format",
                "json",
                "--session",
                "session-1",
                "continue"
            ]
        );
    }
}
