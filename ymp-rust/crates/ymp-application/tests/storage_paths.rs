//! Acceptance: trusted foreground code can request only one concrete attempt workspace or
//! evidence directory, never use the typed operation as a relative route back into storage.

use tempfile::tempdir;
use ymp_application::{Application, ApplicationError};
use ymp_domain::Budget;

#[test]
fn attempt_paths_are_concrete_children_and_refuse_storage_traversal() {
    let temporary = tempdir().expect("temporary directory");
    let application = Application::create(temporary.path(), "run-1", Budget::new(1, 1))
        .expect("create application");

    assert_eq!(
        application
            .private_workspace_path("attempt-1")
            .expect("private workspace"),
        temporary.path().join("workspaces/attempt-1")
    );
    assert_eq!(
        application
            .runtime_evidence_path("attempt-1")
            .expect("runtime evidence"),
        temporary.path().join("runtime-evidence/attempt-1")
    );

    for invalid in ["", ".", "../objects", "nested/attempt"] {
        assert!(matches!(
            application.private_workspace_path(invalid),
            Err(ApplicationError::InvalidIdentifier { kind: "attempt_id" })
        ));
        assert!(matches!(
            application.runtime_evidence_path(invalid),
            Err(ApplicationError::InvalidIdentifier { kind: "attempt_id" })
        ));
    }
}
