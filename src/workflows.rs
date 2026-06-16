use std::{fs, path::Path};

use serde::Deserialize;

use crate::{home::Home, state::NodeSpec};

pub fn nodes_for(name: &str) -> Vec<NodeSpec> {
    match name {
        "plan-only" => vec![
            NodeSpec::new("plan", "planner"),
            NodeSpec::new("review-plan", "reviewer").depends_on(&["plan"]),
            NodeSpec::new("final-plan", "finalizer").depends_on(&["review-plan"]),
        ],
        "implement-with-review" => vec![
            NodeSpec::new("plan", "planner"),
            NodeSpec::new("implement", "implementer").depends_on(&["plan"]),
            NodeSpec::new("verify", "verifier").depends_on(&["implement"]),
            NodeSpec::new("review", "reviewer").depends_on(&["verify"]),
            NodeSpec::new("merge-decision", "approver").depends_on(&["review"]),
        ],
        "parallel-proposal" => vec![
            NodeSpec::new("plan", "planner"),
            NodeSpec::new("codex-proposal", "implementer")
                .depends_on(&["plan"])
                .agent("codex"),
            NodeSpec::new("claude-proposal", "implementer")
                .depends_on(&["plan"])
                .agent("claude"),
            NodeSpec::new("opencode-proposal", "implementer")
                .depends_on(&["plan"])
                .agent("opencode"),
            NodeSpec::new("judge", "reviewer").depends_on(&[
                "codex-proposal",
                "claude-proposal",
                "opencode-proposal",
            ]),
            NodeSpec::new("verify", "verifier").depends_on(&["judge"]),
        ],
        "fix-ci" => vec![
            NodeSpec::new("triage", "triage"),
            NodeSpec::new("implement-fix", "implementer").depends_on(&["triage"]),
            NodeSpec::new("run-tests", "verifier").depends_on(&["implement-fix"]),
            NodeSpec::new("review-diff", "reviewer").depends_on(&["run-tests"]),
        ],
        "research-then-build" => vec![
            NodeSpec::new("research", "researcher"),
            NodeSpec::new("plan", "planner").depends_on(&["research"]),
            NodeSpec::new("implement", "implementer").depends_on(&["plan"]),
            NodeSpec::new("verify", "verifier").depends_on(&["implement"]),
            NodeSpec::new("docs-update", "documenter").depends_on(&["verify"]),
        ],
        _ => vec![NodeSpec::new("plan", "planner")],
    }
}

pub fn names() -> &'static [&'static str] {
    &[
        "plan-only",
        "implement-with-review",
        "parallel-proposal",
        "fix-ci",
        "research-then-build",
    ]
}

pub fn nodes_for_home(home: &Home, name: &str) -> crate::Result<Vec<NodeSpec>> {
    nodes_for_project(home, Path::new("."), name)
}

pub fn nodes_for_project(
    home: &Home,
    project_root: &Path,
    name: &str,
) -> crate::Result<Vec<NodeSpec>> {
    if let Some(nodes) = load_template(
        project_root
            .join(".zgent/workflows")
            .join(format!("{name}.toml")),
    )? {
        return Ok(nodes);
    }
    if let Some(nodes) = load_template(home.workflows_dir().join(format!("{name}.toml")))? {
        return Ok(nodes);
    }
    Ok(nodes_for(name))
}

pub fn list_names(home: &Home) -> crate::Result<Vec<String>> {
    let mut names: Vec<_> = names().iter().map(|name| (*name).to_string()).collect();
    collect_names(&home.workflows_dir(), &mut names)?;
    collect_names(Path::new(".zgent/workflows"), &mut names)?;
    names.sort();
    names.dedup();
    Ok(names)
}

fn collect_names(dir: &Path, names: &mut Vec<String>) -> crate::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|ext| ext.to_str()) == Some("toml")
            && let Some(name) = entry.path().file_stem().and_then(|stem| stem.to_str())
        {
            names.push(name.to_string());
        }
    }
    Ok(())
}

fn load_template(path: impl AsRef<Path>) -> crate::Result<Option<Vec<NodeSpec>>> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(None);
    }
    let template: WorkflowTemplate = toml::from_str(&fs::read_to_string(path)?)?;
    Ok(Some(template.nodes.into_iter().map(Into::into).collect()))
}

#[derive(Debug, Deserialize)]
struct WorkflowTemplate {
    nodes: Vec<WorkflowNode>,
}

#[derive(Debug, Deserialize)]
struct WorkflowNode {
    name: String,
    role: String,
    #[serde(default)]
    depends_on: Vec<String>,
    agent: Option<String>,
    skill: Option<String>,
}

impl From<WorkflowNode> for NodeSpec {
    fn from(node: WorkflowNode) -> Self {
        NodeSpec {
            name: node.name,
            role: node.role,
            depends_on: node.depends_on,
            agent: node.agent,
            skill: node.skill,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::home::Home;

    #[test]
    fn loads_file_backed_workflow_with_skill_reference() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let home = Home::from_path(temp.path().join(".zgent"));
        fs::create_dir_all(home.workflows_dir())?;
        fs::write(
            home.workflows_dir().join("custom.toml"),
            r#"
[[nodes]]
name = "plan"
role = "planner"
skill = "plan"

[[nodes]]
name = "review"
role = "reviewer"
depends_on = ["plan"]
skill = "code-review"
"#,
        )?;
        let nodes = super::nodes_for_project(&home, temp.path(), "custom")?;
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].skill.as_deref(), Some("plan"));
        assert_eq!(nodes[1].depends_on, ["plan"]);
        Ok(())
    }
}
