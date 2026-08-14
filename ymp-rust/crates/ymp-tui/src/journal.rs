//! Durable events in, screen data out.
//!
//! [`Model`] accumulates what the journal records and rebuilds the projection from it. It holds
//! an event cursor so a lagging reader catches up from the journal rather than assuming every
//! notification arrived (INV-6). Every field it produces is read from a domain value; a concept
//! the domain does not carry is written as an absence, never as a placeholder value.
//!
//! Two facts the design artifact draws have no application projection and are therefore absent
//! here rather than invented: journal events carry a sequence but no wall-clock time, and the
//! budget carries two dimensions rather than five.

use ratatui::text::Span;
use ymp_domain::{Budget, EventEnvelope, EventKind, RunState};

use crate::pages::{Body, Cell, Column, DescribeGroup, Page, Row};
use crate::projection::{
    self, AttemptFacts, BudgetDimension, CandidateFacts, CandidateVerdict, ContractFacts,
    Environment, EventFacts, Projection, RunFacts,
};
use crate::state::{Command, PageKind, PaletteItem};
use crate::style;
use crate::theme;
use crate::transcript::{Entry, Plane};

/// How many transcript entries are kept. Older ones are replaced by one counted elision, so a
/// long run stays bounded without pretending the earlier entries never happened.
const MAX_TRANSCRIPT_ENTRIES: usize = 4_000;

/// The accumulated read model of one data root.
#[derive(Clone, Debug)]
pub struct Model {
    environment: Environment,
    contracts: Vec<ContractFacts>,
    run: Option<RunFacts>,
    initial_budget: Option<Budget>,
    entries: Vec<Entry>,
    elided: usize,
    candidates: Vec<CandidateFacts>,
    attempts: Vec<AttemptFacts>,
    events: Vec<EventFacts>,
    /// The answer the interface is waiting for, while a request is being drafted.
    awaiting: Option<String>,
    /// What is running away from the thread that draws, while something is.
    working: Option<String>,
    /// Whether this store was refused because this binary cannot read it.
    refused: bool,
    /// The last journal position folded into this model.
    pub cursor: u64,
}

impl Model {
    /// An opening transcript for a data root, before any event is folded in.
    pub fn cold(environment: Environment, contracts: Vec<ContractFacts>) -> Self {
        let mut model = Self {
            environment,
            contracts,
            run: None,
            initial_budget: None,
            entries: Vec::new(),
            elided: 0,
            candidates: Vec::new(),
            attempts: Vec::new(),
            events: Vec::new(),
            awaiting: None,
            working: None,
            refused: false,
            cursor: 0,
        };
        model.push(Entry::Banner {
            version: model.environment.version.clone(),
        });
        model.push(Entry::Blank);
        model.push(Entry::AppReply {
            text: format!("store {}", model.environment.data_root.display()),
        });
        model.push(Entry::AppReply {
            text: format!(
                "assurance {} ▲ — {}",
                model.environment.assurance_profile, model.environment.assurance_limit
            ),
        });
        model
    }

    /// What the interface says when the store holds no run: the absence, and the next step.
    ///
    /// A store that was refused is a different case and says so: whether it holds a run is
    /// unknown to this binary, so nothing claims it is empty and nothing is offered over it.
    fn cold_start_entries(&self) -> Vec<Entry> {
        if self.refused {
            return vec![
                Entry::Blank,
                Entry::AppReply {
                    text: "this store was left exactly as it was found — whether it holds a run \
                           is not something this binary can read"
                        .into(),
                },
                Entry::AppReply {
                    text: "no request can be drafted here · point ymp at another store, or use a \
                           binary that reads this one's version"
                        .into(),
                },
                Entry::AppReply {
                    text: "?          key map".into(),
                },
            ];
        }
        vec![
            Entry::Blank,
            Entry::AppReply {
                text: "no run recorded for this project — nothing has been started and nothing \
                       has been spent"
                    .into(),
            },
            Entry::AppReply {
                text: contracts_hint(&self.contracts),
            },
            Entry::AppReply {
                text: "state your request below in one line · ymp asks only for what it cannot \
                       infer, and starts nothing until you authorize it"
                    .into(),
            },
            Entry::AppReply {
                text: "/runtimes  which runtime profiles this host can start".into(),
            },
            Entry::AppReply {
                text: "?          key map".into(),
            },
        ]
    }

