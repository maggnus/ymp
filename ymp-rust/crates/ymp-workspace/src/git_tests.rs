//! Embedded fixtures exercise real repository objects without invoking Git or a shell.
use super::*;
use std::fs;

struct Fixture {
    dir: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut options = git2::RepositoryInitOptions::new();
        options.initial_head("main");
        let repo = Repository::init_opts(dir.path(), &options).unwrap();
        repo.config()
            .unwrap()
            .set_str("user.name", "Fixture")
            .unwrap();
        repo.config()
            .unwrap()
            .set_str("user.email", "fixture@example.invalid")
            .unwrap();
        Self { dir }
    }
    fn root(&self) -> &Path {
        self.dir.path()
    }
    fn repo(&self) -> Repository {
        Repository::open(self.root()).unwrap()
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.root().join(name), text).unwrap();
    }
    fn stage(&self, name: &str) {
        let repo = self.repo();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(name)).unwrap();
        index.write().unwrap();
    }
    fn commit(&self) -> String {
        let repo = self.repo();
        let mut index = repo.index().unwrap();
        let id = index.write_tree().unwrap();
        let tree = repo.find_tree(id).unwrap();
        let signature = git2::Signature::now("Fixture", "fixture@example.invalid").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<_> = parent.iter().collect();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "fixture",
            &tree,
            &parents,
        )
        .unwrap()
        .to_string()
    }
    fn initial(&self) -> String {
        self.write("code.rs", "fn before() {}\n");
        self.stage("code.rs");
        self.commit()
    }
}

#[tokio::test]
async fn empty_and_unborn_repositories_are_distinct_from_non_git() {
    let plain = tempfile::tempdir().unwrap();
    assert_eq!(
        inspect(plain.path(), None, None).await.unwrap_err(),
        GitError::NotRepository
    );
    let f = Fixture::new();
    let snapshot = inspect(f.root(), None, None).await.unwrap();
    assert_eq!(snapshot.branch.as_deref(), Some("main"));
    assert!(snapshot.head.is_none());
    assert!(snapshot.files.is_empty());
    f.write("new file.rs", "fn new_file() {}\n");
    let snapshot = inspect(f.root(), None, None).await.unwrap();
    assert!(snapshot.dirty);
    assert_eq!(snapshot.comparison, Comparison::Uncommitted);
    assert_eq!(snapshot.files.len(), 1);
    let patch = diff(&snapshot, &snapshot.files[0]).await.unwrap();
    assert!(patch.text.contains("+fn new_file() {}"), "{patch:?}");
}

