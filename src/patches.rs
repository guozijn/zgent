use std::{fs, path::Path, process::Command};

use anyhow::{Context, bail};
use uuid::Uuid;

use crate::state::Store;

pub fn capture_git_diff(
    store: &Store,
    task_id: &str,
    node_id: Option<&str>,
    repo: &Path,
) -> crate::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["diff", "--binary"])
        .output()
        .with_context(|| format!("run git diff in {}", repo.display()))?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }

    let path = store
        .home()
        .task_dir(task_id)
        .join("patches")
        .join(format!("patch-{}.patch", Uuid::new_v4()));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, output.stdout)?;
    let status = if fs::metadata(&path)?.len() == 0 {
        "empty"
    } else {
        "captured"
    };
    store.record_patch(task_id, node_id, &path, status)
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use crate::{home::Home, state::NodeSpec, state::Store};

    #[test]
    fn captures_git_diff_patch() -> anyhow::Result<()> {
        if Command::new("git").arg("--version").output().is_err() {
            return Ok(());
        }
        let temp = tempfile::tempdir()?;
        let repo = temp.path().join("repo");
        fs::create_dir(&repo)?;
        Command::new("git").arg("init").arg(&repo).output()?;
        fs::write(repo.join("file.txt"), "before\n")?;
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "file.txt"])
            .output()?;
        fs::write(repo.join("file.txt"), "after\n")?;

        let home = Home::from_path(temp.path().join(".zgent"));
        let store = Store::open(home.clone())?;
        let task_id = store.create_task(
            "capture patch",
            "plan-only",
            vec![NodeSpec::new("plan", "planner")],
        )?;
        let patch_id = super::capture_git_diff(&store, &task_id, None, &repo)?;
        assert!(patch_id.starts_with("patch-"));
        let patch_dir = home.task_dir(&task_id).join("patches");
        let patch = fs::read_to_string(fs::read_dir(patch_dir)?.next().unwrap()?.path())?;
        assert!(patch.contains("after"));
        Ok(())
    }
}
