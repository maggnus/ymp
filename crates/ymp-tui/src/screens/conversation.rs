//! The conversation: attributed entries read from the projection, merged with the
//! runtime's local answers, wrapped to the width and positioned by entry so that
//! arriving activity does not move what the user is reading.
use super::pages::{UNKNOWN, cell, clock, outcome};
use crate::app::{Draft, Entry, Motion, Projection, Reading, Tone};
use serde_json::Value;
use unicode_width::UnicodeWidthChar;
use ymp_runtime::kernel::finalization::render_report;

/// Lines of reported output shown inline; the Activity page has all of it.
const EXCERPT: usize = 6;

/// Characters that change how following text is drawn instead of showing
/// themselves: controls and the directional marks, embeddings and isolates.
pub fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        )
}
/// Text from an agent or a file is data. It reaches the terminal as printable
/// characters only, so it cannot carry terminal control sequences or reorder
/// what surrounds it.
pub fn printable(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\n' => '\n',
            '\t' => '\t',
            c if hidden(c) => '\u{fffd}',
            c => c,
        })
        .collect()
}
/// The input as typed: lines break at newlines and at the width only, so the
/// returned line and column are where the next typed character appears.
pub fn compose(input: &str, cursor: usize, width: usize) -> (Vec<String>, (usize, usize)) {
    let width = width.max(1);
    let mut lines = vec![String::new()];
    let mut used = 0;
    let mut at = None;
    for (index, c) in input.char_indices() {
        if c == '\n' {
            if index == cursor {
                at = Some((lines.len() - 1, used));
            }
            lines.push(String::new());
            used = 0;
            continue;
        }
        let c = match c {
            '\t' => ' ',
            c if hidden(c) => '\u{fffd}',
            c => c,
        };
        let w = c.width().unwrap_or(0);
        if used + w > width {
            lines.push(String::new());
            used = 0;
        }
        if index == cursor {
            at = Some((lines.len() - 1, used));
        }
        if let Some(line) = lines.last_mut() {
            line.push(c);
        }
        used += w;
    }
    let at = at.unwrap_or((lines.len() - 1, used));
    (lines, at)
}
/// Wrap to `width` columns, keeping the indentation of each source line.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    let mut lines = vec![];
    for source in printable(text).replace('\t', "    ").split('\n') {
        let indent: String = source
            .chars()
            .take_while(|c| *c == ' ')
            .take(width / 2)
            .collect();
        let mut line = String::new();
        let mut used = 0;
        let mut word = String::new();
        let mut word_width = 0;
        let flush = |word: &mut String,
                     word_width: &mut usize,
                     line: &mut String,
                     used: &mut usize,
                     lines: &mut Vec<String>| {
            if *used + *word_width > width && !line.trim().is_empty() {
                lines.push(std::mem::take(line).trim_end().to_string());
                line.push_str(&indent);
                *used = indent.len();
                *word = word.trim_start().to_string();
                *word_width = word.chars().map(|c| c.width().unwrap_or(0)).sum();
            }
            for c in word.drain(..) {
                let w = c.width().unwrap_or(0);
                if *used + w > width {
                    lines.push(std::mem::take(line));
                    line.push_str(&indent);
                    *used = indent.len();
                }
                line.push(c);
                *used += w;
            }
            *word_width = 0;
        };
        for c in source.chars() {
            if c == ' ' && !word.trim().is_empty() {
                flush(&mut word, &mut word_width, &mut line, &mut used, &mut lines);
            }
            word.push(c);
            word_width += c.width().unwrap_or(0);
        }
        flush(&mut word, &mut word_width, &mut line, &mut used, &mut lines);
        lines.push(line.trim_end().to_string());
    }
    lines
}
fn entry(key: String, at: u64, order: u32, tone: Tone, heading: String) -> Entry {
    Entry {
        key,
        at,
        order,
        tone,
        heading,
        body: vec![],
        major: false,
    }
}
fn listed(value: &Value) -> Vec<(String, &Value)> {
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
/// Model and effort of a profile; an unrecorded effort is not shown as a value.
pub fn profile(settings: &Value) -> String {
    match settings["effort"].as_str() {
        Some(effort) => format!("{} · effort {effort}", cell(&settings["model"])),
        None => cell(&settings["model"]),
    }
}
pub fn usage_line(record: &Value, runs: bool) -> String {
    let receipt = &record["receipt"];
    if receipt.is_null() {
        return if runs {
            format!("usage {UNKNOWN} (still working)")
        } else if record["terminal"].is_null() && !record["backend_terminal"].is_null() {
            format!("usage {UNKNOWN} (the provider ended; no receipt recorded; usage unknown)")
        } else if record["terminal"].is_null() {
            format!("usage {UNKNOWN} (no end and no receipt recorded; usage unknown)")
        } else {
            format!("usage {UNKNOWN} (no receipt recorded; usage unknown)")
        };
    }
    let usage = &receipt["usage"];
    let mut line = format!(
        "usage: input {} · output {}",
        cell(&usage["input"]),
        cell(&usage["output"])
    );
    if !usage["reasoning"].is_null() {
        line.push_str(&format!(" (reasoning {})", cell(&usage["reasoning"])));
    }
    line.push_str(&format!(" · coverage {}", cell(&receipt["coverage"])));
    if receipt["coverage"] != "Complete" {
        line.push_str(" — usage is not fully known");
    }
    line
}
fn draft(draft: &Draft, entries: &mut Vec<Entry>) {
    if !draft.goal.is_empty() {
        let mut goal = entry(
            "draft-goal".into(),
            0,
            0,
            Tone::User,
            "You · goal (not started)".into(),
        );
        goal.body = vec![draft.goal.clone()];
        goal.major = true;
        entries.push(goal);
    }
    if !draft.expectations.is_empty() {
        let mut stated = entry(
            "draft-expectations".into(),
            0,
            1,
            Tone::User,
            format!("You · expectations (not started) · budget {}", draft.budget),
        );
        stated.body = draft
            .expectations
            .iter()
            .map(|e| {
                if e.preserve {
                    format!("{} keeps its current {} bytes", e.path, e.bytes.len())
                } else {
                    format!(
                        "{} contains exactly {:?}",
                        e.path,
                        String::from_utf8_lossy(&e.bytes)
                    )
                }
            })
            .collect();
        entries.push(stated);
    }
}
/// Every entry the projection supports, oldest first.
pub fn entries(projection: Option<&Projection>, stated: &Draft, notices: &[Entry]) -> Vec<Entry> {
    let mut entries = vec![];
    draft(stated, &mut entries);
    if let Some(projection) = projection {
        recorded(projection, &mut entries);
    }
    entries.extend(notices.iter().cloned());
    entries.sort_by(|a, b| (a.at, a.order, &a.key).cmp(&(b.at, b.order, &b.key)));
    entries
}
fn recorded(projection: &Projection, entries: &mut Vec<Entry>) {
    let json = &projection.json;
    let latest = projection.view.latest_at();
    let goal = &json["task"]["goal"];
    if let Some(request) = goal["request"].as_str() {
        let mut stated = entry("goal".into(), 0, 0, Tone::User, "You · goal".into());
        stated.body = vec![request.into()];
        stated.major = true;
        entries.push(stated);
    }
    let criteria = listed(&json["criteria"]);
    if !criteria.is_empty() {
        let mut all = entry(
            "criteria".into(),
            0,
            1,
            Tone::Runtime,
            "Criteria · current recorded status".into(),
        );
        all.body = criteria
            .iter()
            .map(|(_, criterion)| {
                let status = &json["ledger"]["entries"][cell(&criterion["id"])]["status"];
                format!(
                    "[{}] {} — {}",
                    if status.is_null() {
                        "not assessed".into()
                    } else {
                        cell(status)
                    },
                    cell(&criterion["id"]),
                    cell(&criterion["text"])
                )
            })
            .collect();
        entries.push(all);
    }
    for (index, answer) in listed(&goal["clarifications"]) {
        let mut said = entry(
            format!("clarification:{index}"),
            answer["at"].as_u64().unwrap_or(latest),
            10,
            Tone::User,
            "You · answer".into(),
        );
        said.body = vec![
            format!("Q: {}", cell(&answer["question"])),
            format!("A: {}", cell(&answer["answer"])),
        ];
        said.major = true;
        entries.push(said);
    }
    for (index, assumption) in listed(&goal["assumptions"]) {
        let mut noted = entry(
            format!("assumption:{index}"),
            0,
            2,
            Tone::Runtime,
            "Assumption recorded".into(),
        );
        noted.body = vec![
            cell(&assumption["text"]),
            format!("reason: {}", cell(&assumption["reason"])),
        ];
        entries.push(noted);
    }
    for (name, record) in listed(&json["execution"]["invocations"]) {
        let assignment = &record["dispatch"]["assignment"];
        let runs = projection.runs(record);
        let ended = outcome(record, runs);
        let mut call = entry(
            format!("call:{name}"),
            record["dispatch"]["at"].as_u64().unwrap_or(latest),
            20,
            if record["terminal"].is_null() || record["terminal"] == "Completed" {
                Tone::Agent
            } else {
                Tone::Bad
            },
            format!(
                "{} · {} · {} · {}",
                cell(&assignment["agent"]),
                cell(&assignment["role"]),
                profile(&record["dispatch"]["settings"]),
                ended
            ),
        );
        call.body.push(usage_line(record, runs));
        if record["terminal"].is_null() && !record["backend_terminal"].is_null() {
            call.body.push(
                if projection.driving {
                    "the runtime keeps the hold until usage and effects settle or the call deadline passes"
                } else {
                    "usage and effects were not settled; the hold stays as recorded"
                }
                .into(),
            );
        }
        if let Some(output) = record["output"].as_str() {
            let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
            for line in lines.iter().take(EXCERPT) {
                call.body.push(format!("reported: {line}"));
            }
            if lines.len() > EXCERPT {
                call.body.push(format!(
                    "… {} more lines; /activity shows the full reported output",
                    lines.len() - EXCERPT
                ));
            }
        }
        for diagnostic in listed(&record["diagnostics"]) {
            call.body.push(format!(
                "diagnostic {}: {}",
                cell(&diagnostic.1[1]),
                cell(&diagnostic.1[2])
            ));
        }
        entries.push(call);
    }
    for (name, run) in listed(&json["check_runs"]) {
        let check = &json["checks"][run["check"].as_str().unwrap_or_default()];
        entries.push(entry(
            format!("run:{name}"),
            run["at"].as_u64().unwrap_or(latest),
            30,
            if run["outcome"] == "Pass" {
                Tone::Good
            } else {
                Tone::Bad
            },
            format!(
                "Check {} for criterion {} on {} {}: {}",
                cell(&run["check"]),
                cell(&check["criterion"]),
                cell(&run["role"]),
                cell(&run["target"]),
                cell(&run["outcome"])
            ),
        ));
    }
    for (name, record) in listed(&json["reviews"]) {
        let review = &record["review"];
        let mut said = entry(
            format!("review:{name}"),
            record["at"].as_u64().unwrap_or(latest),
            40,
            if review["verdict"] == "Approve" {
                Tone::Agent
            } else {
                Tone::Bad
            },
            format!(
                "{} · review of {} · {}: {}",
                cell(&review["reviewer"]),
                cell(&review["result"]),
                profile(&review["profile"]),
                cell(&review["verdict"])
            ),
        );
        said.body = listed(&review["findings"])
            .iter()
            .map(|(_, finding)| format!("finding: {}", cell(finding)))
            .collect();
        entries.push(said);
    }
    for (name, record) in listed(&json["acceptances"]) {
        let acceptance = &record["acceptance"];
        entries.push(entry(
            format!("acceptance:{name}"),
            acceptance["at"].as_u64().unwrap_or(latest),
            50,
            if acceptance["decision"] == "Accepted" {
                Tone::Good
            } else {
                Tone::Bad
            },
            format!(
                "Runtime · acceptance of {}: {} · grade {}",
                cell(&acceptance["subject"]),
                cell(&acceptance["decision"]),
                cell(&acceptance["grade"])
            ),
        ));
    }
    for (index, question) in projection.pending_questions().into_iter().enumerate() {
        let mut asked = entry(
            format!("question:{index}"),
            latest,
            60 + index as u32,
            Tone::Runtime,
            if index == 0 {
                "Question · waiting for your answer".into()
            } else {
                "Question · asked after the one above".into()
            },
        );
        asked.body = vec![
            cell(&question["question"]),
            format!("reason: {}", cell(&question["reason"])),
            format!(
                "without an answer the assumption would be: {}",
                cell(&question["assumption"])
            ),
        ];
        asked.major = true;
        entries.push(asked);
    }
    let state = &json["session_state"];
    if state["stopped"] == true || json["finalization"]["stopped"] == true {
        entries.push(entry(
            "stopped".into(),
            latest,
            70,
            Tone::Runtime,
            "Runtime · stop recorded; no further model call is started".into(),
        ));
    }
    if let Some(code) = state["phase"]["Blocked"].as_str() {
        entries.push(entry(
            "blocked".into(),
            latest,
            71,
            Tone::Bad,
            format!("Runtime · session blocked: {code}"),
        ));
    }
    if let Some(delivered) = &projection.view.finalization().delivered {
        let recorded = &json["finalization"]["delivered"];
        let mut report = entry(
            "report".into(),
            latest,
            80,
            Tone::Report,
            format!(
                "Report · outcome {} · grade {}",
                cell(&recorded["outcome"]),
                cell(&recorded["report"]["grade"])
            ),
        );
        report.body = render_report(delivered).lines().map(String::from).collect();
        let accounting = &recorded["accounting"];
        report.body.push(format!(
            "accounting: spent {} · held {} · unknown usage {}",
            cell(&accounting["spent"]),
            cell(&accounting["held"]),
            cell(&accounting["unknown"])
        ));
        report.major = true;
        entries.push(report);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Heading,
    Body,
    Gap,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub entry: usize,
    /// Position inside the entry; the reading position refers to it.
    pub line: usize,
    pub kind: RowKind,
    pub text: String,
}
pub fn rows(entries: &[Entry], width: usize) -> Vec<Row> {
    let mut rows = vec![];
    for (index, entry) in entries.iter().enumerate() {
        let mut line = 0;
        let mut push = |kind, text: String, rows: &mut Vec<Row>| {
            rows.push(Row {
                entry: index,
                line,
                kind,
                text,
            });
            line += 1;
        };
        let stamp = if entry.at == 0 {
            String::new()
        } else {
            format!("{} ", clock(&Value::from(entry.at)))
        };
        for text in wrap(&format!("{stamp}{}", entry.heading), width) {
            push(RowKind::Heading, text, &mut rows);
        }
        for text in entry
            .body
            .iter()
            .flat_map(|text| wrap(text, width.saturating_sub(2)))
        {
            push(RowKind::Body, format!("  {text}"), &mut rows);
        }
        if entry.major {
            push(RowKind::Gap, String::new(), &mut rows);
        }
    }
    rows
}
/// The first visible row. Without a reading position the view follows the end.
/// Motions move the position; reaching the end resumes following.
pub fn position(
    entries: &[Entry],
    rows: &[Row],
    height: usize,
    reading: &mut Option<Reading>,
    motions: &mut Vec<Motion>,
) -> usize {
    let end = rows.len().saturating_sub(height);
    let mut top = match reading {
        None => end,
        Some(at) => rows
            .iter()
            .position(|row| entries[row.entry].key == at.key && row.line >= at.line)
            // The entry is gone: stay at the same distance from the beginning.
            .unwrap_or(at.index)
            .min(end),
    };
    let mut following = reading.is_none();
    for motion in motions.drain(..) {
        let page = height.saturating_sub(1).max(1) as isize;
        let target = match motion {
            Motion::Lines(n) => top as isize + n,
            Motion::Pages(n) => top as isize + n * page,
            Motion::Start => 0,
            Motion::End => end as isize,
        };
        top = target.clamp(0, end as isize) as usize;
        following = top >= end;
    }
    *reading = if following {
        None
    } else {
        rows.get(top).map(|row| Reading {
            key: entries[row.entry].key.clone(),
            line: row.line,
            index: top,
        })
    };
    top
}
#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    fn said(key: &str, lines: usize) -> Entry {
        let mut entry = entry(key.into(), 1, 0, Tone::Agent, key.into());
        entry.body = (0..lines).map(|n| format!("{key} line {n}")).collect();
        entry
    }
    #[test]
    fn wrapping_keeps_every_line_inside_the_width() {
        // Spaces wider or longer in bytes than in columns once broke the count.
        let text =
            "alpha\u{a0}\u{a0}beta \u{3000}\u{3000}gamma delta\u{a0}epsilon zeta 広い文字の列 eta";
        for width in 8..40 {
            for line in wrap(text, width) {
                assert!(line.width() <= width, "{line:?} exceeds {width}");
            }
        }
    }
    #[test]
    fn marks_that_redraw_text_are_replaced() {
        let shown = printable("a\u{1b}[2Jb\u{202e}c\u{2066}d\re");
        assert_eq!(shown, "a\u{fffd}[2Jb\u{fffd}c\u{fffd}d\u{fffd}e");
    }
    #[test]
    fn the_cursor_is_where_the_next_character_appears() {
        assert_eq!(compose("ab ", 3, 10).1, (0, 3));
        assert_eq!(
            compose("abcdef", 6, 3),
            (vec!["abc".into(), "def".into()], (1, 3))
        );
        assert_eq!(compose("abcdef", 3, 3).1, (1, 0));
        assert_eq!(compose("ab\ncd", 2, 10).1, (0, 2));
        assert_eq!(compose("ab\ncd", 3, 10).1, (1, 0));
        assert_eq!(compose("広い", 3, 3).1, (1, 0));
    }
    #[test]
    fn arriving_activity_does_not_move_what_is_read() {
        let mut entries = vec![said("first", 6), said("second", 6), said("third", 6)];
        let mut reading = None;
        let mut motions = vec![Motion::Lines(-9)];
        let all = rows(&entries, 40);
        let top = position(&entries, &all, 5, &mut reading, &mut motions);
        let read = all[top].clone();
        assert!(reading.is_some());
        // More activity arrives, some of it sorted before what is being read.
        entries.insert(0, said("earlier", 4));
        entries.push(said("fourth", 8));
        let all = rows(&entries, 40);
        let top = position(&entries, &all, 5, &mut reading, &mut motions);
        assert_eq!(all[top].text, read.text);
        // End follows the newest activity again.
        motions.push(Motion::End);
        let top = position(&entries, &all, 5, &mut reading, &mut motions);
        assert_eq!(top, all.len() - 5);
        assert!(reading.is_none());
    }
}
