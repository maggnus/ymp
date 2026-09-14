use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub team: Vec<crate::AgentProfile>,
    #[serde(default)]
    pub turns_used: usize,
}

/// What a recorded check says about its own result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    Passed,
    Failed,
    /// The record carries no result. An older or partial record is not a pass.
    Unrecorded,
}

/// One acceptance command the runtime ran itself, as a session recorded it.
///
/// Every field is read back from the stored event. A value the record does not carry stays
/// absent instead of being replaced by a default, so a reader cannot mistake a missing
/// result for a successful one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRun {
    /// Position in the session log. Two runs of the same command stay distinguishable.
    pub seq: i64,
    pub command: Option<String>,
    /// The task attempt the run was recorded for. The final pass over every declared
    /// command is recorded without one, and so is any record written before runs carried
    /// one, so absence means unscoped rather than belonging to no task.
    pub task: Option<crate::TaskAttemptRef>,
    /// The directory the command ran in, when the record names one.
    pub directory: Option<String>,
    pub outcome: CheckOutcome,
    /// Captured output, already truncated by the runtime that wrote it.
    pub output: Option<String>,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub seq: i64,
    pub session_id: String,
    pub author: String,
    pub recipient: Option<String>,
    pub kind: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Ready,
    Running,
    Review,
    Accepted,
    Blocked,
}

/// Requested filesystem authority, independent of the task's competence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskAccess {
    ReadOnly,
    #[default]
    Write,
}