    /// Fold committed events into the model and adopt the current run state.
    pub fn absorb(&mut self, state: &RunState, events: &[EventEnvelope]) {
        let mut terminal_reason = self
            .run
            .as_ref()
            .and_then(|run| run.terminal_reason.clone());

        for envelope in events {
            if envelope.sequence <= self.cursor {
                continue;
            }
            self.cursor = envelope.sequence;
            self.record(envelope, &mut terminal_reason);
        }

        self.run = Some(RunFacts::from_state(state, terminal_reason));
        for attempt in &mut self.attempts {
            attempt.active = state.active_attempts.contains(&attempt.attempt_id);
        }
    }

    fn record(&mut self, envelope: &EventEnvelope, terminal_reason: &mut Option<String>) {
        let (plane, kind, subject) = describe_event(envelope);
        self.events.push(EventFacts {
            sequence: envelope.sequence,
            plane,
            kind,
            subject: subject.clone(),
        });
        self.push(Entry::RunEvent {
            time: format!("#{:04}", envelope.sequence),
            plane,
            text: subject,
        });

        match &envelope.event {
            EventKind::RunStarted { budget } => {
                self.initial_budget = Some(budget.clone());
            }
            EventKind::ContractApproved { .. } => {}
            EventKind::AttemptStarted { attempt_id } => {
                self.attempts.push(AttemptFacts {
                    attempt_id: attempt_id.clone(),
                    started_at_sequence: envelope.sequence,
                    active: true,
                    candidates: 0,
                });
            }
            EventKind::CandidateSubmitted {
                attempt_id,
                base_digest,
                object_digest,
            } => {
                if let Some(attempt) = self
                    .attempts
                    .iter_mut()
                    .find(|attempt| &attempt.attempt_id == attempt_id)
                {
                    attempt.candidates += 1;
                }
                self.candidates.push(CandidateFacts {
                    sequence: envelope.sequence,
                    attempt_id: attempt_id.clone(),
                    base_digest: base_digest.clone(),
                    object_digest: object_digest.clone(),
                    verdict: None,
                });
            }
            EventKind::VerificationRecorded {
                candidate_digest,
                contract_digest,
                oracle_digest,
                evidence_digest,
                accepted,
            } => {
                if let Some(candidate) = self
                    .candidates
                    .iter_mut()
                    .find(|candidate| &candidate.object_digest == candidate_digest)
                {
                    candidate.verdict = Some(CandidateVerdict {
                        sequence: envelope.sequence,
                        accepted: *accepted,
                        evidence_digest: evidence_digest.clone(),
                        oracle_digest: oracle_digest.clone(),
                        contract_digest: contract_digest.clone(),
                    });
                }
            }
            EventKind::RunExhausted { reason }
            | EventKind::RunAbstained { reason }
            | EventKind::RunCancelled { reason }
            | EventKind::RunFailed { reason } => {
                *terminal_reason = Some(reason.clone());
            }
        }
    }

    fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
        if self.entries.len() > MAX_TRANSCRIPT_ENTRIES {
            let drop = self.entries.len() - MAX_TRANSCRIPT_ENTRIES;
            self.entries.drain(..drop);
            self.elided += drop;
        }
    }

    /// Append an interface reply to the transcript. The text is the interface speaking about
    /// what it just did; it carries no domain value that the journal does not already hold.
    pub fn reply(&mut self, text: impl Into<String>) {
        self.push(Entry::Blank);
        self.push(Entry::AppReply { text: text.into() });
    }

    pub fn human(&mut self, text: impl Into<String>) {
        self.push(Entry::Blank);
        self.push(Entry::Human { text: text.into() });
    }

    /// Append what the runtime doing the work reported, attributed to its profile.
    ///
    /// It carries no journal fact and is never counted as one: the durable record of the same
    /// invocation is the run's runtime evidence, and the events page reads the journal.
    pub fn runtime(&mut self, profile: impl Into<String>, text: impl Into<String>) {
        self.push(Entry::RuntimeNote {
            profile: profile.into(),
            text: text.into(),
        });
    }

    /// Record that this store cannot be read by this binary, so nothing is offered over it.
    pub fn refuse_store(&mut self) {
        self.refused = true;
    }

    /// Whether this store was refused as unreadable.
    pub fn store_refused(&self) -> bool {
        self.refused
    }

    /// State which answer the interface is waiting for, or that it waits for none.
    pub fn await_answer(&mut self, hint: Option<String>) {
        self.awaiting = hint;
    }

    /// State what is running away from the thread that draws, or that nothing is.
    pub fn working(&mut self, notice: Option<String>) {
        self.working = notice;
    }

    /// Report a refusal: what was not done, and why. Nothing here is a journal fact.
    pub fn error(&mut self, text: impl Into<String>) {
        self.push(Entry::Blank);
        self.push(Entry::AppError { text: text.into() });
    }

    /// Drop a contract the interface no longer offers, by name.
    ///
    /// An amended draft replaces the contract its earlier form produced: the operator is judging
    /// one draft, so the projection carries one contract for it rather than a pile of the
    /// versions it passed through.
    pub fn forget_contract(&mut self, contract_id: &str) {
        self.contracts
            .retain(|contract| contract.contract_id != contract_id);
    }

    /// Take a contract the application prepared, replacing an earlier draft of the same name.
    pub fn record_contract(&mut self, facts: ContractFacts) {
        match self
            .contracts
            .iter_mut()
            .find(|contract| contract.contract_id == facts.contract_id)
        {
            Some(existing) => *existing = facts,
            None => self.contracts.push(facts),
        }
    }

    pub fn run(&self) -> Option<&RunFacts> {
        self.run.as_ref()
    }

    pub fn contracts(&self) -> &[ContractFacts] {
        &self.contracts
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn candidates(&self) -> &[CandidateFacts] {
        &self.candidates
    }

    /// Build everything the drawing layer reads.
    pub fn projection(&self, runtimes: Option<&crate::runtimes::Report>) -> Projection {
        let mut entries = Vec::with_capacity(self.entries.len() + 1);
        if self.elided > 0 {
            entries.push(Entry::AppReply {
                text: format!(
                    "{} earlier transcript entries are not held in memory — /events reads the \
                     journal from its head",
                    self.elided
                ),
            });
        }
        entries.extend(self.entries.iter().cloned());
        if self.run.is_none() {
            entries.extend(self.cold_start_entries());
        }

        let mut pages: Vec<(PageKind, Page)> = Vec::new();
        pages.push((
            PageKind::Runtimes,
            crate::runtimes::page(runtimes, self.status_line()),
        ));
        if let Some(run) = &self.run {
            pages.push((PageKind::Candidates, self.candidates_page(run)));
            pages.push((PageKind::Events, self.events_page(run)));
            pages.push((PageKind::Budgets, self.budgets_page(run)));
            pages.push((PageKind::Attempts, self.attempts_page(run)));
        }

        Projection {
            environment: Some(self.environment.clone()),
            run: self.run.clone(),
            contracts: self.contracts.clone(),
            entries,
            commands: self.commands(&pages),
            pages,
            runtimes: runtimes.cloned(),
            // Where the work would go, and which store the next run belongs in, are the session's
            // to settle: it holds the profile the operator named and knows the root this store was
            // addressed under, and the journal records neither.
            addresses_a_store_of_its_own: false,
            route: None,
            route_note: String::new(),
            awaiting: self.awaiting.clone(),
            working: self.working.clone(),
            status: self.status_line(),
        }
    }

    /// Commands the current state actually offers, so no palette entry lands nowhere.
    fn commands(&self, pages: &[(PageKind, Page)]) -> Vec<PaletteItem> {
        let mut items: Vec<PaletteItem> = pages
            .iter()
            .map(|(kind, _)| PaletteItem {
                name: kind.command_name().to_owned(),
                description: page_description(*kind).to_owned(),
                command: Command::OpenPage(*kind),
            })
            .collect();

        // Whether authorizing this contract would start a run is not the palette's to say: it
        // turns on which store the next run belongs in, which the session settles and the journal
        // does not record. The entry offers the review; the coverage map states what follows it.
        for (index, contract) in self.contracts.iter().enumerate() {
            items.push(PaletteItem {
                name: format!("authorize {}", contract.contract_id),
                description: "review what this contract would have checked, before anything is \
                              spent"
                    .to_owned(),
                command: Command::Authorize(index),
            });
        }
        if let Some(run) = &self.run
            && run.is_live()
        {
            items.push(PaletteItem {
                name: format!("cancel {}", run.run_id),
                description: "end the live run — asks for typed confirmation".to_owned(),
                command: Command::CancelRun,
            });
        }
        // There is something to take out of the store only once a candidate exists: an export
        // carries the exact candidate and the evidence that judged it, and neither exists before.
        if let Some(run) = &self.run
            && run.candidate_digest.is_some()
        {
            items.push(PaletteItem {
                name: "export".into(),
                description: "write this run's candidate and verifier evidence out of the store"
                    .to_owned(),
                command: Command::Export,
            });
        }
        items.push(PaletteItem {
            name: "quit".into(),
            description: "leave ymp — the run state stays journaled".into(),
            command: Command::Quit,
        });
        items
    }

    /// The status line: what is true about the run right now.
    pub fn status_line(&self) -> String {
        if self.refused {
            return format!(
                "unreadable store · nothing changed · store {}",
                self.environment.data_root.display()
            );
        }
        match &self.run {
            None => format!(
                "idle · no run · store {}",
                self.environment.data_root.display()
            ),
            Some(run) => {
                let mut line = format!(
                    "{} {} {} · attempts left {} · verification queries left {} · ev {}",
                    run.run_id,
                    projection::outcome_marker(run.status),
                    projection::outcome(run.status),
                    run.budget.attempts_remaining,
                    run.budget.verification_queries_remaining,
                    run.last_sequence
                );
                if let Some(reason) = &run.terminal_reason {
                    line.push_str(&format!(" · {reason}"));
                }
                line
            }
        }
    }

    fn candidates_page(&self, run: &RunFacts) -> Page {
        let rows = self
            .candidates
            .iter()
            .map(|candidate| {
                let (verdict, style) = match &candidate.verdict {
                    Some(verdict) if verdict.accepted => ("accepted".to_owned(), theme::green()),
                    Some(_) => ("rejected".to_owned(), theme::red()),
                    None => ("unverified".to_owned(), theme::muted()),
                };
                Row {
                    cells: vec![
                        Cell::new(format!("#{:04}", candidate.sequence), theme::bold()),
                        Cell::new(candidate.attempt_id.clone(), theme::dim()),
                        Cell::new(
                            projection::short_digest(&candidate.base_digest),
                            theme::muted(),
                        ),
                        Cell::new(verdict, style),
                        Cell::new(
                            projection::short_digest(&candidate.object_digest),
                            theme::faint(),
                        ),
                    ],
                    fix: None,
                    dim: false,
                }
            })
            .collect::<Vec<_>>();

        let count = rows.len();
        Page {
            breadcrumb: vec![
                "transcript".into(),
                format!("candidates({})[{count}]", run.run_id),
            ],
            summary: Vec::new(),
            body: Body::Table {
                columns: vec![
                    Column {
                        title: "EVENT",
                        width: 8,
                    },
                    Column {
                        title: "ATTEMPT",
                        width: 16,
                    },
                    Column {
                        title: "BASE",
                        width: 14,
                    },
                    Column {
                        title: "VERIFICATION",
                        width: 14,
                    },
                    Column {
                        title: "DIGEST",
                        width: 0,
                    },
                ],
                rows,
            },
            notes: if count == 0 {
                vec!["empty — no candidate published yet; rows appear as attempts submit".into()]
            } else {
                vec![
                    "candidates are immutable snapshots; ymp never applies one to your working \
                     tree"
                        .into(),
                ]
            },
            footer: style::spans(&self.status_line(), theme::muted()),
            keys: vec![("Enter", "describe"), ("Esc", "back")],
            selected: count.saturating_sub(1),
        }
    }

    fn events_page(&self, run: &RunFacts) -> Page {
        let rows = self
            .events
            .iter()
            .map(|event| Row {
                cells: vec![
                    Cell::new(format!("#{:04}", event.sequence), theme::bold()),
                    Cell::new(event.plane.label(), plane_style(event.plane)),
                    Cell::new(event.kind, theme::dim()),
                    Cell::new(event.subject.clone(), theme::muted()),
                ],
                fix: None,
                dim: false,
            })
            .collect::<Vec<_>>();

        let count = rows.len();
        let mut summary = vec![Span::styled(" · ".to_owned(), theme::faint())];
        summary.extend(style::spans(
            &format!("head {} · append-only", run.last_sequence),
            theme::muted(),
        ));

        Page {
            breadcrumb: vec![
                "transcript".into(),
                format!("events({})[{count}]", run.run_id),
            ],
            summary,
            body: Body::Table {
                columns: vec![
                    Column {
                        title: "ID",
                        width: 8,
                    },
                    Column {
                        title: "PLANE",
                        width: 8,
                    },
                    Column {
                        title: "KIND",
                        width: 24,
                    },
                    Column {
                        title: "SUBJECT",
                        width: 0,
                    },
                ],
                rows,
            },
            notes: vec![
                "the journal is durable and append-only; plane labels never mix: ctrl control · \
                 verif protected verifier. the collaboration plane arrives with POC-2 and has no \
                 events today"
                    .into(),
            ],
            footer: style::spans(&self.status_line(), theme::muted()),
            keys: vec![("Esc", "back")],
            selected: count.saturating_sub(1),
        }
    }

    fn budgets_page(&self, run: &RunFacts) -> Page {
        budgets_page(
            &run.run_id,
            &run.budget_dimensions(self.initial_budget.as_ref()),
            self.status_line(),
        )
    }

    fn attempts_page(&self, run: &RunFacts) -> Page {
        let rows = self
            .attempts
            .iter()
            .map(|attempt| Row {
                cells: vec![
                    Cell::new(attempt.attempt_id.clone(), theme::bold()),
                    Cell::new(
                        if attempt.active { "active" } else { "finished" },
                        if attempt.active {
                            theme::green()
                        } else {
                            theme::muted()
                        },
                    ),
                    Cell::new(format!("#{:04}", attempt.started_at_sequence), theme::dim()),
                    Cell::new(attempt.candidates.to_string(), theme::dim()),
                ],
                fix: None,
                dim: false,
            })
            .collect::<Vec<_>>();

        let count = rows.len();
        Page {
            breadcrumb: vec![
                "transcript".into(),
                format!("attempts({})[{count}]", run.run_id),
            ],
            summary: Vec::new(),
            body: Body::Table {
                columns: vec![
                    Column {
                        title: "ATTEMPT",
                        width: 20,
                    },
                    Column {
                        title: "STATE",
                        width: 12,
                    },
                    Column {
                        title: "STARTED",
                        width: 10,
                    },
                    Column {
                        title: "CANDIDATES",
                        width: 0,
                    },
                ],
                rows,
            },
            notes: vec![
                if count == 0 {
                    "empty — no attempt has started under this run".into()
                } else {
                    "no ranking, no lead, no assignment: the page lists what the journal \
                     recorded"
                        .to_owned()
                },
                "participants, offers and the collaboration board arrive with POC-2 and are \
                 unavailable in this domain"
                    .into(),
            ],
            footer: style::spans(&self.status_line(), theme::muted()),
            keys: vec![("Esc", "back")],
            selected: count.saturating_sub(1),
        }
    }

    /// Field groups for one candidate, reached with Enter from the candidates page.
    pub fn describe_candidate(&self, index: usize) -> Option<Page> {
        let candidate = self.candidates.get(index)?;
        // Describing a candidate outside a run would have nothing to describe it against.
        self.run.as_ref()?;

        let value = |text: String, style: ratatui::style::Style| vec![Span::styled(text, style)];

        let mut groups = vec![
            DescribeGroup {
                title: "identity".into(),
                fields: vec![
                    (
                        "published".into(),
                        value(format!("#{:04}", candidate.sequence), theme::text()),
                    ),
                    (
                        "immutable".into(),
                        value(
                            "yes — content-addressed snapshot".to_owned(),
                            theme::green(),
                        ),
                    ),
                    (
                        "digest".into(),
                        value(candidate.object_digest.clone(), theme::text()),
                    ),
                ],
            },
            DescribeGroup {
                title: "producer".into(),
                fields: vec![(
                    "attempt".into(),
                    value(candidate.attempt_id.clone(), theme::text()),
                )],
            },
            DescribeGroup {
                title: "base".into(),
                fields: vec![(
                    "base".into(),
                    value(candidate.base_digest.clone(), theme::muted()),
                )],
            },
        ];

        groups.push(match &candidate.verdict {
            Some(verdict) => DescribeGroup {
                title: "verification".into(),
                fields: vec![
                    (
                        "verdict".into(),
                        value(
                            if verdict.accepted {
                                "accepted".to_owned()
                            } else {
                                "rejected".to_owned()
                            },
                            if verdict.accepted {
                                theme::green()
                            } else {
                                theme::red()
                            },
                        ),
                    ),
                    (
                        "recorded".into(),
                        value(format!("#{:04}", verdict.sequence), theme::dim()),
                    ),
                    (
                        "evidence".into(),
                        value(verdict.evidence_digest.clone(), theme::muted()),
                    ),
                    (
                        "oracle".into(),
                        value(
                            format!(
                                "{} — protected; unreadable to agents",
                                projection::short_digest(&verdict.oracle_digest)
                            ),
                            theme::muted(),
                        ),
                    ),
                    (
                        "contract".into(),
                        value(
                            projection::short_digest(&verdict.contract_digest),
                            theme::muted(),
                        ),
                    ),
                ],
            },
            None => DescribeGroup {
                title: "verification".into(),
                fields: vec![(
                    "verdict".into(),
                    value(
                        "unverified — no verification names this candidate".to_owned(),
                        theme::muted(),
                    ),
                )],
            },
        });

        Some(Page {
            breadcrumb: vec![
                "transcript".into(),
                "candidates".into(),
                format!("describe #{:04}", candidate.sequence),
            ],
            summary: Vec::new(),
            body: Body::Describe { groups },
            notes: vec![
                "not applied to your working tree — applying a candidate is a separate command \
                 under your own authority"
                    .into(),
            ],
            footer: style::spans(&self.status_line(), theme::muted()),
            keys: vec![("Esc", "back")],
            selected: 0,
        })
    }
}

