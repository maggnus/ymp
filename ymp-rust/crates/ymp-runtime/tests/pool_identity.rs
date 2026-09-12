use std::collections::HashSet;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::{Config, ProviderKind, SessionAgentView};
use ymp_runtime::Engine;
use ymp_storage::Store;

#[tokio::test]
async fn agents_sharing_a_backend_keep_contexts_authorship_usage_and_captured_names() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let store = Store::open(&temp.path().join("state")).unwrap();
    let mut config = Config::default();
    config.providers.truncate(1);
    config.providers[0].kind = ProviderKind::Mock;
    config.providers[0].command = "internal".into();
    config.agents.truncate(2);
    for (index, profile) in config.agents.iter_mut().enumerate() {
        profile.id = format!("individual-{index}");
        profile.name = format!("Captured name {index}");
        profile.provider = "codex".into();
        profile.model = Some("offline-fixture-model".into());
        profile.instructions = "[mock:usage]".into();
    }
    config.team = config.agents.iter().map(|a| a.id.clone()).collect();
    config.validate().unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut engine = Engine::new(store.clone(), config, tx, CancellationToken::new()).unwrap();
    engine.use_memory = false;
    let outcome = engine
        .run(&project, "Create a greeting", None)
        .await
        .unwrap();
    assert_eq!(outcome.session.status, "completed");
    let session_id = &outcome.session.id;
    let context_keys = outcome
        .session
        .team
        .iter()
        .map(|agent| {
            format!(
                "native:{session_id}:{}:{}:read",
                agent.id,
                project.display()
            )
        })
        .collect::<Vec<_>>();
    let contexts = context_keys
        .iter()
        .map(|key| store.value(key).unwrap().unwrap())
        .collect::<Vec<_>>();
    assert_ne!(
        contexts[0], contexts[1],
        "native conversation was shared across agent IDs"
    );

    let usage = store.session_usage(session_id).unwrap();
    assert_eq!(usage.agents.len(), 2);
    assert!(
        !usage.agents.contains_key("codex"),
        "usage was grouped by provider"
    );
    assert!(usage
        .agents
        .values()
        .all(|u| u.calls > 0 && u.known_total().unwrap() > 0));
    let messages = store.messages(session_id, 0, 10000).unwrap();
    let authors = messages
        .iter()
        .map(|m| m.author.as_str())
        .collect::<HashSet<_>>();
    assert!(authors.contains("individual-0") && authors.contains("individual-1"));
    let tasks = store.tasks(session_id).unwrap();
    assert_ne!(tasks[0].assignee, tasks[0].reviewer);
    assert!(tasks.iter().all(|task| task
        .assignee
        .as_ref()
        .is_some_and(|id| usage.agents.contains_key(id))));

    for profile in &mut engine.config.agents {
        profile.name = format!("Edited {}", profile.id);
    }
    let follow_up = engine
        .follow_up(&project, "Where is the output?", session_id)
        .await
        .unwrap();
    assert_eq!(follow_up.session.id, *session_id);
    for (key, original) in context_keys.iter().zip(contexts) {
        assert_eq!(
            store.value(key).unwrap().unwrap(),
            original,
            "continuation replaced an agent's own context"
        );
    }
    let restored_store = Store::open(&temp.path().join("state")).unwrap();
    let captured = restored_store.session(session_id).unwrap();
    let view = SessionAgentView::from_captured(&captured, None, None).unwrap();
    assert_eq!(view.captured_name("individual-0"), Some("Captured name 0"));
    assert_eq!(view.captured_name("individual-1"), Some("Captured name 1"));
    assert!(
        view.active_invocations.is_none(),
        "saved native contexts became live activity"
    );
    assert_eq!(
        restored_store
            .session_usage(session_id)
            .unwrap()
            .agents
            .len(),
        2
    );
}