impl TaskAccess {
    pub fn is_write(&self) -> bool {
        *self == Self::Write
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    #[serde(default, skip_serializing_if = "TaskAccess::is_write")]
    pub access: TaskAccess,
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub description: String,
    pub competence: String,
    pub difficulty: String,
    pub dependencies: Vec<String>,
    pub checks: Vec<String>,
    pub state: TaskState,
    pub assignee: Option<String>,
    pub reviewer: Option<String>,
    pub attempts: usize,
    pub result: Option<String>,
    pub workspace: Option<PathBuf>,
    #[serde(default)]
    pub base_commit: Option<String>,
    #[serde(default)]
    pub interrupted: bool,
}

impl Task {
    pub fn assign(&mut self, agent: &str, accepted: &HashSet<String>) -> Result<()> {
        if self.state != TaskState::Ready {
            bail!("Task is not ready");
        }
        if !self.dependencies.iter().all(|id| accepted.contains(id)) {
            bail!("Dependencies are not accepted");
        }
        self.state = TaskState::Running;
        self.assignee = Some(agent.into());
        self.attempts += 1;
        self.interrupted = false;
        Ok(())
    }
    pub fn submit(&mut self, actor: &str, result: String) -> Result<()> {
        if self.state != TaskState::Running || self.assignee.as_deref() != Some(actor) {
            bail!("Only the assigned running agent may submit");
        }
        self.result = Some(result);
        self.state = TaskState::Review;
        Ok(())
    }
    pub fn review(&mut self, actor: &str, approved: bool, limit: usize) -> Result<()> {
        if self.state != TaskState::Review {
            bail!("Task is not awaiting review");
        }
        if self.assignee.as_deref() == Some(actor) {
            bail!("Self-acceptance is forbidden");
        }
        self.reviewer = Some(actor.into());
        self.state = if approved {
            TaskState::Accepted
        } else if self.attempts >= limit {
            TaskState::Blocked
        } else {
            TaskState::Ready
        };
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanTask {
    #[serde(default, skip_serializing_if = "TaskAccess::is_write")]
    pub access: TaskAccess,
    pub title: String,
    pub description: String,
    #[serde(default = "implementation")]
    pub competence: String,
    #[serde(default = "standard")]
    pub difficulty: String,
    /// Indexes in this plan, not database ids.
    #[serde(default)]
    pub dependencies: Vec<usize>,
    #[serde(default)]
    pub checks: Vec<String>,
}
fn implementation() -> String {
    "implementation".into()
}
fn standard() -> String {
    "standard".into()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub summary: String,
    pub tasks: Vec<PlanTask>,
}
impl Plan {
    pub fn validate(&self) -> Result<()> {
        if self.tasks.is_empty() || self.tasks.len() > 24 {
            bail!("Plan must contain 1..24 tasks");
        }
        for (i, task) in self.tasks.iter().enumerate() {
            if task.title.trim().is_empty() || task.description.trim().is_empty() {
                bail!("Task needs a title and description");
            }
            if ![
                "analysis",
                "planning",
                "implementation",
                "verification",
                "synthesis",
            ]
            .contains(&task.competence.as_str())
            {
                bail!("Unknown competence");
            }
            if !["simple", "standard", "complex"].contains(&task.difficulty.as_str()) {
                bail!("Unknown difficulty");
            }
            if task
                .dependencies
                .iter()
                .any(|&d| d >= self.tasks.len() || d == i)
            {
                bail!("Invalid dependency");
            }
        }
        let mut done = HashSet::new();
        loop {
            let before = done.len();
            for (i, t) in self.tasks.iter().enumerate() {
                if t.dependencies.iter().all(|d| done.contains(d)) {
                    done.insert(i);
                }
            }
            if done.len() == self.tasks.len() {
                return Ok(());
            }
            if done.len() == before {
                bail!("Cyclic task dependencies");
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub approved: bool,
    pub reason: String,
    #[serde(default)]
    pub lesson: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    #[serde(default)]
    pub provenance: Option<crate::KnowledgeProvenance>,
    pub id: String,
    pub project_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub source_session: String,
    pub author: String,
    pub reviewer: Option<String>,
    pub status: String,
    pub created_at: String,
    #[serde(default)]
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Older records lack objective provenance; deserialization preserves that
    /// distinction instead of treating historical success as confirmation.
    #[serde(default = "unknown_confirmation")]
    pub confirmation: crate::ConfirmationStatus,
    pub id: String,
    pub agent_version: String,
    pub agent_name: String,
    pub competence: String,
    pub difficulty: String,
    pub success: bool,
    pub evidence: String,
    pub created_at: String,
}

fn unknown_confirmation() -> crate::ConfirmationStatus {
    crate::ConfirmationStatus::Unknown
}

fn unquoted_member_name(bytes: &[u8]) -> Option<&[u8]> {
    let mut cursor = bytes
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        .unwrap_or(bytes.len());
    if cursor == bytes.len() || !matches!(bytes[cursor], b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$') {
        return None;
    }
    let start = cursor;
    cursor += 1;
    while cursor < bytes.len()
        && matches!(
            bytes[cursor],
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'$'
        )
    {
        cursor += 1;
    }
    let end = cursor;
    while cursor < bytes.len() && matches!(bytes[cursor], b' ' | b'\t') {
        cursor += 1;
    }
    (bytes.get(cursor) == Some(&b':')).then_some(&bytes[start..end])
}

fn starts_single_quoted_member(bytes: &[u8]) -> bool {
    let mut escaped = false;
    let Some(end) = bytes[1..].iter().position(|byte| {
        if escaped {
            escaped = false;
            false
        } else if *byte == b'\\' {
            escaped = true;
            false
        } else {
            *byte == b'\''
        }
    }) else {
        return false;
    };
    let mut cursor = end + 2;
    while cursor < bytes.len() && matches!(bytes[cursor], b' ' | b'\t') {
        cursor += 1;
    }
    bytes.get(cursor) == Some(&b':')
}

fn has_multiple_unquoted_members(bytes: &[u8], first: usize) -> bool {
    let mut nested = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut css_declaration_ended = false;
    for (index, byte) in bytes[first..].iter().enumerate() {
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == delimiter {
                quote = None;
            }
            continue;
        }
        match *byte {
            b'"' | b'\'' => quote = Some(*byte),
            b'{' | b'[' | b'(' => nested += 1,
            b'}' | b']' | b')' => nested = nested.saturating_sub(1),
            b';' if nested == 0 => css_declaration_ended = true,
            b',' if nested == 0 => {
                if unquoted_member_name(&bytes[first + index + 1..]).is_some() {
                    return true;
                }
            }
            b'\n' | b'\r' if nested == 0 && !css_declaration_ended => {
                if unquoted_member_name(&bytes[first + index + 1..]).is_some() {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn looks_like_json_object(bytes: &[u8]) -> bool {
    let Some(first) = bytes
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    else {
        return true;
    };

    match bytes[first] {
        // A quoted JSON member, an empty object, and a nested candidate must
        // all be handled as one candidate.
        b'"' | b'}' | b'{' => true,
        b'\'' => starts_single_quoted_member(&bytes[first..]),
        _ => unquoted_member_name(&bytes[first..]).is_some_and(|name| {
            // An unquoted Review field is a malformed decision; unrelated
            // one-member prose such as `{verdict: reject}` remains skippable.
            name == b"approved" || has_multiple_unquoted_members(bytes, first)
        }),
    }
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    Usage {
        session_id: String,
        usage: crate::SessionUsage,
    },
    Message(Message),
    Delta {
        agent: String,
        text: String,
    },
    AgentStatus {
        agent: String,
        status: String,
    },
    Task(Task),
    Status(String),
    Finished {
        session_id: String,
        status: String,
    },
}

/// Providers may stream commentary before their final structured response. Accept
/// one complete final JSON object, but reject ambiguous objects, malformed outer
/// objects, and contradictory trailing prose. Schema validation still happens in T.
pub fn parse_response<T: serde::de::DeserializeOwned>(text: &str) -> Result<T> {
    let trimmed = text.trim();
    let payload = if trimmed.starts_with("```") && trimmed.ends_with("```") {
        trimmed
            .split_once('\n')
            .map(|(_, s)| s[..s.len() - 3].trim())
            .unwrap_or(trimmed)
    } else {
        trimmed
    };
    if let Ok(value) = serde_json::from_str(payload) {
        return Ok(value);
    }

    let bytes = trimmed.as_bytes();
    let mut cursor = 0;
    let mut object = None;
    while let Some(relative_start) = trimmed[cursor..].find('{') {
        let start = cursor + relative_start;
        let mut depth = 1usize;
        let mut quoted = false;
        let mut escaped = false;
        let mut end = None;
        for (relative_index, byte) in bytes[start + 1..].iter().enumerate() {
            let index = start + 1 + relative_index;
            if quoted {
                if escaped {
                    escaped = false;
                } else if *byte == b'\\' {
                    escaped = true;
                } else if *byte == b'"' {
                    quoted = false;
                }
                continue;
            }
            match *byte {
                b'"' => quoted = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(index + 1);
                        break;
                    }
                }
                _ => {}
            }
        }

        let json_looking = looks_like_json_object(
            end.map_or(&bytes[start + 1..], |end| &bytes[start + 1..end - 1]),
        );
        let Some(end) = end else {
            if json_looking {
                bail!("Agent returned an incomplete JSON object");
            }
            cursor = start + 1;
            continue;
        };
        if !json_looking {
            cursor = start + 1;
            continue;
        }

        let candidate = &trimmed[start..end];
        serde_json::from_str::<serde_json::Value>(candidate)
            .map_err(|e| anyhow::anyhow!("Agent returned malformed JSON: {e}"))?;
        if object.is_some() {
            bail!("Agent returned multiple JSON objects; the decision is ambiguous");
        }
        object = Some((candidate, end));
        cursor = end;
    }
    let (candidate, end) =
        object.ok_or_else(|| anyhow::anyhow!("Agent did not return a complete JSON object"))?;
    let suffix = trimmed[end..].trim();
    if !suffix.is_empty() && suffix != "```" {
        bail!(
            "Agent returned text after its JSON decision; a final unambiguous response is required"
        );
    }
    Ok(serde_json::from_str(candidate)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_review(approved: bool) -> String {
        format!(r#"{{"approved":{approved},"reason":"checked"}}"#)
    }

    fn assert_review_error(text: &str, expected: &str) {
        let error = parse_response::<Review>(text).unwrap_err().to_string();
        assert!(
            error.contains(expected),
            "expected {expected:?} in parse error, got {error:?}"
        );
    }

    #[test]
    fn parses_review_after_streamed_commentary() {
        let response = "The revised plan addresses my prior feedback. I verified the checks by inspection.\n\n{\"approved\":true,\"reason\":\"One self-contained index.html; checks cover required markup.\"}";
        let review: Review = parse_response(response).unwrap();
        assert!(review.approved);
        let fenced = "Inspection complete.\n```json\n{\"approved\":false,\"reason\":\"Missing <title>; braces {inside strings} are text.\"}\n```";
        let review: Review = parse_response(fenced).unwrap();
        assert!(!review.approved);
    }
    #[test]
    fn parses_final_review_after_css_commentary() {
        let response = r#"I inspected the browser output and verified the responsive stylesheet:
```css
body{overflow-x:hidden}
```
The implementation meets the acceptance criteria.
{"approved":true,"reason":"The browser checks pass.","lesson":"Keep the layout responsive."}"#;

        let review: Review = parse_response(response).unwrap();

        assert!(review.approved);
        assert_eq!(review.reason, "The browser checks pass.");
    }

    #[test]
    fn ignores_non_json_braces_in_quoted_commentary_and_code_fences() {
        let response = r#"The stylesheet note says "body { overflow-x: hidden; }".
```css
.card::before { content: "{quoted brace}"; }
```
{"approved":true,"reason":"checked"}"#;

        let review: Review = parse_response(response).unwrap();

        assert!(review.approved);

        let single_quoted_brace =
            ".card::before { content: '{'; }\n{\"approved\":true,\"reason\":\"checked\"}";
        let review: Review = parse_response(single_quoted_brace).unwrap();

        assert!(review.approved);

        let review: Review =
            parse_response("Checked a{color:red}.\n{\"approved\":true,\"reason\":\"checked\"}")
                .unwrap();

        assert!(review.approved);
    }

    #[test]
    fn parses_json_whitespace_after_a_non_json_brace_block() {
        let response =
            "Checked body{x:y}.\n{ \r\n\t\"approved\":true,\n \"reason\":\"exact ✓ { source }\" }";

        let review: Review = parse_response(response).unwrap();

        assert!(review.approved);
        assert_eq!(review.reason, "exact ✓ { source }");
    }

    #[test]
    fn preserves_direct_and_fenced_json_responses() {
        let direct: Review = parse_response(&valid_review(true)).unwrap();
        let fenced: Review =
            parse_response("```json\n{\"approved\":false,\"reason\":\"checked\"}\n```").unwrap();

        assert!(direct.approved);
        assert!(!fenced.approved);
    }

    #[test]
    fn rejects_two_valid_decisions_as_ambiguous() {
        let response = format!(
            "First: {}\nSecond: {}",
            valid_review(true),
            valid_review(false)
        );

        assert_review_error(&response, "multiple JSON objects");
    }

    #[test]
    fn rejects_nested_decision_inside_malformed_json_looking_object() {
        let response = r#"Result: {"wrapper":{"approved":true,"reason":"nested"},"broken":}"#;

        assert_review_error(response, "malformed JSON");
    }

    #[test]
    fn rejects_malformed_json_looking_object_before_valid_decision() {
        let response = r#"Draft: {"approved":tru,"reason":"draft"}
Final: {"approved":true,"reason":"checked"}"#;

        assert_review_error(response, "malformed JSON");
    }

    #[test]
    fn rejects_non_json_object_drafts_and_nested_fragments() {
        for (case, response, expected) in [
            (
                "c16_js_style_draft_opposite_before_final",
                "Draft: {approved: false, reason: \"draft\"}\nFinal: {\"approved\":true,\"reason\":\"checked\"}",
                "malformed JSON",
            ),
            (
                "c17_python_dict_draft_before_final",
                "Draft: {'approved': False, 'reason': 'draft'}\nFinal: {\"approved\":true,\"reason\":\"checked\"}",
                "malformed JSON",
            ),
            (
                "c18_unclosed_unquoted_outer_with_nested",
                "Result: {approved: true, details: {\"approved\":true,\"reason\":\"nested\"}",
                "incomplete JSON object",
            ),
            (
                "c19_malformed_inner_in_non_json_block",
                ".card { {\"approved\":tru} }\n{\"approved\":true,\"reason\":\"checked\"}",
                "malformed JSON",
            ),
            (
                "c32_yaml_like_draft_multiline",
                "Draft:\n{\n  approved: false\n  reason: draft\n}\nFinal: {\"approved\":true,\"reason\":\"checked\"}",
                "malformed JSON",
            ),
        ] {
            let error = parse_response::<Review>(response).unwrap_err().to_string();
            assert!(
                error.contains(expected),
                "{case}: expected {expected:?} in parse error, got {error:?}"
            );
        }

        assert_review_error(
            "Draft: {approved: false}\nFinal: {\"approved\":true,\"reason\":\"checked\"}",
            "malformed JSON",
        );
        let review: Review = parse_response(
            "Note: {verdict: reject}\nFinal: {\"approved\":true,\"reason\":\"checked\"}",
        )
        .unwrap();
        assert!(review.approved);
    }

    #[test]
    fn detects_decisions_nested_in_non_json_blocks() {
        for (case, response, expected) in [
            (
                "c34_valid_decision_inside_code_block",
                r#"Example:
function f() { return {"approved":false,"reason":"draft"}; }
Final: {"approved":true,"reason":"checked"}"#,
                "multiple JSON objects",
            ),
            (
                "c35_malformed_json_after_css_declaration",
                r#".card { color: red; {"approved":tru} }
{"approved":true,"reason":"checked"}"#,
                "malformed JSON",
            ),
            (
                "c44_valid_decision_after_css_declaration",
                r#".a { x: y; {"approved":false,"reason":"draft"} }
{"approved":true,"reason":"checked"}"#,
                "multiple JSON objects",
            ),
        ] {
            let error = parse_response::<Review>(response).unwrap_err().to_string();
            assert!(
                error.contains(expected),
                "{case}: expected {expected:?} in parse error, got {error:?}"
            );
        }
    }

    #[test]
    fn rejects_incomplete_json_looking_candidate() {
        let response = r#"Final: {"approved":true,"reason":"checked""#;

        assert_review_error(response, "incomplete JSON object");
    }

    #[test]
    fn rejects_prose_after_final_decision() {
        let response = r#"{"approved":true,"reason":"checked"}
Actually, reject this result."#;

        assert_review_error(response, "text after its JSON decision");
    }

    #[test]
    fn rejects_ambiguous_malformed_or_wrong_schema_responses() {
        for text in [
            "First: {\"approved\":true,\"reason\":\"pass\"} Then: {\"approved\":false,\"reason\":\"fail\"}",
            "Result: {\"nested\":{\"approved\":true,\"reason\":\"pass\"}",
            "Result: {\"approved\":true}",
            "Result: {\"approved\":true,\"reason\":\"pass\"} Actually, reject this result.",
            "No structured decision.",
        ] { assert!(parse_response::<Review>(text).is_err(), "accepted {text}"); }
    }
    #[test]
    fn rejects_cycles() {
        let mut plan = Plan {
            summary: String::new(),
            tasks: (0..2)
                .map(|i| PlanTask {
                    access: crate::TaskAccess::default(),
                    title: "x".into(),
                    description: "x".into(),
                    competence: implementation(),
                    difficulty: standard(),
                    dependencies: vec![1 - i],
                    checks: vec![],
                })
                .collect(),
        };
        assert!(plan.validate().is_err());
        plan.tasks[1].dependencies.clear();
        assert!(plan.validate().is_ok());
    }
    #[test]
    fn rejects_self_acceptance_and_double_assignment() {
        let mut task = Task {
            access: crate::TaskAccess::default(),
            id: new_id(),
            session_id: new_id(),
            title: "x".into(),
            description: "x".into(),
            competence: implementation(),
            difficulty: standard(),
            dependencies: vec![],
            checks: vec![],
            state: TaskState::Ready,
            assignee: None,
            reviewer: None,
            attempts: 0,
            result: None,
            workspace: None,
            base_commit: None,
            interrupted: false,
        };
        task.assign("a", &HashSet::new()).unwrap();
        assert!(task.assign("b", &HashSet::new()).is_err());
        task.submit("a", "done".into()).unwrap();
        assert!(task.review("a", true, 3).is_err());
        task.review("b", true, 3).unwrap();
        assert_eq!(task.state, TaskState::Accepted);
    }
}
