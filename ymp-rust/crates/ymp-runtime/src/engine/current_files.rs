use super::*;
use anyhow::ensure;

impl Engine {
    /// Reviewable context only; no native call, permission or source copy is made.
    pub fn current_files_context(
        &self,
        session: &str,
        stage_id: &str,
    ) -> Result<CurrentFilesContext> {
        let captured = self.store.session(session)?;
        let policy = self
            .store
            .session_policy(session)?
            .context("Missing current-files session policy")?;
        let directory = policy.cwd.canonicalize()?;
        let stage = self
            .store
            .recovery_stages(session)?
            .into_iter()
            .find(|s| s.id == stage_id)
            .context("Unknown current-files stage")?;
        let workspace = Workspace::load(&self.store.session_dir(&captured).join("workspace"))?;
        ensure!(
            workspace.directory.canonicalize()? == directory,
            "current_files_context: stored workspace differs from session"
        );
        let paths = workspace.files()?;
        ensure!(
            paths.len() <= 4096,
            "current_files_context: workspace listing exceeds 4096 files"
        );
        let files = paths
            .into_iter()
            .map(|path| {
                let relative = path.strip_prefix(&directory)?.to_path_buf();
                if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
                    Ok(CurrentFileVersion {
                        path: relative,
                        sha256: None,
                        length: None,
                        symlink_target: Some(std::fs::read_link(path)?),
                    })
                } else {
                    let observed = FileSnapshot::capture(&directory, &relative)?;
                    Ok(CurrentFileVersion {
                        path: relative,
                        sha256: observed.sha256,
                        length: observed.bytes.map(|bytes| bytes.len() as u64),
                        symlink_target: None,
                    })
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let board = self.store.board(session)?;
        Ok(CurrentFilesContext {
            session_id: session.into(),
            stage_id: stage.id,
            stage_revision: stage.revision,
            plan: stage.plan,
            result: stage.result,
            task: stage.task,
            team_revision: board
                .team
                .as_ref()
                .context("Current team missing")?
                .revision,
            board_version: content_digest(&serde_json::to_string(&board)?),
            budget: self
                .store
                .session_budget(session)?
                .context("Current budget missing")?,
            failures: self.store.current_files_failures(session)?,
            directory,
            files,
        })
    }

    pub fn current_files_authorizations(
        &self,
        session: &str,
    ) -> Result<Vec<CurrentFilesAuthorization>> {
        self.store.current_files_authorizations(session)
    }

    /// Explicit local owner authorization for new work only. Historical effects
    /// remain unresolved and ordinary admission still governs every actual call.
    pub fn continue_with_current_files(
        &self,
        command: &ContinueWithCurrentFilesCommand,
    ) -> Result<CurrentFilesReceipt> {
        if let Some(receipt) = self.store.current_files_receipt(command)? {
            return Ok(receipt);
        }
        let context = &command.context;
        let session = self.store.session(&context.session_id)?;
        let policy = self
            .store
            .session_policy(&session.id)?
            .context("Missing current-files session policy")?;
        let project = self.store.project(&policy.cwd)?;
        ensure!(
            project.id == session.project_id,
            "current_files_context: foreign project"
        );
        let _owner = crate::WorkspaceOwner::acquire(&self.store, &project)?;
        let _lock = self.store.lock_session(&session)?;
        if let Some(receipt) = self.store.current_files_receipt(command)? {
            return Ok(receipt);
        }
        self.owner_boundary(&session.id)?;
        let current = self.current_files_context(&session.id, &context.stage_id)?;
        ensure!(current == *context, "stale_current_files: reviewed workspace or saved state changed; refresh the owner context");
        self.store.authorize_current_files(command)
    }
}