/// The `:budgets` page over the dimensions a projection produced.
///
/// The class of a dimension is read from the dimension, never written here: a page that spelled
/// the class out would keep saying `enforced` after the projection stopped meaning it.
pub fn budgets_page(run_id: &str, dimensions: &[BudgetDimension], status: String) -> Page {
    let rows: Vec<Row> = dimensions
        .iter()
        .map(|dimension| Row {
            cells: vec![
                Cell::new(dimension.name, theme::bold()),
                Cell::new(
                    dimension.class.label(),
                    if dimension.class.gates() {
                        theme::green()
                    } else {
                        theme::muted()
                    },
                ),
                Cell::new(unknown_or(dimension.total), theme::muted()),
                Cell::new(unknown_or(dimension.used()), theme::dim()),
                Cell::new(dimension.remaining.to_string(), theme::text()),
            ],
            fix: None,
            dim: false,
        })
        .collect();

    Page {
        breadcrumb: vec![
            "transcript".into(),
            format!("budgets({run_id})[{}]", dimensions.len()),
        ],
        summary: Vec::new(),
        body: Body::Table {
            columns: vec![
                Column {
                    title: "DIMENSION",
                    width: 24,
                },
                Column {
                    title: "CLASS",
                    width: 11,
                },
                Column {
                    title: "TOTAL",
                    width: 9,
                },
                Column {
                    title: "USED",
                    width: 9,
                },
                Column {
                    title: "REMAINING",
                    width: 0,
                },
            ],
            rows,
        },
        notes: vec![
            "dimensions are independent — spare capacity in one never authorizes an action \
             blocked by another"
                .into(),
            "the domain carries these two dimensions today; cost, wall time and participant \
             starts are unavailable, not zero"
                .into(),
        ],
        footer: style::spans(&status, theme::muted()),
        keys: vec![("Esc", "back")],
        selected: 0,
    }
}

