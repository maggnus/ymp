//! Inspection pages: one table per recorded subject and a description of the
//! selected record. Rows are read from the serialized projection, so a page shows
//! what the kernel recorded and nothing the interface computed on its own.
use super::conversation::printable;
use crate::app::{Member, Projection, SessionRow};
use serde_json::Value;

/// An unrecorded value is shown as a dash, never as zero or an empty cell.
pub const UNKNOWN: &str = "—";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKind {
    Sessions,
    Team,
    Criteria,
    Plan,
    Assignments,
    Commitments,
    Resources,
    Activity,
    Results,
    Checks,
    Acceptance,
    Report,
    Policies,
}
impl PageKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Sessions => "Sessions",
            Self::Team => "Team",
            Self::Criteria => "Criteria",
            Self::Plan => "Plan",
            Self::Assignments => "Assignments",
            Self::Commitments => "Commitments",
            Self::Resources => "Resources",
            Self::Activity => "Activity",
            Self::Results => "Results",
            Self::Checks => "Checks",
            Self::Acceptance => "Acceptance",
            Self::Report => "Report",
            Self::Policies => "Policies",
        }
    }
}
pub struct Column {
    pub title: &'static str,
    pub numeric: bool,
}
pub struct Record {
    pub key: String,
    pub cells: Vec<String>,
    pub detail: String,
}
pub struct Page {
    pub kind: PageKind,
    pub columns: Vec<Column>,
    pub records: Vec<Record>,
    /// Shown above the table: totals, or why the table is empty.
    pub note: String,
}
fn column(title: &'static str) -> Column {
    Column {
        title,
        numeric: false,
    }
}
fn number(title: &'static str) -> Column {
    Column {
        title,
        numeric: true,
    }
}
pub fn cell(value: &Value) -> String {
    match value {
        Value::Null => UNKNOWN.into(),
        Value::Bool(true) => "yes".into(),
        Value::Bool(false) => "no".into(),
        Value::Number(n) => match (n.as_u64(), n.as_f64()) {
            (Some(n), _) => n.to_string(),
            (_, Some(f)) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{f:.0}"),
            (_, Some(f)) => {
                let text = format!("{f:.3}");
                text.trim_end_matches('0').trim_end_matches('.').to_string()
            }
            _ => n.to_string(),
        },
        Value::String(s) if s.is_empty() => UNKNOWN.into(),
        // Recorded text is data: it is shown, never interpreted by the terminal.
        Value::String(s) => printable(s),
        Value::Array(items) if items.is_empty() => UNKNOWN.into(),
        Value::Array(items) => items.iter().map(cell).collect::<Vec<_>>().join(", "),
        // An externally tagged value such as {"Blocked":"decoding"}.
        Value::Object(map) if map.len() == 1 => {
            let (name, inner) = map.iter().next().expect("one entry");
            match inner {
                Value::Object(_) | Value::Array(_) => name.clone(),
                inner => format!("{name}({})", cell(inner)),
            }
        }
        Value::Object(map) => format!("{} fields", map.len()),
    }
}
/// Milliseconds since the epoch as a UTC time of day.
pub fn clock(value: &Value) -> String {
    match value.as_u64() {
        Some(ms) if ms > 0 => {
            let seconds = ms / 1000 % 86_400;
            format!(
                "{:02}:{:02}:{:02}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            )
        }
        _ => UNKNOWN.into(),
    }
}
fn short(text: &str, limit: usize) -> String {
    let line = text.lines().next().unwrap_or_default();
    if line.chars().count() > limit {
        let mut cut: String = line.chars().take(limit.saturating_sub(1)).collect();
        cut.push('…');
        cut
    } else if line.is_empty() {
        UNKNOWN.into()
    } else {
        line.into()
    }
}
/// A readable description of a recorded value, in the manner of a k9s describe view.
pub fn describe(value: &Value) -> String {
    let mut out = String::new();
    write(value, 0, &mut out);
    out
}
fn scalar(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::String(text) => !text.contains('\n'),
        _ => true,
    }
}
fn inline(value: &Value) -> String {
    match value {
        Value::Object(_) => "{}".into(),
        Value::Array(_) => "[]".into(),
        other => cell(other),
    }
}
fn write(value: &Value, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (name, inner) in map {
                if scalar(inner) {
                    out.push_str(&format!("{pad}{name}: {}\n", inline(inner)));
                } else {
                    out.push_str(&format!("{pad}{name}:\n"));
                    write(inner, depth + 1, out);
                }
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for inner in items {
                if scalar(inner) {
                    out.push_str(&format!("{pad}- {}\n", inline(inner)));
                } else {
                    out.push_str(&format!("{pad}-\n"));
                    write(inner, depth + 1, out);
                }
            }
        }
        Value::String(text) => {
            for line in text.lines() {
                out.push_str(&format!("{pad}{line}\n"));
            }
        }
        other => out.push_str(&format!("{pad}{}\n", inline(other))),
    }
}
fn entries(value: &Value) -> Vec<(String, &Value)> {
    match value {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        _ => vec![],
    }
}
fn record(key: String, cells: Vec<String>, detail: String) -> Record {
    Record { key, cells, detail }
}
/// Usage counters of a receipt, or dashes while no receipt is recorded.
fn usage(receipt: &Value) -> (String, String, String) {
    if receipt.is_null() {
        return (UNKNOWN.into(), UNKNOWN.into(), "no receipt".into());
    }
    (
        cell(&receipt["usage"]["input"]),
        cell(&receipt["usage"]["output"]),
        cell(&receipt["coverage"]),
    )
}
pub fn resources_note(json: &Value) -> String {
    let budget = &json["treasury"]["budget"];
    if budget.is_null() {
        return "No budget is recorded".into();
    }
    format!(
        "limit {} · spent {} · held {} · verification reserve {} · reporting reserve {} · unknown usage: {}",
        cell(&budget["limit"]),
        cell(&budget["spent"]),
        cell(&budget["held"]),
        cell(&budget["verification_reserve"]),
        cell(&budget["reporting_reserve"]),
        unknown_usage(json),
    )
}
/// Words for the accounting coverage of the whole session.
pub fn unknown_usage(json: &Value) -> &'static str {
    let receipts = entries(&json["execution"]["invocations"]);
    let incomplete = receipts.iter().any(|(_, record)| {
        !record["receipt"].is_null() && record["receipt"]["coverage"] != "Complete"
    });
    let ended_without_receipt = receipts
        .iter()
        .any(|(_, record)| record["receipt"].is_null() && !record["terminal"].is_null());
    // A call without a recorded end has used something nobody has reported yet.
    let open_without_receipt = receipts
        .iter()
        .any(|(_, record)| record["receipt"].is_null() && record["terminal"].is_null());
    if json["treasury"]["unsettled_usage"] == true
        || json["finalization"]["delivered"]["accounting"]["unknown"] == true
        || incomplete
        || ended_without_receipt
    {
        "present"
    } else if open_without_receipt {
        "a call has no receipt yet"
    } else {
        "none recorded"
    }
}
pub fn build(
    kind: PageKind,
    projection: Option<&Projection>,
    members: &[Member],
    sessions: &[SessionRow],
) -> Page {
    let null = Value::Null;
    let json = projection.map(|p| &p.json).unwrap_or(&null);
    let runs = |call: &Value| projection.is_some_and(|p| p.runs(call));
    let mut note = String::new();
    let (columns, records) = match kind {
        PageKind::Sessions => (
            vec![
                column("Session"),
                column("Status"),
                number("Revision"),
                column("Goal"),
            ],
            sessions
                .iter()
                .map(|row| {
                    record(
                        row.session.clone(),
                        vec![
                            row.session.clone(),
                            row.status.clone(),
                            row.revision.to_string(),
                            short(&row.goal, 60),
                        ],
                        format!(
                            "session: {}\nstatus: {}\nrevision: {}\ngoal:\n{}\n\nEnter opens the recorded session read-only.",
                            row.session, row.status, row.revision, row.goal
                        ),
                    )
                })
                .collect(),
        ),
        PageKind::Team => {
            let agents = entries(&json["registry"]["input"]["facts"]["agents"]);
            let columns = vec![
                column("Agent"),
                column("Provider"),
                column("Model"),
                column("Effort"),
                column("Pool"),
            ];
            if agents.is_empty() {
                note = "Agents offered by this composition; no pool is recorded yet".into();
                (
                    columns,
                    members
                        .iter()
                        .map(|m| {
                            let model = m.model.clone().unwrap_or_else(|| UNKNOWN.into());
                            let effort = m.effort.clone().unwrap_or_else(|| UNKNOWN.into());
                            record(
                                m.agent.clone(),
                                vec![
                                    m.agent.clone(),
                                    m.provider.clone(),
                                    model.clone(),
                                    effort.clone(),
                                    "not recorded".into(),
                                ],
                                format!(
                                    "agent: {}\nprovider: {}\nmodel: {model}\neffort: {effort}\n",
                                    m.agent, m.provider
                                ),
                            )
                        })
                        .collect(),
                )
            } else {
                let outcome = &json["registry"]["outcome"];
                (
                    columns,
                    agents
                        .into_iter()
                        .map(|(_, agent)| {
                            let id = cell(&agent["id"]);
                            let eligible = outcome["eligible"]
                                .as_array()
                                .is_some_and(|all| all.iter().any(|a| a == &agent["id"]));
                            let mut detail = describe(agent);
                            detail.push_str("\nrecorded offerings:\n");
                            detail.push_str(&describe(&outcome["offerings"][&id]));
                            if !outcome["excluded"][&id].is_null() {
                                detail.push_str("\nexcluded:\n");
                                detail.push_str(&describe(&outcome["excluded"][&id]));
                            }
                            record(
                                id.clone(),
                                vec![
                                    id,
                                    cell(&agent["provider"]),
                                    cell(&agent["defaults"]["model"]),
                                    cell(&agent["defaults"]["effort"]),
                                    if eligible { "eligible" } else { "excluded" }.into(),
                                ],
                                detail,
                            )
                        })
                        .collect(),
                )
            }
        }
        PageKind::Criteria => (
            vec![
                column("Criterion"),
                column("Status"),
                number("Belief"),
                column("Kind"),
                column("Required"),
                column("Origin"),
                column("Text"),
            ],
            entries(&json["criteria"])
                .into_iter()
                .map(|(_, criterion)| {
                    let id = cell(&criterion["id"]);
                    let entry = &json["ledger"]["entries"][&id];
                    let mut detail = describe(criterion);
                    detail.push_str("\nledger:\n");
                    detail.push_str(&if entry.is_null() {
                        format!("  {UNKNOWN} no assessment is recorded\n")
                    } else {
                        describe(entry)
                    });
                    for (name, check) in entries(&json["checks"]) {
                        if check["criterion"] == criterion["id"] {
                            detail.push_str(&format!("\ncheck {name}:\n{}", describe(check)));
                        }
                    }
                    record(
                        id.clone(),
                        vec![
                            id,
                            if entry.is_null() {
                                "not assessed".into()
                            } else {
                                cell(&entry["status"])
                            },
                            cell(&entry["belief"]),
                            cell(&criterion["kind"]),
                            cell(&criterion["required"]),
                            cell(&criterion["origin"]),
                            short(criterion["text"].as_str().unwrap_or_default(), 60),
                        ],
                        detail,
                    )
                })
                .collect(),
        ),
        PageKind::Plan => {
            note = format!(
                "plans: {}",
                cell(&Value::Array(
                    entries(&json["results"]["plans"])
                        .into_iter()
                        .map(|(name, _)| Value::String(name))
                        .collect()
                ))
            );
            (
                vec![
                    column("Work item"),
                    column("State"),
                    column("Title"),
                    column("Targets"),
                    column("Writes"),
                    number("Attempts"),
                    column("Accepted result"),
                ],
                entries(&json["results"]["items"])
                    .into_iter()
                    .map(|(name, item)| {
                        record(
                            name.clone(),
                            vec![
                                name,
                                cell(&item["state"]),
                                short(item["title"].as_str().unwrap_or_default(), 40),
                                cell(&item["targets"]),
                                cell(&item["writes"]),
                                item["attempts"]
                                    .as_array()
                                    .map_or(0, |a| a.len())
                                    .to_string(),
                                cell(&item["accepted"]),
                            ],
                            describe(item),
                        )
                    })
                    .collect(),
            )
        }
        PageKind::Assignments => (
            vec![
                column("Assignment"),
                column("Agent"),
                column("Role"),
                column("State"),
                column("Model"),
                column("Effort"),
                column("Lease until"),
            ],
            entries(&json["admission"]["assignments"])
                .into_iter()
                .map(|(name, entry)| {
                    let assignment = &entry["intent"]["assignment"];
                    record(
                        name.clone(),
                        vec![
                            name,
                            cell(&assignment["agent"]),
                            cell(&assignment["role"]),
                            cell(&assignment["state"]),
                            cell(&assignment["profile"]["model"]),
                            cell(&assignment["profile"]["effort"]),
                            clock(&entry["intent"]["lease"]["expires"]),
                        ],
                        describe(&entry["intent"]),
                    )
                })
                .collect(),
        ),
        PageKind::Commitments => (
            vec![
                column("Commitment"),
                column("Debtor"),
                column("Creditor"),
                column("State"),
                column("Lease until"),
                number("Renewals left"),
            ],
            entries(&json["coordination"]["commitments"])
                .into_iter()
                .map(|(name, entry)| {
                    record(
                        name.clone(),
                        vec![
                            name,
                            cell(&entry["debtor"]),
                            cell(&entry["creditor"]),
                            cell(&entry["state"]),
                            clock(&entry["lease"]["expires"]),
                            cell(&entry["lease"]["renewals_left"]),
                        ],
                        describe(entry),
                    )
                })
                .collect(),
        ),
        PageKind::Resources => {
            note = resources_note(json);
            (
                vec![
                    column("Account"),
                    column("Purpose"),
                    column("Agent"),
                    number("Reserved"),
                    column("State"),
                    number("Input"),
                    number("Output"),
                    column("Coverage"),
                    number("Cost"),
                ],
                entries(&json["treasury"]["accounts"])
                    .into_iter()
                    .map(|(name, account)| {
                        let (input, output, coverage) = usage(&account["receipt"]);
                        record(
                            name.clone(),
                            vec![
                                name,
                                cell(&account["reservation"]["purpose"]),
                                cell(&account["demand"]["profile"]["agent"]),
                                cell(&account["reservation"]["amount"]),
                                cell(&account["reservation"]["state"]),
                                input,
                                output,
                                coverage,
                                cell(&account["complete_cost"]),
                            ],
                            describe(account),
                        )
                    })
                    .collect(),
            )
        }
        PageKind::Activity => (
            vec![
                column("Invocation"),
                column("Agent"),
                column("Role"),
                column("Model"),
                column("Outcome"),
                number("Input"),
                number("Output"),
                column("Coverage"),
                column("Started"),
            ],
            {
                let mut all = entries(&json["execution"]["invocations"]);
                all.sort_by_key(|(_, record)| record["dispatch"]["at"].as_u64());
                all.into_iter()
                    .map(|(name, entry)| {
                        let assignment = &entry["dispatch"]["assignment"];
                        let (input, output, coverage) = usage(&entry["receipt"]);
                        let mut detail = format!(
                            "agent: {}\nrole: {}\nprovider: {}\noutcome: {}\nsettings requested: {}\nsettings sent: {}\nsettings reported: {}\n",
                            cell(&assignment["agent"]),
                            cell(&assignment["role"]),
                            cell(&entry["dispatch"]["provider"]),
                            outcome(entry, runs(entry)),
                            settings(&entry["invocation"]["settings"]["requested"]),
                            settings(&entry["invocation"]["settings"]["sent"]),
                            settings(&entry["invocation"]["settings"]["reported"]),
                        );
                        detail.push_str("\nreceipt:\n");
                        detail.push_str(&if entry["receipt"].is_null() {
                            format!("  {UNKNOWN} no receipt is recorded\n")
                        } else {
                            describe(&entry["receipt"])
                        });
                        detail.push_str("\nallowance:\n");
                        detail.push_str(&describe(&entry["dispatch"]["allowance"]));
                        detail.push_str("\ndiagnostics:\n");
                        detail.push_str(&describe(&entry["diagnostics"]));
                        detail.push_str("\nreported output (not a verified result):\n");
                        detail.push_str(&describe(&entry["output"]));
                        record(
                            name.clone(),
                            vec![
                                name,
                                cell(&assignment["agent"]),
                                cell(&assignment["role"]),
                                cell(&entry["dispatch"]["settings"]["model"]),
                                outcome(entry, runs(entry)),
                                input,
                                output,
                                coverage,
                                clock(&entry["dispatch"]["at"]),
                            ],
                            detail,
                        )
                    })
                    .collect()
            },
        ),
        PageKind::Results => (
            vec![
                column("Result"),
                column("Work item"),
                column("Producer"),
                column("Model"),
                column("Attempt outcome"),
                column("Artifacts"),
            ],
            entries(&json["results"]["results"])
                .into_iter()
                .map(|(name, result)| {
                    let attempt = entries(&json["results"]["attempts"])
                        .into_iter()
                        .find(|(_, a)| a["attempt"]["result"] == result["id"])
                        .map(|(_, a)| a.clone())
                        .unwrap_or(Value::Null);
                    let mut detail = describe(result);
                    detail.push_str("\nattempt:\n");
                    detail.push_str(&describe(&attempt));
                    record(
                        name.clone(),
                        vec![
                            name,
                            cell(&result["item"]),
                            cell(&result["producer"]),
                            cell(&result["profile"]["model"]),
                            cell(&attempt["attempt"]["outcome"]),
                            cell(&Value::Array(
                                entries(&result["artifacts"])
                                    .into_iter()
                                    .map(|(_, a)| a["path"].clone())
                                    .collect(),
                            )),
                        ],
                        detail,
                    )
                })
                .collect(),
        ),
        PageKind::Checks => {
            let runs = entries(&json["check_runs"]);
            let mut records: Vec<Record> = entries(&json["checks"])
                .into_iter()
                .filter(|(_, check)| !runs.iter().any(|(_, run)| run["check"] == check["id"]))
                .map(|(name, check)| {
                    record(
                        name.clone(),
                        vec![
                            name,
                            cell(&check["criterion"]),
                            UNKNOWN.into(),
                            UNKNOWN.into(),
                            "not run".into(),
                            UNKNOWN.into(),
                        ],
                        describe(check),
                    )
                })
                .collect();
            let mut runs = runs;
            runs.sort_by_key(|(_, run)| run["at"].as_u64());
            records.extend(runs.into_iter().map(|(name, run)| {
                let check = &json["checks"][run["check"].as_str().unwrap_or_default()];
                let mut detail = describe(run);
                detail.push_str("\ncheck:\n");
                detail.push_str(&describe(check));
                record(
                    name,
                    vec![
                        cell(&run["check"]),
                        cell(&check["criterion"]),
                        cell(&run["role"]),
                        cell(&run["target"]),
                        cell(&run["outcome"]),
                        clock(&run["at"]),
                    ],
                    detail,
                )
            }));
            (
                vec![
                    column("Check"),
                    column("Criterion"),
                    column("Run on"),
                    column("Target"),
                    column("Outcome"),
                    column("At"),
                ],
                records,
            )
        }
        PageKind::Acceptance => {
            let mut records: Vec<Record> = entries(&json["reviews"])
                .into_iter()
                .map(|(name, entry)| {
                    record(
                        name.clone(),
                        vec![
                            "review".into(),
                            name,
                            cell(&entry["review"]["result"]),
                            cell(&entry["review"]["reviewer"]),
                            cell(&entry["review"]["verdict"]),
                            UNKNOWN.into(),
                            clock(&entry["at"]),
                        ],
                        describe(entry),
                    )
                })
                .collect();
            records.extend(
                entries(&json["acceptances"])
                    .into_iter()
                    .map(|(name, entry)| {
                        record(
                            name.clone(),
                            vec![
                                "acceptance".into(),
                                name,
                                cell(&entry["acceptance"]["subject"]),
                                "Runtime".into(),
                                cell(&entry["acceptance"]["decision"]),
                                cell(&entry["acceptance"]["grade"]),
                                clock(&entry["acceptance"]["at"]),
                            ],
                            describe(entry),
                        )
                    }),
            );
            let last = &json["finalization"]["acceptance"];
            if !last.is_null() {
                records.push(record(
                    "final".into(),
                    vec![
                        "final acceptance".into(),
                        cell(&last["acceptance"]["id"]),
                        cell(&last["acceptance"]["subject"]),
                        "Runtime".into(),
                        cell(&last["acceptance"]["decision"]),
                        cell(&last["acceptance"]["grade"]),
                        clock(&last["acceptance"]["at"]),
                    ],
                    describe(last),
                ));
            } else {
                note = "No final acceptance is recorded".into();
            }
            (
                vec![
                    column("Kind"),
                    column("Record"),
                    column("Subject"),
                    column("By"),
                    column("Decision"),
                    column("Grade"),
                    column("At"),
                ],
                records,
            )
        }
        PageKind::Report => {
            let columns = vec![
                column("Report"),
                column("Outcome"),
                column("Grade"),
                number("Unmet"),
                number("Spent"),
                number("Held"),
                column("Unknown usage"),
            ];
            match projection.and_then(|p| p.view.finalization().delivered.as_ref()) {
                Some(delivered) => {
                    let recorded = &json["finalization"]["delivered"];
                    (
                        columns,
                        vec![record(
                            "report".into(),
                            vec![
                                cell(&recorded["report"]["id"]),
                                cell(&recorded["outcome"]),
                                cell(&recorded["report"]["grade"]),
                                recorded["report"]["unmet"]
                                    .as_array()
                                    .map_or(0, |a| a.len())
                                    .to_string(),
                                cell(&recorded["accounting"]["spent"]),
                                cell(&recorded["accounting"]["held"]),
                                cell(&recorded["accounting"]["unknown"]),
                            ],
                            ymp_runtime::kernel::finalization::render_report(delivered),
                        )],
                    )
                }
                None => {
                    note = "No report has been delivered".into();
                    (columns, vec![])
                }
            }
        }
        PageKind::Policies => (
            vec![
                column("Port"),
                column("Implementation"),
                column("Version"),
            ],
            entries(&json["policies"])
                .into_iter()
                .map(|(name, entry)| {
                    record(
                        name.clone(),
                        vec![
                            name,
                            cell(&entry["policy"]["impl"]),
                            cell(&entry["policy"]["version"]),
                        ],
                        describe(entry),
                    )
                })
                .collect(),
        ),
    };
    if records.is_empty() && note.is_empty() {
        note = "Nothing is recorded here yet".into();
    }
    Page {
        kind,
        columns,
        records,
        note,
    }
}
/// The recorded end of an invocation in words. Only a call this interface
/// drives can be said to work; a record alone says that no end is recorded.
pub fn outcome(entry: &Value, runs: bool) -> String {
    if !entry["terminal"].is_null() {
        cell(&entry["terminal"])
    } else if !entry["backend_terminal"].is_null() {
        "provider ended; not settled".into()
    } else if runs {
        "working".into()
    } else {
        "no end recorded".into()
    }
}
fn settings(value: &Value) -> String {
    if value.is_null() {
        return UNKNOWN.into();
    }
    format!(
        "model {} · effort {}",
        cell(&value["model"]),
        cell(&value["effort"])
    )
}
