use std::{fs, path::Path, process::Command};

use anyhow::{Context, bail};

use crate::home::Home;

pub fn create(
    home: &Home,
    repo: &Path,
    task_id: &str,
    agent_id: &str,
) -> crate::Result<std::path::PathBuf> {
    let repo_name = repo
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    let dest = home
        .worktrees_dir()
        .join(repo_name)
        .join(task_id)
        .join(agent_id);
    if dest.exists() {
        return Ok(dest);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "add", "--detach"])
        .arg(&dest)
        .output()
        .with_context(|| format!("create worktree from {}", repo.display()))?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use crate::home::Home;

    #[test]
    fn creates_git_worktree_for_task_agent() -> anyhow::Result<()> {
        if Command::new("git").arg("--version").output().is_err() {
            return Ok(());
        }
        let temp = tempfile::tempdir()?;
        let repo = temp.path().join("repo");
        fs::create_dir(&repo)?;
        Command::new("git").arg("init").arg(&repo).output()?;
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["config", "user.email", "test@example.com"])
            .output()?;
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["config", "user.name", "Test"])
            .output()?;
        fs::write(repo.join("file.txt"), "ok\n")?;
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "file.txt"])
            .output()?;
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["commit", "-m", "init"])
            .output()?;

        let home = Home::from_path(temp.path().join(".zgent"));
        let path = super::create(&home, &repo, "task-1", "codex")?;
        assert!(path.join("file.txt").exists());
        Ok(())
    }
}