/// A number the journal does not carry is shown as unknown, never as zero.
fn unknown_or(value: Option<u32>) -> String {
    value.map_or_else(|| "—".to_owned(), |value| value.to_string())
}

fn plane_style(plane: Plane) -> ratatui::style::Style {
    match plane {
        Plane::Control => theme::muted(),
        Plane::Collaboration => theme::amber(),
        Plane::Verification => theme::green(),
    }
}

fn page_description(kind: PageKind) -> &'static str {
    match kind {
        PageKind::Runtimes => "which runtime profiles this host can start",
        PageKind::Candidates => "immutable candidates of the current run",
        PageKind::Events => "the durable journal — head and planes",
        PageKind::Budgets => "independent dimensions of the run budget",
        PageKind::Attempts => "attempts recorded under the current run",
        PageKind::Describe => "full field groups of the selection",
    }
}

fn contracts_hint(contracts: &[ContractFacts]) -> String {
    match contracts.len() {
        0 => "no contract drafted yet · one is drafted from the request you type below".to_owned(),
        1 => format!(
            "/authorize {} · nothing runs and nothing is spent until you authorize it",
            contracts[0].contract_id
        ),
        count => format!(
            "{count} managed contracts available · nothing runs and nothing is spent until you \
             authorize one"
        ),
    }
}

