use std::fs;

use serde_json::json;

use crate::{adapters, config::Config, home::Home, skills, state::Store};

pub fn run(home: Home) -> crate::Result<()> {
    create_layout(&home)?;
    let store = Store::open(home.clone())?;
    write_config(&home)?;
    write_policy(&home)?;
    write_default_agent(&home, &store)?;

    let detected = adapters::detect_all();
    fs::write(
        home.adapters_file(),
        serde_json::to_string_pretty(&detected)?,
    )?;
    for adapter in &detected {
        store.upsert_adapter(adapter)?;
        if adapter.installed {
            write_provider_profile(&home, &store, adapter)?;
        }
    }

    let installed_skills = skills::install_defaults(&home)?;
    store.sync_skill_index(&installed_skills)?;
    store.record_event(None, None, "zgent.init", json!({ "home": home.root() }))?;
    print_report(&home, &detected, installed_skills.len());
    Ok(())
}

fn create_layout(home: &Home) -> crate::Result<()> {
    for dir in [
        home.root().to_path_buf(),
        home.state_dir(),
        home.agents_dir(),
        home.adapters_dir(),
        home.skills_dir(),
        home.workflows_dir(),
        home.plugins_dir().join("installed"),
        home.plugins_dir().join("cache"),
        home.collaboration_dir(),
        home.tasks_dir(),
        home.worktrees_dir(),
        home.logs_dir(),
        home.policy_dir(),
    ] {
        fs::create_dir_all(dir)?;
    }
    Ok(())
}

fn write_config(home: &Home) -> crate::Result<()> {
    let config = Config::new(home);
    fs::write(home.config(), toml::to_string_pretty(&config)?)?;
    Ok(())
}

fn write_policy(home: &Home) -> crate::Result<()> {
    write_if_missing(
        home.policy_dir().join("default.toml"),
        "default_mode = \"review-first\"\nwrite_requires_lock = true\ndangerous_commands_require_approval = true\n",
    )?;
    write_if_missing(
        home.policy_dir().join("trusted-tools.toml"),
        "tools = [\"cargo test\", \"cargo fmt\", \"git diff\"]\n",
    )?;
    Ok(())
}

fn write_default_agent(home: &Home, store: &Store) -> crate::Result<()> {
    let text = r#"[agent]
id = "zgent-core"
kind = "coordinator"
description = "Default local coordinator for routing tasks to installed coding agents."

[routing]
strategy = "capability_then_cost_then_recency"
allow_parallel = true
require_resource_locks = true

[safety]
default_mode = "review-first"
write_requires_lock = true
dangerous_commands_require_approval = true
"#;
    fs::write(home.agents_dir().join("default.toml"), text)?;
    store.upsert_agent_profile(
        "zgent-core",
        "coordinator",
        None,
        json!({
            "routing": {
                "strategy": "capability_then_cost_then_recency",
                "allow_parallel": true,
                "require_resource_locks": true
            },
            "safety": {
                "default_mode": "review-first",
                "write_requires_lock": true,
                "dangerous_commands_require_approval": true
            }
        }),
    )?;
    Ok(())
}

fn write_provider_profile(
    home: &Home,
    store: &Store,
    adapter: &adapters::AdapterInfo,
) -> crate::Result<()> {
    let text = format!(
        "[agent]\nid = \"{}\"\nkind = \"provider\"\nadapter = \"{}\"\ncommand = \"{}\"\ncontrol_surface = \"{}\"\n",
        adapter.id, adapter.id, adapter.command, adapter.control_surface
    );
    fs::write(home.agents_dir().join(format!("{}.toml", adapter.id)), text)?;
    store.upsert_agent_profile(
        &adapter.id,
        "provider",
        Some(&adapter.id),
        json!({
            "command": adapter.command,
            "binary": adapter.binary,
            "version": adapter.version,
            "control_surface": adapter.control_surface,
            "auth_status": adapter.auth_status
        }),
    )?;
    Ok(())
}

fn write_if_missing(path: std::path::PathBuf, text: &str) -> crate::Result<()> {
    if !path.exists() {
        fs::write(path, text)?;
    }
    Ok(())
}

fn print_report(home: &Home, adapters: &[adapters::AdapterInfo], skill_count: usize) {
    println!("zgent initialized at {}", home.root().display());
    println!("state: {}", home.db().display());
    println!("default agent: zgent-core");
    println!("skills installed: {skill_count}");
    println!("agents:");
    for adapter in adapters {
        let status = if adapter.installed {
            "installed"
        } else {
            "missing"
        };
        let version = adapter.version.as_deref().unwrap_or("-");
        println!("  {}: {} ({})", adapter.id, status, version);
    }
}

#[cfg(test)]
mod tests {
    use crate::home::Home;

    #[test]
    fn init_creates_home_layout() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::from_path(temp.path().join(".zgent"));
        super::run(home.clone()).unwrap();
        assert!(home.config().exists());
        assert!(home.db().exists());
        assert!(home.adapters_file().exists());
        assert!(home.agents_dir().join("default.toml").exists());
        assert!(home.skills_dir().join("plan").join("SKILL.md").exists());
        assert!(home.workflows_dir().exists());
    }
}
