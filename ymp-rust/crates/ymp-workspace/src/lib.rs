use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub root: PathBuf,
    pub integration: PathBuf,
    pub source: PathBuf,
    pub baseline: String,
}

pub async fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "ymp")
        .env("GIT_AUTHOR_EMAIL", "ymp@localhost")
        .env("GIT_COMMITTER_NAME", "ymp")
        .env("GIT_COMMITTER_EMAIL", "ymp@localhost")
        .output()
        .await?;
    if !out.status.success() {
        bail!(
            "git {}: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

impl Workspace {
    pub async fn create(source: &Path, root: &Path) -> Result<Self> {
        let source = source.canonicalize()?;
        std::fs::create_dir_all(root)?;
        let root = root.canonicalize()?;
        if root.starts_with(&source) {
            bail!("Workspace storage must be outside the source directory");
        }
        let integration = root.join("integration");
        if integration.exists() {
            bail!("Integration workspace already exists");
        }
        std::fs::create_dir_all(&integration)?;
        // A snapshot repository keeps original refs, index and uncommitted files untouched.
        // Prefer Git's tracked/untracked list so ignored build outputs do not become artifacts.
        let listed = Command::new("git")
            .args([
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ])
            .current_dir(&source)
            .output()
            .await?;
        if listed.status.success() {
            for bytes in listed.stdout.split(|&b| b == 0).filter(|b| !b.is_empty()) {
                #[cfg(unix)]
                let rel = {
                    use std::os::unix::ffi::OsStrExt;
                    Path::new(std::ffi::OsStr::from_bytes(bytes))
                };
                if source.join(rel).symlink_metadata().is_ok() {
                    copy_entry(&source.join(rel), &integration.join(rel))?;
                }
            }
        } else {
            for entry in walkdir::WalkDir::new(&source)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    e.depth() == 0
                        || ![".git", "node_modules", "target", ".DS_Store"]
                            .contains(&e.file_name().to_string_lossy().as_ref())
                })
            {
                let e = entry?;
                if e.depth() == 0 {
                    continue;
                }
                let rel = e.path().strip_prefix(&source)?;
                copy_entry(e.path(), &integration.join(rel))?;
            }
        }
        git(&integration, &["init", "-b", "ymp-result"]).await?;
        git(&integration, &["config", "core.hooksPath", "/dev/null"]).await?;
        git(&integration, &["add", "-A"]).await?;
        git(
            &integration,
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "ymp: source snapshot",
            ],
        )
        .await?;
        let baseline = git(&integration, &["rev-parse", "HEAD"]).await?;
        // Keep generated caches out of result patches. Add exclusions after the
        // initial snapshot so deliberately tracked source files remain tracked.
        std::fs::write(
            integration.join(".git/info/exclude"),
            "__pycache__/\n*.py[cod]\nnode_modules/\ntarget/\n.DS_Store\n",
        )?;
        let w = Self {
            root,
            integration,
            source,
            baseline,
        };
        std::fs::write(
            w.root.join("workspace.json"),
            serde_json::to_vec_pretty(&w)?,
        )?;
        Ok(w)
    }
    pub fn load(root: &Path) -> Result<Self> {
        Ok(serde_json::from_slice(&std::fs::read(
            root.join("workspace.json"),
        )?)?)
    }
    pub async fn fork(&self, label: &str) -> Result<(PathBuf, String)> {
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            bail!("Invalid workspace label");
        }
        let base = git(&self.integration, &["rev-parse", "HEAD"]).await?;
        let path = self.root.join(label);
        git(
            &self.integration,
            &[
                "worktree",
                "add",
                "--detach",
                path.to_str().context("Non-UTF8 path")?,
                &base,
            ],
        )
        .await?;
        Ok((path, base))
    }
    pub async fn checkpoint(path: &Path, message: &str) -> Result<String> {
        git(path, &["add", "-A"]).await?;
        git(
            path,
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                message,
            ],
        )
        .await?;
        git(path, &["rev-parse", "HEAD"]).await
    }
    pub async fn integrate(&self, path: &Path, base: &str) -> Result<()> {
        let label = path
            .file_name()
            .context("Missing candidate label")?
            .to_string_lossy();
        let marker = format!("ymp: integrate {label}");
        let history = git(
            &self.integration,
            &["log", "--format=%s", &format!("{}..HEAD", self.baseline)],
        )
        .await?;
        if history.lines().any(|line| line == marker) {
            return Ok(());
        }
        let candidate = Self::checkpoint(path, "ymp: accepted task").await?;
        let commits = git(
            path,
            &["rev-list", "--reverse", &format!("{base}..{candidate}")],
        )
        .await?;
        if commits.trim().is_empty() {
            return Ok(());
        }
        // Squash the complete candidate tree against its original base. Agents may
        // make several commits; selecting only their final commit loses changes.
        let tree = git(path, &["rev-parse", "HEAD^{tree}"]).await?;
        let squash = git(path, &["commit-tree", &tree, "-p", base, "-m", &marker]).await?;
        if let Err(error) = git(
            &self.integration,
            &[
                "-c",
                "commit.gpgsign=false",
                "cherry-pick",
                "--allow-empty",
                &squash,
            ],
        )
        .await
        {
            let _ = git(&self.integration, &["cherry-pick", "--abort"]).await;
            return Err(error.context("Integration conflict; candidate workspace retained"));
        }
        Ok(())
    }
    pub async fn diff(&self) -> Result<String> {
        git(
            &self.integration,
            &["diff", "--binary", &self.baseline, "HEAD"],
        )
        .await
    }
}

fn copy_entry(source: &Path, target: &Path) -> Result<()> {
    let meta = source.symlink_metadata()?;
    if meta.is_dir() {
        std::fs::create_dir_all(target)?;
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if meta.file_type().is_symlink() {
        #[cfg(unix)]
        std::os::unix::fs::symlink(std::fs::read_link(source)?, target)?;
    } else if meta.is_file() {
        std::fs::copy(source, target)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn preserves_dirty_source_and_merges_all_candidate_commits() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        std::fs::create_dir(&source).unwrap();
        git(&source, &["init"]).await.unwrap();
        std::fs::write(source.join("a"), "initial").unwrap();
        Workspace::checkpoint(&source, "initial").await.unwrap();
        std::fs::write(source.join("a"), "dirty").unwrap();
        std::fs::write(source.join("new"), "new").unwrap();
        let before = git(&source, &["status", "--porcelain"]).await.unwrap();
        let w = Workspace::create(&source, &temp.path().join("work"))
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(w.integration.join("a")).unwrap(),
            "dirty"
        );
        let (p, base) = w.fork("task").await.unwrap();
        std::fs::write(p.join("one"), "1").unwrap();
        Workspace::checkpoint(&p, "one").await.unwrap();
        std::fs::write(p.join("two"), "2").unwrap();
        w.integrate(&p, &base).await.unwrap();
        let integrated_head = git(&w.integration, &["rev-parse", "HEAD"]).await.unwrap();
        w.integrate(&p, &base).await.unwrap();
        assert_eq!(
            integrated_head,
            git(&w.integration, &["rev-parse", "HEAD"]).await.unwrap()
        );
        assert!(w.integration.join("one").exists());
        assert!(w.integration.join("two").exists());
        assert_eq!(
            before,
            git(&source, &["status", "--porcelain"]).await.unwrap()
        );
        assert!(!source.join("one").exists());
    }
}