#[tokio::test]
async fn committed_comparison_uses_the_base_and_not_working_edits() {
    let f = Fixture::new();
    let initial = f.initial();
    let repo = f.repo();
    let commit = repo.find_commit(Oid::from_str(&initial).unwrap()).unwrap();
    repo.branch("feature", &commit, false).unwrap();
    switch_branch(f.root(), "feature", Some(&initial))
        .await
        .unwrap();
    f.write("code.rs", "fn committed() {}\n");
    f.stage("code.rs");
    f.commit();
    f.write("code.rs", "fn not_committed() {}\n");
    let committed = inspect(f.root(), Some(Comparison::Committed), None)
        .await
        .unwrap();
    assert_eq!(committed.base.as_deref(), Some("main"));
    assert_eq!(committed.base_oid.as_deref(), Some(initial.as_str()));
    assert_eq!(committed.files.len(), 1);
    let patch = diff(&committed, &committed.files[0]).await.unwrap();
    assert!(patch.text.contains("+fn committed() {}"));
    assert!(!patch.text.contains("not_committed"));
    let working = inspect(f.root(), None, None).await.unwrap();
    assert_eq!(working.comparison, Comparison::Uncommitted);
    assert!(diff(&working, &working.files[0])
        .await
        .unwrap()
        .text
        .contains("+fn not_committed() {}"));
    assert!(
        inspect(f.root(), Some(Comparison::Committed), Some("not-a-branch"))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn status_preserves_index_and_working_changes_even_when_the_net_diff_cancels() {
    let f = Fixture::new();
    f.initial();
    f.write("code.rs", "fn staged() {}\n");
    f.stage("code.rs");
    f.write("code.rs", "fn before() {}\n");
    let before = fs::read(f.repo().path().join("index")).unwrap();
    let snapshot = inspect(f.root(), None, None).await.unwrap();
    assert!(snapshot.dirty);
    assert_eq!(snapshot.files.len(), 1);
    assert!(snapshot.files[0].staged && snapshot.files[0].unstaged);
    assert_eq!(
        before,
        fs::read(f.repo().path().join("index")).unwrap(),
        "inspection rewrote the index"
    );
    assert!(diff(&snapshot, &snapshot.files[0])
        .await
        .unwrap()
        .notice
        .is_some());
}

#[tokio::test]
async fn native_paths_and_renames_keep_their_identity() {
    let f = Fixture::new();
    f.initial();
    for name in ["same-a\nb.rs", "same-a b.rs", "[literal].rs"] {
        f.write(name, &format!("// {name:?}\n"));
    }
    let snapshot = inspect(f.root(), None, None).await.unwrap();
    for name in ["same-a\nb.rs", "same-a b.rs", "[literal].rs"] {
        let change = snapshot
            .files
            .iter()
            .find(|c| c.path == Path::new(name))
            .unwrap();
        let patch = diff(&snapshot, change).await.unwrap();
        assert!(patch.text.contains(&format!("+// {name:?}")), "{patch:?}");
    }
    // A committed rename is identified through native paths on both sides.
    fs::rename(f.root().join("code.rs"), f.root().join("renamed.rs")).unwrap();
    let repo = f.repo();
    let mut index = repo.index().unwrap();
    index.remove_path(Path::new("code.rs")).unwrap();
    index.add_path(Path::new("renamed.rs")).unwrap();
    index.write().unwrap();
    let snapshot = inspect(f.root(), None, None).await.unwrap();
    let renamed = snapshot
        .files
        .iter()
        .find(|c| c.path == Path::new("renamed.rs"))
        .unwrap();
    assert_eq!(renamed.old_path.as_deref(), Some(Path::new("code.rs")));
    assert_eq!(renamed.status, "R");
}

#[tokio::test]
async fn branch_switching_refuses_dirty_stale_and_occupied_branches() {
    let f = Fixture::new();
    let original = f.initial();
    let repo = f.repo();
    let commit = repo.find_commit(Oid::from_str(&original).unwrap()).unwrap();
    repo.branch("feature", &commit, false).unwrap();
    repo.branch("occupied", &commit, false).unwrap();
    let other = tempfile::tempdir().unwrap();
    let linked = other.path().join("linked");
    let reference = repo.find_reference("refs/heads/occupied").unwrap();
    repo.worktree(
        "linked",
        &linked,
        Some(git2::WorktreeAddOptions::new().reference(Some(&reference))),
    )
    .unwrap();
    let selected = choices(&linked).await.unwrap();
    assert_eq!(selected.worktrees.len(), 2);
    assert!(selected
        .branches
        .iter()
        .find(|b| b.name == "main")
        .unwrap()
        .worktree
        .is_some());
    assert!(switch_branch(f.root(), "occupied", Some(&original))
        .await
        .is_err());
    f.write("untracked.txt", "keep");
    assert_eq!(
        switch_branch(f.root(), "feature", Some(&original))
            .await
            .unwrap_err(),
        GitError::Dirty
    );
    assert_eq!(
        fs::read_to_string(f.root().join("untracked.txt")).unwrap(),
        "keep"
    );
    fs::remove_file(f.root().join("untracked.txt")).unwrap();
    assert_eq!(
        switch_branch(
            f.root(),
            "feature",
            Some("0000000000000000000000000000000000000000")
        )
        .await
        .unwrap_err(),
        GitError::Stale
    );
    switch_branch(f.root(), "feature", Some(&original))
        .await
        .unwrap();
    assert_eq!(identity(&f.repo()).unwrap().0.as_deref(), Some("feature"));
}

#[tokio::test]
async fn stale_diff_and_binary_or_oversized_content_are_not_misrepresented() {
    let f = Fixture::new();
    f.initial();
    f.write("code.rs", "fn pending() {}\n");
    let snapshot = inspect(f.root(), None, None).await.unwrap();
    f.stage("code.rs");
    f.commit();
    assert_eq!(
        diff(&snapshot, &snapshot.files[0]).await.unwrap_err(),
        GitError::Stale
    );
    fs::write(f.root().join("binary.bin"), [0, 1, 2, 3]).unwrap();
    f.write("large.txt", &"x".repeat(MAX_DIFF_BYTES + 1));
    let snapshot = inspect(f.root(), None, None).await.unwrap();
    for name in ["binary.bin", "large.txt"] {
        let entry = snapshot
            .files
            .iter()
            .find(|c| c.path == Path::new(name))
            .unwrap();
        let value = diff(&snapshot, entry).await.unwrap();
        assert!(
            value.text.is_empty() && value.notice.is_some(),
            "{name}: {value:?}"
        );
    }
}

#[tokio::test]
async fn checkout_does_not_execute_hooks_or_overwrite_ignored_collisions() {
    let f = Fixture::new();
    let initial = f.initial();
    let repo = f.repo();
    let commit = repo.find_commit(Oid::from_str(&initial).unwrap()).unwrap();
    repo.branch("target", &commit, false).unwrap();
    switch_branch(f.root(), "target", Some(&initial))
        .await
        .unwrap();
    f.write("collision", "tracked");
    f.stage("collision");
    let target = f.commit();
    switch_branch(f.root(), "main", Some(&target))
        .await
        .unwrap();
    fs::write(repo.path().join("info/exclude"), "collision\n").unwrap();
    f.write("collision", "local ignored data");
    assert!(switch_branch(f.root(), "target", Some(&initial))
        .await
        .is_err());
    assert_eq!(
        fs::read_to_string(f.root().join("collision")).unwrap(),
        "local ignored data"
    );
    fs::remove_file(f.root().join("collision")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let hook = repo.path().join("hooks/post-checkout");
        fs::write(&hook, "#!/bin/sh\nprintf unexpected > hook-was-run\n").unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
    switch_branch(f.root(), "target", Some(&initial))
        .await
        .unwrap();
    assert!(!f.root().join("hook-was-run").exists());
}

#[test]
fn non_utf8_paths_are_not_lossily_replaced() {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        assert_eq!(
            path(b"file-\xff.rs").unwrap().as_os_str().as_bytes(),
            b"file-\xff.rs"
        );
    }
    assert!(path(b"../outside").is_err());
    assert!(path(b"/absolute").is_err());
}

#[tokio::test]
async fn cancellation_keeps_worker_ownership_until_blocking_work_finishes() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct Held(Arc<AtomicBool>);
    impl Drop for Held {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let released = Arc::new(AtomicBool::new(false));
    let held = Held(released.clone());
    let (started, ready) = tokio::sync::oneshot::channel();
    let (finish, wait) = std::sync::mpsc::channel();
    let task = tokio::spawn(worker(move || {
        let _held = held;
        started.send(()).unwrap();
        wait.recv().unwrap();
        Ok(())
    }));
    ready.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(
        !released.load(Ordering::SeqCst),
        "a dropped UI waiter released ongoing work"
    );
    finish.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !released.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn a_locked_head_refuses_checkout_before_changing_files_or_index() {
    let f = Fixture::new();
    let initial = f.initial();
    let repo = f.repo();
    let commit = repo.find_commit(Oid::from_str(&initial).unwrap()).unwrap();
    repo.branch("target", &commit, false).unwrap();
    switch_branch(f.root(), "target", Some(&initial))
        .await
        .unwrap();
    f.write("code.rs", "fn target() {}\n");
    f.stage("code.rs");
    let target = f.commit();
    switch_branch(f.root(), "main", Some(&target))
        .await
        .unwrap();
    let before = fs::read(f.root().join("code.rs")).unwrap();
    let index = fs::read(repo.path().join("index")).unwrap();
    fs::write(
        repo.path().join("HEAD.lock"),
        "held by another Git operation\n",
    )
    .unwrap();
    assert!(switch_branch(f.root(), "target", Some(&initial))
        .await
        .is_err());
    assert_eq!(fs::read(f.root().join("code.rs")).unwrap(), before);
    assert_eq!(fs::read(repo.path().join("index")).unwrap(), index);
}
