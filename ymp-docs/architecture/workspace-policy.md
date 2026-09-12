# Workspace policy and release scope

Owner decision, 2026-09-12: direct work in the selected directory is accepted for the **0.4.0 MVP only**. It is not the intended default protection for large production projects.

## MVP behavior

The selected user directory remains the working directory and the location of deliverables. `~/.ymp2` stores application metadata and evidence. Do not create hidden replacement source trees or Git repositories as application behavior.

Schedule independent read-only work concurrently where its actual access does not conflict. Serialize conflicting writes. If an execution backend cannot reliably limit write access to declared resources, treat that assignment as a writer of the whole working directory. A planner's narrow resource claim is not, by itself, an enforcement guarantee.

Capture and expose the effective workspace/access policy. Show actual recorded output locations and the limits of recovery; direct execution does not promise rollback or preservation of the original directory. Changing a project's current path must not rewrite the historical location of a captured result.

YMP-115 owns resource coordination and a narrow workspace/access-policy boundary. Its first implementation is the direct MVP policy. Keep the boundary suitable for another implementation without adding an unimplemented generic workspace framework.

## After the MVP

YMP-124 owns isolated execution and recoverable publication for larger projects. Candidates are produced in isolated working areas, independently checked, and applied to the canonical user directory through a controlled publication step. The design must detect conflicts, preserve user changes and support recovery from interrupted publication. Ordinary directories and non-software artifacts remain supported; Git worktrees can be one implementation rather than a universal prerequisite.

Publication must record the relationship between the reviewed candidate and the delivered artifact, including actual locations and versions. Workspace isolation and native permission guarantees must be stated precisely. A file copy alone is not an OS sandbox.

This is deferred production work, not a protection already delivered by the MVP. The task defines its storage/lifetime, publication and recovery contracts before implementation.

## Development worktrees

The owner separately authorized isolated development worktrees for parallel work on YMP itself. Those checkouts are development infrastructure, not the application's MVP working-directory behavior.
