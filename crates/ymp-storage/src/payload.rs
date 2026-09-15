//! Strict payload codec for the durable journal's version-1 record payloads.
//!
//! The two payload forms are fixed by the durable Journal contract:
//!
//! ```json
//! {"type":"session_opened","session_id":S,"task":{"id":T,"goal":{"request":G},"acceptance_contract":{"criteria":[{"id":C,"description":D}]},"constraints":{"conditions":[K]}}}
//! ```
//!
//! ```json
//! {"type":"session_cancelled","session_id":S}
//! ```
//!
//! Every field is required; unknown fields, duplicate fields and an unknown
//! `type` are rejected. Key order is insignificant on decode; the encoder
//! produces one byte-stable form. Strings are restored exactly, with no
//! normalization.

use ymp_domain::{
    AcceptanceContract, Constraints, Criterion, CriterionId, DomainError, Goal, SessionId, Task,
    TaskId,
};
use ymp_kernel::{JournalError, SessionEvent};

/// A single payload string is at most 2^20 UTF-8 bytes.
pub(super) const MAX_STRING_BYTES: usize = 1 << 20;
/// A record payload is at most 2^20 bytes.
pub(super) const MAX_PAYLOAD_BYTES: usize = 1 << 20;

/// Fixed structural bytes of the largest payload form, used only to bound the
/// pre-allocation size check.
const STRUCTURAL_BOUND: u64 = 512;

fn input_limit(message: String) -> JournalError {
    JournalError::AdapterFailure { message }
}

fn string_bound(value: &str) -> u64 {
    // Every escaped output byte costs at most six bytes (`\u00xx`).
    value.len() as u64 * 6 + 2
}

fn require_string_limit(value: &str) -> Result<(), JournalError> {
    if value.len() > MAX_STRING_BYTES {
        return Err(input_limit(format!(
            "payload string exceeds the {MAX_STRING_BYTES}-byte limit"
        )));
    }
    Ok(())
}

fn task_strings_bound(task: &Task) -> u64 {
    let mut bound = string_bound(task.id().as_str()) + string_bound(task.goal().request());
    for criterion in task.acceptance_contract().criteria() {
        bound += string_bound(criterion.id().as_str());
        bound += string_bound(criterion.description());
    }
    for condition in task.constraints().conditions() {
        bound += string_bound(condition);
    }
    bound
}

fn event_bound(event: &SessionEvent) -> u64 {
    match event {
        SessionEvent::SessionOpened { session_id, task } => {
            string_bound(session_id.as_str()) + task_strings_bound(task) + STRUCTURAL_BOUND
        }
        SessionEvent::SessionCancelled { session_id } => {
            string_bound(session_id.as_str()) + STRUCTURAL_BOUND
        }
    }
}

