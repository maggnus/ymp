#![allow(dead_code)]
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use ymp_domain::{
    Id, Proposal,
    journal::{Actor, Envelope, EscalationStep, Method, MethodKind, PolicySelection},
    task::*,
};
use ymp_kernel::{
    decision::{DecisionConsumer, MethodDecision},
    events::Event,
    journal::Journal,
};
use ymp_runtime::application::{Application, IntakeRequest};

pub struct Directory(pub PathBuf);
impl Directory {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "ymp-store-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("Cannot create isolated storage test directory: {error}"),
            }
        }
    }
    pub fn database(&self) -> PathBuf {
        self.0.join("journal.sqlite")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub fn policy(kind: &str) -> PolicySelection {
    PolicySelection::new(
        "MethodRouter",
        "FixedMethod",
        "1",
        serde_json::json!({"kind":kind,"ladder":["StopPreserving"]}),
    )
    .unwrap()
}
pub fn opening(session: &Id) -> Envelope<Event> {
    Envelope {
        seq: 1,
        session: session.clone(),
        at: 1,
        actor: Actor::Runtime,
        policy: None,
        input: None,
        refs: vec![],
        payload: Event::SessionOpened {
            version: 1,
            selections: vec![policy("SoloWithVerifier")],
        },
    }
}
pub fn populate<J: Journal>(journal: Arc<J>) {
    let session = Id::new("methods").unwrap();
    let consumer = DecisionConsumer::new(journal.clone());
    let first = policy("SoloWithVerifier");
    let owner = consumer
        .open(session.clone(), 1, vec![first.clone()])
        .unwrap();
    for (at, selection) in [(2, first), (3, policy("Solo"))] {
        let view = journal.view(&session, None).unwrap();
        let method = Method {
            id: Id::new("method").unwrap(),
            kind: if at == 2 {
                MethodKind::SoloWithVerifier
            } else {
                MethodKind::Solo
            },
            ladder: vec![EscalationStep::StopPreserving],
            params: BTreeMap::new(),
            policy: selection.policy.clone(),
        };
        let request = MethodDecision {
            expected_revision: view.revision(),
            at,
            input: view.digest().unwrap(),
            proposal: Proposal {
                value: method,
                rationale: "Record the selected method".into(),
                basis: vec![],
                policy: selection.policy.clone(),
            },
        };
        let change = if at == 2 {
            None
        } else {
            Some((&owner, selection))
        };
        consumer.record_method(&session, request, change).unwrap();
    }
    let app = Application::new(journal);
    let task = Task {
        id: Id::new("task").unwrap(),
        goal: Goal {
            request: "Preserve this exact request".into(),
            assumptions: vec![],
            clarifications: vec![],
        },
        contract: Id::new("contract").unwrap(),
        constraints: Constraints {
            budget: Real::new(98.31267718040647).unwrap(),
            verification_reserve: Real::new(10.0).unwrap(),
            deadline: None,
            pins: Pins::unrestricted(),
            allowed: BTreeSet::new(),
            parallel_limit: 1,
            attempt_limit: 2,
            max_members: 2,
        },
    };
    let criterion = Criterion {
        id: Id::new("criterion").unwrap(),
        text: "Keep the result exact".into(),
        kind: CriterionKind::Preserve,
        weight: Real::new(1.0).unwrap(),
        required: true,
        origin: CriterionOrigin::User,
        needs_class: BTreeSet::from([EvidenceClass::Executed]),
    };
    app.open(
        Id::new("intake").unwrap(),
        1,
        IntakeRequest {
            task,
            criteria: vec![criterion],
        },
        vec![policy("SoloWithVerifier")],
    )
    .unwrap();
}
