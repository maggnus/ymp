//! Build and steer isolated terminal-test repositories through the embedded Git library.
use git2::{IndexAddOption, Oid, Repository, Signature};
use std::path::Path;

fn commit(repo: &Repository) -> anyhow::Result<Oid> {
    let mut index = repo.index()?;
    index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;
    let signature = Signature::now("Fixture", "fixture@example.invalid")?;
    let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
    let parents: Vec<_> = parent.iter().collect();
    Ok(repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "fixture",
        &tree,
        &parents,
    )?)
}

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    let command = args.get(1).and_then(|a| a.to_str()).unwrap_or_default();
    let directory = Path::new(
        args.get(2)
            .ok_or_else(|| anyhow::anyhow!("fixture directory required"))?,
    );
    let marker = directory.join(".ymp-git-fixture");
    if command == "init" {
        anyhow::ensure!(
            !directory.exists() || std::fs::read_dir(directory)?.next().is_none(),
            "fixture must start empty"
        );
        std::fs::create_dir_all(directory)?;
        let mut options = git2::RepositoryInitOptions::new();
        options.initial_head("main");
        let repo = Repository::init_opts(directory, &options)?;
        repo.config()?.set_str("core.autocrlf", "false")?;
        std::fs::write(&marker, "ymp143 isolated fixture\n")?;
        std::fs::write(directory.join(".gitignore"), ".ymp-git-fixture\n")?;
        std::fs::write(directory.join("code.rs"), "fn BASE_143() {}\n")?;
        let base = commit(&repo)?;
        let base_commit = repo.find_commit(base)?;
        repo.branch("feature", &base_commit, false)?;
        repo.branch("other", &base_commit, false)?;
        repo.set_head("refs/heads/feature")?;
        std::fs::write(directory.join("code.rs"), "fn COMMITTED_143() {}\n")?;
        commit(&repo)?;
        let linked = directory.parent().unwrap().join("linked-worktree");
        let reference = repo.find_reference("refs/heads/other")?;
        repo.worktree(
            "linked",
            &linked,
            Some(git2::WorktreeAddOptions::new().reference(Some(&reference))),
        )?;
        std::fs::write(linked.join("linked.rs"), "fn LINKED_143() {}\n")?;
        std::fs::write(directory.join("code.rs"), "fn WORKING_143() {}\n")?;
        std::fs::write(directory.join("new.rs"), "fn UNTRACKED_143() {}\n")?;
    } else {
        anyhow::ensure!(
            std::fs::read_to_string(&marker)? == "ymp143 isolated fixture\n",
            "not an owned fixture"
        );
        let repo = Repository::open(directory)?;
        match command {
            "live" => {
                std::fs::write(directory.join("a-live.rs"), "fn LIVE_143() {}\n")?;
            }
            "clean" => {
                let object = repo.head()?.peel(git2::ObjectType::Commit)?;
                repo.reset(&object, git2::ResetType::Hard, None)?;
                for name in ["a-live.rs", "new.rs"] {
                    let path = directory.join(name);
                    if path.exists() {
                        std::fs::remove_file(path)?;
                    }
                }
            }
            "commit" => {
                commit(&repo)?;
            }
            "head" => {}
            _ => anyhow::bail!("unknown fixture operation"),
        }
    }
    let repo = Repository::open(directory)?;
    println!(
        "{}",
        serde_json::json!({"branch": repo.head()?.shorthand()?, "head": repo.head()?.target().map(|id|id.to_string())})
    );
    Ok(())
}