/// Encodes one event into its exact version-1 payload form.
///
/// Input limits are validated before the payload buffer is built; a violation
/// is an input-limit failure (`AdapterFailure`) raised before any storage
/// access by the caller.
pub(super) fn encode_event(event: &SessionEvent) -> Result<Vec<u8>, JournalError> {
    if event_bound(event) > MAX_PAYLOAD_BYTES as u64 {
        return Err(input_limit(format!(
            "encoded record payload exceeds the {MAX_PAYLOAD_BYTES}-byte limit"
        )));
    }
    let mut out = Vec::with_capacity(256);
    match event {
        SessionEvent::SessionOpened { session_id, task } => {
            require_string_limit(session_id.as_str())?;
            require_string_limit(task.id().as_str())?;
            require_string_limit(task.goal().request())?;
            for criterion in task.acceptance_contract().criteria() {
                require_string_limit(criterion.id().as_str())?;
                require_string_limit(criterion.description())?;
            }
            for condition in task.constraints().conditions() {
                require_string_limit(condition)?;
            }
            out.extend_from_slice(br#"{"type":"session_opened","session_id":"#);
            write_string(&mut out, session_id.as_str());
            out.extend_from_slice(br#","task":{"id":"#);
            write_string(&mut out, task.id().as_str());
            out.extend_from_slice(br#","goal":{"request":"#);
            write_string(&mut out, task.goal().request());
            out.extend_from_slice(br#"},"acceptance_contract":{"criteria":["#);
            for (index, criterion) in task.acceptance_contract().criteria().iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                out.extend_from_slice(br#"{"id":"#);
                write_string(&mut out, criterion.id().as_str());
                out.extend_from_slice(br#","description":"#);
                write_string(&mut out, criterion.description());
                out.push(b'}');
            }
            out.extend_from_slice(br#"]},"constraints":{"conditions":["#);
            for (index, condition) in task.constraints().conditions().iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_string(&mut out, condition);
            }
            out.extend_from_slice(br#"]}}}"#);
        }
        SessionEvent::SessionCancelled { session_id } => {
            require_string_limit(session_id.as_str())?;
            out.extend_from_slice(br#"{"type":"session_cancelled","session_id":"#);
            write_string(&mut out, session_id.as_str());
            out.extend_from_slice(b"}");
        }
    }
    if out.len() > MAX_PAYLOAD_BYTES {
        return Err(input_limit(
            "encoded record payload exceeds the limit".to_owned(),
        ));
    }
    Ok(out)
}

/// Writes a JSON string with a fixed escaping: the two mandatory escapes, the
/// five short control escapes, `\u00xx` (lowercase hex) for the remaining
/// control characters, and raw UTF-8 for everything else.
fn write_string(out: &mut Vec<u8>, value: &str) {
    out.push(b'"');
    for &byte in value.as_bytes() {
        match byte {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            0x08 => out.extend_from_slice(b"\\b"),
            0x0C => out.extend_from_slice(b"\\f"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            0x00..=0x1F => out.extend_from_slice(format!("\\u{byte:04x}").as_bytes()),
            _ => out.push(byte),
        }
    }
    out.push(b'"');
}

/// Decodes one version-1 payload. Every violation of the fixed forms is
/// returned as a human-readable reason the caller maps to `Corruption`.
pub(super) fn decode_payload(bytes: &[u8]) -> Result<SessionEvent, String> {
    let mut parser = Parser { bytes, position: 0 };
    parser.skip_ws();
    let root = parser.parse_root()?;
    parser.skip_ws();
    if parser.position != bytes.len() {
        return Err("payload has trailing bytes after the JSON object".to_owned());
    }
    finish_root(root)
}

#[derive(Default)]
struct RootFields {
    kind: Option<String>,
    session_id: Option<String>,
    task: Option<TaskFields>,
}

#[derive(Default)]
struct TaskFields {
    id: Option<String>,
    goal: Option<GoalFields>,
    acceptance_contract: Option<AcceptanceFields>,
    constraints: Option<ConstraintsFields>,
}

#[derive(Default)]
struct GoalFields {
    request: Option<String>,
}

#[derive(Default)]
struct AcceptanceFields {
    criteria: Option<Vec<CriterionFields>>,
}

#[derive(Default)]
struct CriterionFields {
    id: Option<String>,
    description: Option<String>,
}

#[derive(Default)]
struct ConstraintsFields {
    conditions: Option<Vec<String>>,
}

struct Parser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        while let Some(&byte) = self.bytes.get(self.position) {
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
                self.position += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn expect(&mut self, expected: u8) -> Result<(), String> {
        match self.peek() {
            Some(byte) if byte == expected => {
                self.position += 1;
                Ok(())
            }
            other => Err(format!(
                "expected '{}' but found {other:?} at byte {}",
                expected as char, self.position
            )),
        }
    }

    /// Parses one JSON string. Raw segments must be valid UTF-8; unescaped
    /// control characters and invalid escape sequences are rejected.
    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        let mut segment_start = self.position;
        loop {
            let byte = self
                .peek()
                .ok_or_else(|| "unterminated JSON string".to_owned())?;
            match byte {
                b'"' => {
                    self.append_segment(&mut out, segment_start)?;
                    if out.len() > MAX_STRING_BYTES {
                        return Err(format!(
                            "payload string exceeds the {MAX_STRING_BYTES}-byte limit"
                        ));
                    }
                    self.position += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.append_segment(&mut out, segment_start)?;
                    self.position += 1;
                    self.parse_escape(&mut out)?;
                    segment_start = self.position;
                }
                0x00..=0x1F => {
                    return Err("unescaped control character inside a JSON string".to_owned());
                }
                _ => {
                    self.position += 1;
                }
            }
        }
    }

    fn append_segment(&mut self, out: &mut String, start: usize) -> Result<(), String> {
        let segment = self
            .bytes
            .get(start..self.position)
            .ok_or_else(|| "JSON string segment out of bounds".to_owned())?;
        let text = std::str::from_utf8(segment)
            .map_err(|_| "JSON string is not valid UTF-8".to_owned())?;
        out.push_str(text);
        Ok(())
    }

    fn parse_escape(&mut self, out: &mut String) -> Result<(), String> {
        let escape = self
            .peek()
            .ok_or_else(|| "unterminated JSON escape".to_owned())?;
        self.position += 1;
        match escape {
            b'"' => out.push('"'),
            b'\\' => out.push('\\'),
            b'/' => out.push('/'),
            b'b' => out.push('\u{0008}'),
            b'f' => out.push('\u{000C}'),
            b'n' => out.push('\n'),
            b'r' => out.push('\r'),
            b't' => out.push('\t'),
            b'u' => {
                let first = self.parse_hex4()?;
                let code_point = if (0xD800..=0xDBFF).contains(&first) {
                    if self.peek() != Some(b'\\') {
                        return Err("unterminated UTF-16 surrogate pair".to_owned());
                    }
                    self.position += 1;
                    if self.peek() != Some(b'u') {
                        return Err("invalid UTF-16 surrogate pair".to_owned());
                    }
                    self.position += 1;
                    let second = self.parse_hex4()?;
                    if !(0xDC00..=0xDFFF).contains(&second) {
                        return Err("invalid low surrogate in a UTF-16 pair".to_owned());
                    }
                    0x10000u32 + ((first as u32 - 0xD800) << 10) + (second as u32 - 0xDC00)
                } else if (0xDC00..=0xDFFF).contains(&first) {
                    return Err("lone low surrogate in a JSON string".to_owned());
                } else {
                    first as u32
                };
                let character = char::from_u32(code_point)
                    .ok_or_else(|| "invalid escaped code point".to_owned())?;
                out.push(character);
            }
            other => return Err(format!("invalid JSON escape character '{other}'")),
        }
        Ok(())
    }

    fn parse_hex4(&mut self) -> Result<u16, String> {
        let mut value: u16 = 0;
        for _ in 0..4 {
            let digit = self
                .peek()
                .ok_or_else(|| "truncated \\u escape".to_owned())?;
            let nibble = (digit as char)
                .to_digit(16)
                .ok_or_else(|| "invalid hex digit in a \\u escape".to_owned())?
                as u16;
            value = value * 16 + nibble;
            self.position += 1;
        }
        Ok(value)
    }

    /// Parses an object's members, rejecting trailing separators. The caller
    /// supplies the per-key value parser; duplicate detection uses the parsed
    /// fields themselves.
    fn parse_object<T, F>(&mut self, mut parse_member: F) -> Result<T, String>
    where
        T: Default,
        F: FnMut(&mut Self, &str, &mut T) -> Result<(), String>,
    {
        let mut fields = T::default();
        self.expect(b'{')?;
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.position += 1;
            return Ok(fields);
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            self.skip_ws();
            parse_member(self, &key, &mut fields)?;
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                    self.skip_ws();
                    if self.peek() == Some(b'}') {
                        return Err("trailing comma inside a JSON object".to_owned());
                    }
                }
                Some(b'}') => {
                    self.position += 1;
                    return Ok(fields);
                }
                other => {
                    return Err(format!(
                        "expected ',' or '}}' but found {other:?} at byte {}",
                        self.position
                    ));
                }
            }
        }
    }

    /// Parses an array of strings.
    fn parse_string_array(&mut self) -> Result<Vec<String>, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.position += 1;
            return Ok(items);
        }
        loop {
            self.skip_ws();
            items.push(self.parse_string()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                    self.skip_ws();
                    if self.peek() == Some(b']') {
                        return Err("trailing comma inside a JSON array".to_owned());
                    }
                }
                Some(b']') => {
                    self.position += 1;
                    return Ok(items);
                }
                other => {
                    return Err(format!(
                        "expected ',' or ']' but found {other:?} at byte {}",
                        self.position
                    ));
                }
            }
        }
    }

    fn parse_root(&mut self) -> Result<RootFields, String> {
        self.parse_object(|parser, key, fields: &mut RootFields| {
            match key {
                "type" => {
                    if fields.kind.is_some() {
                        return Err("duplicate field 'type'".to_owned());
                    }
                    fields.kind = Some(parser.parse_string()?);
                }
                "session_id" => {
                    if fields.session_id.is_some() {
                        return Err("duplicate field 'session_id'".to_owned());
                    }
                    fields.session_id = Some(parser.parse_string()?);
                }
                "task" => {
                    if fields.task.is_some() {
                        return Err("duplicate field 'task'".to_owned());
                    }
                    fields.task = Some(parser.parse_task()?);
                }
                other => return Err(format!("unknown field '{other}'")),
            }
            Ok(())
        })
    }

    fn parse_task(&mut self) -> Result<TaskFields, String> {
        self.parse_object(|parser, key, fields: &mut TaskFields| {
            match key {
                "id" => {
                    if fields.id.is_some() {
                        return Err("duplicate field 'id'".to_owned());
                    }
                    fields.id = Some(parser.parse_string()?);
                }
                "goal" => {
                    if fields.goal.is_some() {
                        return Err("duplicate field 'goal'".to_owned());
                    }
                    let goal = parser.parse_object(|parser, key, fields: &mut GoalFields| {
                        match key {
                            "request" => {
                                if fields.request.is_some() {
                                    return Err("duplicate field 'request'".to_owned());
                                }
                                fields.request = Some(parser.parse_string()?);
                            }
                            other => return Err(format!("unknown field '{other}'")),
                        }
                        Ok(())
                    })?;
                    fields.goal = Some(goal);
                }
                "acceptance_contract" => {
                    if fields.acceptance_contract.is_some() {
                        return Err("duplicate field 'acceptance_contract'".to_owned());
                    }
                    let acceptance =
                        parser.parse_object(|parser, key, fields: &mut AcceptanceFields| {
                            match key {
                                "criteria" => {
                                    if fields.criteria.is_some() {
                                        return Err("duplicate field 'criteria'".to_owned());
                                    }
                                    fields.criteria = Some(parser.parse_criteria()?);
                                }
                                other => return Err(format!("unknown field '{other}'")),
                            }
                            Ok(())
                        })?;
                    fields.acceptance_contract = Some(acceptance);
                }
                "constraints" => {
                    if fields.constraints.is_some() {
                        return Err("duplicate field 'constraints'".to_owned());
                    }
                    let constraints =
                        parser.parse_object(|parser, key, fields: &mut ConstraintsFields| {
                            match key {
                                "conditions" => {
                                    if fields.conditions.is_some() {
                                        return Err("duplicate field 'conditions'".to_owned());
                                    }
                                    fields.conditions = Some(parser.parse_string_array()?);
                                }
                                other => return Err(format!("unknown field '{other}'")),
                            }
                            Ok(())
                        })?;
                    fields.constraints = Some(constraints);
                }
                other => return Err(format!("unknown field '{other}'")),
            }
            Ok(())
        })
    }

    fn parse_criteria(&mut self) -> Result<Vec<CriterionFields>, String> {
        self.expect(b'[')?;
        let mut criteria = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.position += 1;
            return Ok(criteria);
        }
        loop {
            self.skip_ws();
            let criterion = self.parse_object(|parser, key, fields: &mut CriterionFields| {
                match key {
                    "id" => {
                        if fields.id.is_some() {
                            return Err("duplicate field 'id'".to_owned());
                        }
                        fields.id = Some(parser.parse_string()?);
                    }
                    "description" => {
                        if fields.description.is_some() {
                            return Err("duplicate field 'description'".to_owned());
                        }
                        fields.description = Some(parser.parse_string()?);
                    }
                    other => return Err(format!("unknown field '{other}'")),
                }
                Ok(())
            })?;
            criteria.push(criterion);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                    self.skip_ws();
                    if self.peek() == Some(b']') {
                        return Err("trailing comma inside a JSON array".to_owned());
                    }
                }
                Some(b']') => {
                    self.position += 1;
                    return Ok(criteria);
                }
                other => {
                    return Err(format!(
                        "expected ',' or ']' but found {other:?} at byte {}",
                        self.position
                    ));
                }
            }
        }
    }
}

