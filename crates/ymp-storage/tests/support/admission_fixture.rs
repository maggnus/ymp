//! Admission integration uses Scripted facts and real services, without starting a backend.
use super::Directory;
use super::fixture;
use super::storage::{content::SqliteContent, journal::SqliteJournal};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use ymp_domain::{
    Digest, Id, Prob, Ref, assignment::*, coordination::*, identity::ExecutionProfile,
    journal::Capability, resources::*, task::Real, workspace::*,
};
use ymp_kernel::{
    arbiter::{Arbiter, award_view},
    gatekeeper::{AdmissionRequest, Gatekeeper},
    journal::Journal,
    ports::{
        organization::AwardPolicy,
        resources::{CostModel, ResourcePolicy},
    },
    treasury::*,
};
use ymp_runtime::{
    policies::{award::FirstOffer, resources::*},
    workspace::direct::Direct,
};
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn n(n: f64) -> Real {
    Real::new(n).unwrap()
}
pub(super) struct Setup<J: Journal> {
    pub(super) intake: ymp_kernel::intake::Intake<J>,
    pub(super) control: ymp_kernel::decision::SessionControl,
    pub(super) journal: Arc<J>,
    pub(super) gate: Gatekeeper<J, SqliteContent>,
    pub(super) provider: Arc<Direct>,
    pub(super) profile: ExecutionProfile,
    pub(super) cost: PriceWeighted,
    pub(super) resource: PurposeBounded,
    pub(super) award: FirstOffer,
    pub(super) session: Id,
    pub(super) files: bool,
}
impl<J: Journal> Setup<J> {
    pub(super) fn new(
        journal: Arc<J>,
        storage: &SqliteJournal,
        root: &Directory,
        files: bool,
    ) -> Self {
        Self::new_named(journal, storage, root, files, "admission")
    }
    pub(super) fn new_named(
        journal: Arc<J>,
        storage: &SqliteJournal,
        root: &Directory,
        files: bool,
        name: &str,
    ) -> Self {
        Self::new_config(journal, storage, root, files, name, None)
    }
    pub(super) fn new_config(
        journal: Arc<J>,
        storage: &SqliteJournal,
        root: &Directory,
        files: bool,
        name: &str,
        constraints: Option<ymp_domain::task::Constraints>,
    ) -> Self {
        Self::new_with_award(
            journal,
            storage,
            root,
            files,
            name,
            constraints,
            FirstOffer::with_commitment_terms(CommitmentTerms {
                lease_duration: 20,
                renewal_duration: 20,
                renew_on: BTreeSet::new(),
                renewals: 0,
                release_delta: n(1.0),
            })
            .unwrap(),
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new_with_award(
        journal: Arc<J>,
        storage: &SqliteJournal,
        root: &Directory,
        files: bool,
        name: &str,
        constraints: Option<ymp_domain::task::Constraints>,
        award: FirstOffer,
    ) -> Self {
        Self::new_with_selections(
            journal,
            storage,
            root,
            files,
            name,
            constraints,
            award,
            vec![],
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new_with_selections(
        journal: Arc<J>,
        storage: &SqliteJournal,
        root: &Directory,
        files: bool,
        name: &str,
        constraints: Option<ymp_domain::task::Constraints>,
        award: FirstOffer,
        mut selections: Vec<ymp_domain::journal::PolicySelection>,
    ) -> Self {
        let provider = Arc::new(Direct::open(&root.0, CaptureLimits::default()).unwrap());
        let cost = PriceWeighted::new(PriceWeightedParameters {
            expected_input: 10,
            expected_output: 0,
            p90_factor: n(1.0),
        })
        .unwrap();
        let resource = PurposeBounded::new(PurposeBoundedParameters {
            max_cost: n(100.0),
            timeout: 100,
            native_turns: 2,
            output_chars: 1000,
            report_call_cost: n(10.0),
        })
        .unwrap();
        let session = id(name);
        let caps = if files {
            BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles])
        } else {
            BTreeSet::new()
        };
        let constraints = constraints.unwrap_or_else(|| fixture::default_constraints(caps.clone()));
        selections.extend([
            cost.selection().clone(),
            resource.selection().clone(),
            award.selection().clone(),
        ]);
        let opening = fixture::open_with_options(
            journal.clone(),
            Arc::new(storage.content_store()),
            &session,
            provider.as_ref(),
            caps,
            selections,
            constraints,
        );
        let profile = opening.profile;
        let gate = Gatekeeper::new(journal.clone(), Arc::new(storage.content_store()));
        if files {
            gate.workspace()
                .bind_workspace(&session, 4, 4, &id("workspace"), provider.as_ref())
                .unwrap();
        }
        let view = gate.view(&session).unwrap();
        let at = view.latest_at() + 1;
        Treasury::new(journal.clone())
            .open(
                &session,
                view.revision(),
                at,
                BudgetRequest {
                    id: id("budget"),
                    pricebook: PriceBook {
                        version: "relative-v1".into(),
                        rates: vec![],
                        fallback: Rates::fallback(),
                    },
                    unknown_usage: UnknownUsage::Stop,
                    reporting: reporting_response(&resource, &reporting_view(&view, at).unwrap())
                        .unwrap(),
                },
            )
            .unwrap();
        Self {
            intake: opening.intake,
            control: opening.control,
            journal,
            gate,
            provider,
            profile,
            cost,
            resource,
            award,
            session,
            files,
        }
    }
    pub(super) fn award(&self, name: &str) -> Ref {
        self.award_subject(name, None)
    }
    pub(super) fn award_subject(&self, name: &str, subject: Option<ContributionSubject>) -> Ref {
        let kind = if self.files {
            ContributionKind::Produce
        } else {
            ContributionKind::Plan
        };
        let needs = if self.files {
            BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles])
        } else {
            BTreeSet::new()
        };
        self.award_kind(name, kind, needs, subject)
    }
    pub(super) fn award_kind(
        &self,
        name: &str,
        kind: ContributionKind,
        needs: BTreeSet<Capability>,
        subject: Option<ContributionSubject>,
    ) -> Ref {
        let arbiter = Arbiter::new(self.journal.clone());
        let view = self.gate.view(&self.session).unwrap();
        let at = view.latest_at() + 1;
        let forecast = Forecast {
            p_success: Prob::new(0.5).unwrap(),
            delta_belief: BTreeMap::new(),
            source: ForecastSource::Model(self.cost.selection().policy.clone()),
        };
        arbiter
            .propose(
                &self.session,
                view.revision(),
                at,
                Contribution {
                    id: id(name),
                    session: self.session.clone(),
                    kind,
                    targets: BTreeSet::from([id("criterion")]),
                    subject,
                    needs,
                    forecast: forecast.clone(),
                    cost: CostEstimate {
                        expected: n(10.0),
                        p90: n(10.0),
                    },
                    difficulty: Difficulty::Simple,
                    proposed_by: ContributionAuthor::Runtime,
                    basis: vec![],
                },
            )
            .unwrap();
        let revision = self.gate.view(&self.session).unwrap().revision();
        arbiter
            .open(
                &self.session,
                revision,
                at + 1,
                Solicitation {
                    id: id(name),
                    contribution: id(name),
                    stimulus: n(1.0),
                    deadline: at + 2,
                    eligible: BTreeSet::from([self.profile.agent.clone()]),
                    visibility: SolicitationVisibility::Open,
                    reopened: 0,
                    state: SolicitationState::Open,
                },
            )
            .unwrap();
        arbiter
            .submit(
                &self.session,
                revision + 1,
                at + 2,
                Offer {
                    id: id(name),
                    solicitation: id(name),
                    agent: self.profile.agent.clone(),
                    profile: self.profile.clone(),
                    forecast,
                    cost: CostEstimate {
                        expected: n(10.0),
                        p90: n(10.0),
                    },
                    approach: "Explicit Scripted fixture".into(),
                    source: OfferSource::RuntimeProxy,
                    at: at + 2,
                },
            )
            .unwrap();
        let view = arbiter.view(&self.session).unwrap();
        let input = award_view(&view, &id(name), at + 2).unwrap();
        arbiter
            .award_with_terms(
                &self.session,
                view.revision(),
                at + 2,
                self.award.award(&input).unwrap(),
                Digest::of_value(&input).unwrap(),
                Commitment {
                    id: id(name),
                    debtor: self.profile.agent.clone(),
                    creditor: Creditor::Runtime,
                    subject: id(name),
                    condition: None,
                    lease: Lease {
                        expires: at + 60,
                        renew_on: BTreeSet::new(),
                        renewals_left: 0,
                    },
                    state: CommitmentState::Proposed,
                    history: vec![],
                },
                self.award.commitment_terms(&input).ok(),
            )
            .unwrap();
        self.gate
            .view(&self.session)
            .unwrap()
            .coordination()
            .awards()[&id(name)]
            .reference
            .clone()
    }
    pub(super) fn request(&self, name: &str, award: Ref) -> (u64, u64, AdmissionRequest) {
        self.request_path(name, award, "file")
    }
    pub(super) fn request_path(
        &self,
        name: &str,
        award: Ref,
        path: &str,
    ) -> (u64, u64, AdmissionRequest) {
        self.request_with_gate(&self.gate, name, award, path)
    }
    pub(super) fn request_with_gate(
        &self,
        gate: &Gatekeeper<J, SqliteContent>,
        name: &str,
        award: Ref,
        path: &str,
    ) -> (u64, u64, AdmissionRequest) {
        let view = gate.view(&self.session).unwrap();
        let at = view.latest_at() + 1;
        let source = view
            .coordination()
            .awards()
            .values()
            .find(|value| value.reference == award)
            .unwrap();
        let contribution =
            &view.coordination().contributions()[&source.value.commitment.subject].value;
        let demand = ResourceDemand {
            contribution: contribution.id.erased(),
            kind: contribution.kind,
            difficulty: contribution.difficulty,
            provider: id("provider"),
            profile: self.profile.clone(),
        };
        let estimate =
            estimate_response(&self.cost, &estimate_view(&view, &demand).unwrap()).unwrap();
        let allowance = allowance_response(
            &self.resource,
            &allowance_view(&view, &demand, estimate.proposal.value.clone(), at).unwrap(),
        )
        .unwrap();
        let lease = source
            .value
            .terms
            .as_ref()
            .map(|decision| {
                decision
                    .outcome
                    .initial_lease(at, allowance.proposal.value.timeout)
                    .unwrap()
            })
            .unwrap_or(Lease {
                expires: at + 20,
                renew_on: BTreeSet::new(),
                renewals_left: 0,
            });
        let access = if self.files {
            BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles])
        } else {
            BTreeSet::new()
        };
        let files = if self.files {
            Some(
                gate.workspace()
                    .prepare_mediation(
                        &self.session,
                        view.revision(),
                        at,
                        ymp_kernel::workspace_guard::LockRequest {
                            assignment: id(name),
                            workspace: id("workspace"),
                            profile: self.profile.clone(),
                            paths: vec![
                                (WorkspacePath::new(path).unwrap(), LockMode::Read),
                                (WorkspacePath::new(path).unwrap(), LockMode::Write),
                            ],
                        },
                        self.provider.clone(),
                    )
                    .unwrap(),
            )
        } else {
            None
        };
        (
            view.revision(),
            at,
            AdmissionRequest {
                assignment: id(name),
                award,
                role: if self.files {
                    RoleKind::Producer
                } else {
                    RoleKind::Planner
                },
                workspace: id("workspace"),
                access,
                reservation: id(name),
                grant: id(name),
                operations: BTreeSet::from([TeamOperation::BoardRead]),
                lease,
                estimate,
                allowance,
                files,
            },
        )
    }
    pub(super) fn switch_to_new_agent(&mut self, name: &str) {
        let view = self.gate.view(&self.session).unwrap();
        let at = view.latest_at() + 1;
        let prior = view.registry().unwrap().clone();
        let mut facts = prior.input.facts;
        let mut agent = facts.agents[0].clone();
        agent.id = id(name);
        facts.agents.push(agent);
        let registry = ymp_kernel::registry::Registry::new(self.journal.clone());
        let input = registry.prepare(&self.session, facts, at).unwrap();
        let responses = ymp_kernel::registry::readiness_views(&input)
            .iter()
            .map(|view| ymp_kernel::registry::ReadinessResponse {
                profile: view.profile.clone(),
                input: Digest::of_value(view).unwrap(),
                proposal: ymp_domain::Proposal {
                    value: ymp_domain::identity::Readiness::Ready,
                    rationale: "Additional eligible Scripted agent".into(),
                    basis: vec![],
                    policy: prior.effective.policy.clone(),
                },
            })
            .collect();
        registry
            .record(
                &self.session,
                view.revision(),
                at,
                input,
                prior.effective,
                responses,
            )
            .unwrap();
        self.profile = registry
            .profile(
                &self.session,
                &id(name),
                &ymp_domain::identity::ProfileSettings::default(),
            )
            .unwrap();
    }
}