/// One durable event, stated as the fact it records. The kind is the domain's own.
fn describe_event(envelope: &EventEnvelope) -> (Plane, &'static str, String) {
    match &envelope.event {
        EventKind::RunStarted { budget } => (
            Plane::Control,
            "run.started",
            format!(
                "run {} started · attempts {} · verification queries {}",
                envelope.run_id, budget.attempts_remaining, budget.verification_queries_remaining
            ),
        ),
        EventKind::ContractApproved {
            contract_id,
            contract_digest,
            oracle_digest,
        } => (
            Plane::Control,
            "contract.approved",
            format!(
                "contract {contract_id} approved · digest {} · oracle {} — this run is judged \
                 against it and against nothing else",
                projection::short_digest(contract_digest),
                projection::short_digest(oracle_digest)
            ),
        ),
        EventKind::AttemptStarted { attempt_id } => (
            Plane::Control,
            "attempt.started",
            format!("attempt {attempt_id} started · private copy of the base snapshot"),
        ),
        EventKind::CandidateSubmitted {
            attempt_id,
            base_digest,
            object_digest,
        } => (
            Plane::Control,
            "candidate.submitted",
            format!(
                "candidate published · {attempt_id} · base {} · digest {}",
                projection::short_digest(base_digest),
                projection::short_digest(object_digest)
            ),
        ),
        EventKind::VerificationRecorded {
            candidate_digest,
            accepted,
            evidence_digest,
            ..
        } => (
            Plane::Verification,
            "verification.recorded",
            format!(
                "candidate {} {} · evidence {}",
                projection::short_digest(candidate_digest),
                if *accepted { "accepted" } else { "rejected" },
                projection::short_digest(evidence_digest)
            ),
        ),
        EventKind::RunExhausted { reason } => (
            Plane::Control,
            "run.exhausted",
            format!("run terminal: exhausted · {reason}"),
        ),
        EventKind::RunAbstained { reason } => (
            Plane::Control,
            "run.abstained",
            format!("run terminal: abstained · {reason}"),
        ),
        EventKind::RunCancelled { reason } => (
            Plane::Control,
            "run.cancelled",
            format!("run terminal: cancelled · {reason}"),
        ),
        EventKind::RunFailed { reason } => (
            Plane::Control,
            "run.failed",
            format!("run terminal: infrastructure_error · {reason}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ymp_domain::RunStatus;

    use super::*;

    fn environment() -> Environment {
        Environment {
            version: "0.1.0".into(),
            project: "checkout".into(),
            project_path: PathBuf::from("/tmp/checkout"),
            data_root: PathBuf::from("/tmp/checkout/.ymp-data"),
            assurance_profile: projection::ASSURANCE_PROFILE.to_owned(),
            assurance_limit: projection::ASSURANCE_LIMIT.to_owned(),
        }
    }

    fn envelope(sequence: u64, event: EventKind) -> EventEnvelope {
        EventEnvelope {
            schema_version: 1,
            run_id: "demo-run".into(),
            sequence,
            command_id: format!("cmd-{sequence}"),
            command_digest: "0".repeat(64),
            predecessor_digest: None,
            event,
            digest: "1".repeat(64),
        }
    }

    fn state(status: RunStatus, last_sequence: u64) -> RunState {
        RunState {
            run_id: "demo-run".into(),
            status,
            budget: Budget::new(2, 1),
            contract: None,
            active_attempts: Vec::new(),
            candidate_digest: None,
            last_sequence,
            last_event_digest: "1".repeat(64),
        }
    }

    #[test]
    fn folding_the_same_batch_twice_does_not_duplicate_the_transcript() {
        let events = vec![envelope(
            1,
            EventKind::AttemptStarted {
                attempt_id: "attempt-1".into(),
            },
        )];
        let mut model = Model::cold(environment(), Vec::new());
        let before = model.entries.len();
        model.absorb(&state(RunStatus::Running, 1), &events);
        model.absorb(&state(RunStatus::Running, 1), &events);
        assert_eq!(model.entries.len(), before + 1);
        assert_eq!(model.attempts.len(), 1);
    }

    #[test]
    fn a_verdict_binds_to_the_candidate_it_names() {
        let events = vec![
            envelope(
                1,
                EventKind::CandidateSubmitted {
                    attempt_id: "attempt-1".into(),
                    base_digest: "4".repeat(64),
                    object_digest: "9".repeat(64),
                },
            ),
            envelope(
                2,
                EventKind::VerificationRecorded {
                    candidate_digest: "9".repeat(64),
                    contract_digest: "c".repeat(64),
                    oracle_digest: "0".repeat(64),
                    evidence_digest: "e".repeat(64),
                    accepted: false,
                },
            ),
        ];
        let mut model = Model::cold(environment(), Vec::new());
        model.absorb(&state(RunStatus::Running, 2), &events);
        let verdict = model.candidates[0].verdict.as_ref().expect("verdict");
        assert!(!verdict.accepted);
    }

    #[test]
    fn a_cancelled_run_states_the_outcome_and_its_recorded_reason() {
        let events = vec![envelope(
            1,
            EventKind::RunCancelled {
                reason: "operator cancelled the run from the terminal".into(),
            },
        )];
        let mut model = Model::cold(environment(), Vec::new());
        model.absorb(&state(RunStatus::Cancelled, 1), &events);
        let line = model.status_line();
        assert!(line.contains("cancelled"), "{line}");
        assert!(line.contains("operator cancelled"), "{line}");
        assert!(!line.contains("infrastructure_error"), "{line}");
    }

    #[test]
    fn a_transcript_longer_than_the_bound_states_what_it_dropped() {
        let mut model = Model::cold(environment(), Vec::new());
        let events: Vec<_> = (1..=(MAX_TRANSCRIPT_ENTRIES as u64 + 50))
            .map(|sequence| {
                envelope(
                    sequence,
                    EventKind::AttemptStarted {
                        attempt_id: format!("attempt-{sequence}"),
                    },
                )
            })
            .collect();
        model.absorb(&state(RunStatus::Running, events.len() as u64), &events);
        let projection = model.projection(None);
        assert_eq!(model.entries.len(), MAX_TRANSCRIPT_ENTRIES);
        let Entry::AppReply { text } = &projection.entries[0] else {
            panic!("expected the elision note first");
        };
        assert!(text.contains("earlier transcript entries"), "{text}");
    }

    #[test]
    fn a_cold_store_offers_runtimes_and_quit_but_no_run_commands() {
        let model = Model::cold(environment(), Vec::new());
        let projection = model.projection(None);
        let names: Vec<&str> = projection
            .commands
            .iter()
            .map(|item| item.name.as_str())
            .collect();
        assert!(names.contains(&"runtimes"), "{names:?}");
        assert!(names.contains(&"quit"), "{names:?}");
        assert!(!names.iter().any(|name| name.starts_with("cancel")));
    }
}