fn finish_root(root: RootFields) -> Result<SessionEvent, String> {
    let kind = root
        .kind
        .ok_or_else(|| "missing required field 'type'".to_owned())?;
    let session_id = root
        .session_id
        .ok_or_else(|| "missing required field 'session_id'".to_owned())?;
    let session_id =
        SessionId::new(session_id).map_err(|error| domain_error("session_id", error))?;
    match kind.as_str() {
        "session_opened" => {
            let task = root
                .task
                .ok_or_else(|| "missing required field 'task'".to_owned())?;
            let event = SessionEvent::SessionOpened {
                session_id,
                task: build_task(task)?,
            };
            Ok(event)
        }
        "session_cancelled" => {
            if root.task.is_some() {
                return Err("unknown field 'task' for type 'session_cancelled'".to_owned());
            }
            Ok(SessionEvent::SessionCancelled { session_id })
        }
        other => Err(format!("unknown payload type '{other}'")),
    }
}

fn build_task(fields: TaskFields) -> Result<Task, String> {
    let id = fields
        .id
        .ok_or_else(|| "missing required field 'id'".to_owned())?;
    let goal = fields
        .goal
        .ok_or_else(|| "missing required field 'goal'".to_owned())?;
    let acceptance = fields
        .acceptance_contract
        .ok_or_else(|| "missing required field 'acceptance_contract'".to_owned())?;
    let constraints = fields
        .constraints
        .ok_or_else(|| "missing required field 'constraints'".to_owned())?;
    let request = goal
        .request
        .ok_or_else(|| "missing required field 'request'".to_owned())?;
    let criteria_fields = acceptance
        .criteria
        .ok_or_else(|| "missing required field 'criteria'".to_owned())?;
    let conditions = constraints
        .conditions
        .ok_or_else(|| "missing required field 'conditions'".to_owned())?;

    let mut criteria = Vec::with_capacity(criteria_fields.len());
    for criterion in criteria_fields {
        let id = criterion
            .id
            .ok_or_else(|| "missing required field 'id'".to_owned())?;
        let description = criterion
            .description
            .ok_or_else(|| "missing required field 'description'".to_owned())?;
        let criterion = Criterion::new(
            CriterionId::new(id).map_err(|error| domain_error("id", error))?,
            description,
        )
        .map_err(|error| domain_error("description", error))?;
        criteria.push(criterion);
    }

    let task = Task::new(
        TaskId::new(id).map_err(|error| domain_error("id", error))?,
        Goal::new(request).map_err(|error| domain_error("request", error))?,
        AcceptanceContract::new(criteria).map_err(|error| domain_error("criteria", error))?,
        Constraints::new(conditions).map_err(|error| domain_error("conditions", error))?,
    );
    Ok(task)
}

fn domain_error(field: &str, error: DomainError) -> String {
    format!("payload field '{field}' is not a valid domain value: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened_event() -> SessionEvent {
        let task = Task::new(
            TaskId::new("task-1").expect("valid task ID"),
            Goal::new("Produce a \"quoted\" goal\\path\nwith lines\tand émoji 🚀")
                .expect("valid goal"),
            AcceptanceContract::new(vec![
                Criterion::new(
                    CriterionId::new("c1").expect("valid criterion ID"),
                    "Matches exactly\u{0001}\u{007F}.",
                )
                .expect("valid criterion"),
                Criterion::new(
                    CriterionId::new("c2").expect("valid criterion ID"),
                    "Second criterion.",
                )
                .expect("valid criterion"),
            ])
            .expect("valid acceptance contract"),
            Constraints::new(vec!["no network".to_owned(), String::new() + "\u{0}"])
                .expect("valid constraints"),
        );
        SessionEvent::SessionOpened {
            session_id: SessionId::new("session \"one\"/\\✔").expect("valid session ID"),
            task,
        }
    }

    #[test]
    fn encode_decode_round_trips_exactly() {
        for event in [
            opened_event(),
            SessionEvent::SessionCancelled {
                session_id: SessionId::new("cancel-me").expect("valid session ID"),
            },
        ] {
            let payload = encode_event(&event).expect("payload encodes");
            assert_eq!(decode_payload(&payload).expect("payload decodes"), event);
        }
    }

    #[test]
    fn encode_is_byte_stable() {
        let first = encode_event(&opened_event()).expect("payload encodes");
        let second = encode_event(&opened_event()).expect("payload encodes");
        assert_eq!(first, second);
    }

    #[test]
    fn decode_ignores_key_order_and_whitespace() {
        let payload = br#" {
            "task" : { "constraints" : { "conditions" : [ ] } ,
            "acceptance_contract" : { "criteria" : [ { "description" : "d" , "id" : "c1" } ] } ,
            "goal" : { "request" : "g" } , "id" : "t1" } ,
            "session_id" : "s1" , "type" : "session_opened"
        } "#;
        let expected = SessionEvent::SessionOpened {
            session_id: SessionId::new("s1").expect("valid session ID"),
            task: Task::new(
                TaskId::new("t1").expect("valid task ID"),
                Goal::new("g").expect("valid goal"),
                AcceptanceContract::new(vec![
                    Criterion::new(CriterionId::new("c1").expect("valid criterion ID"), "d")
                        .expect("valid criterion"),
                ])
                .expect("valid acceptance contract"),
                Constraints::new(vec![]).expect("valid constraints"),
            ),
        };
        assert_eq!(decode_payload(payload).expect("payload decodes"), expected);
    }

    #[test]
    fn decode_rejects_malformed_payloads() {
        let base = br#"{"type":"session_cancelled","session_id":"s1"}"#;
        let cases: Vec<(&str, Vec<u8>)> = vec![
            (
                "unknown type",
                br#"{"type":"session_paused","session_id":"s1"}"#.to_vec(),
            ),
            (
                "unknown field",
                br#"{"type":"session_cancelled","session_id":"s1","extra":1}"#.to_vec(),
            ),
            (
                "duplicate field",
                br#"{"type":"session_cancelled","session_id":"s1","session_id":"s2"}"#.to_vec(),
            ),
            ("missing field", br#"{"type":"session_cancelled"}"#.to_vec()),
            ("trailing bytes", [base.as_slice(), b"tail"].concat()),
            (
                "non-string value",
                br#"{"type":"session_cancelled","session_id":7}"#.to_vec(),
            ),
            (
                "raw control character",
                b"{\"type\":\"session_ca\x01ncelled\"}".to_vec(),
            ),
            (
                "invalid escape",
                br#"{"type":"session_ca\qncelled"}"#.to_vec(),
            ),
            (
                "lone surrogate",
                br#"{"type":"session_cancelled","session_id":"\ud800"}"#.to_vec(),
            ),
            (
                "invalid utf-8",
                b"{\"type\":\"session_\xffcancelled\"}".to_vec(),
            ),
        ];
        for (name, payload) in cases {
            assert!(
                decode_payload(&payload).is_err(),
                "case '{name}' should fail"
            );
        }
    }

    #[test]
    fn encode_rejects_oversized_strings_before_building() {
        let oversized = "x".repeat(MAX_STRING_BYTES + 1);
        let event = SessionEvent::SessionCancelled {
            session_id: SessionId::new(oversized).expect("valid session ID"),
        };
        match encode_event(&event) {
            Err(JournalError::AdapterFailure { .. }) => {}
            other => panic!("expected an input-limit AdapterFailure, got {other:?}"),
        }
    }
}
