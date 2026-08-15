---
id: W1-APP-02e.5
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02e
state: accepted
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02e]
blocks: []
created_at: 2026-08-15T01:52:53+08:00
updated_at: 2026-08-15T08:36:04+08:00
started_at: 2026-08-15T08:15:17+08:00
accepted_at: 2026-08-15T08:36:04+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/8367a9a70451d60d22ec33fa113d85858332751f
closure_commit: https://github.com/maggnus/ymp/commit/b0a14a7df978ac861ebe0db51c5aa69d4d416b28
evidence: ["[b0a14a7](https://github.com/maggnus/ymp/commit/b0a14a7df978ac861ebe0db51c5aa69d4d416b28)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02e.5 — The start screen carries the logo and one line of basics

## Outcome

The transcript opens with the logo, one line of basic facts (current directory, application version), and the request invitation; store, the full assurance sentence, 'no run recorded', 'no contract drafted' and duplicate hints leave the feed (the header keeps store and the assurance glyph; the full text lives in ? and /runtimes). Deterministic 80x24 and 120x40 screen tests updated.

## Scope

### In

- See outcome; zones per the approved plan of 2026-08-15.

### Out

- Everything accepted by the parent and sibling nodes.

## Acceptance

- [ ] The transcript opens with the logo, one line of basic facts (current directory, application version), and the request invitation; store, the full assurance sentence, 'no run recorded', 'no contract drafted' and duplicate hints leave the feed (the header keeps store and the assurance glyph; the full text lives in ? and /runtimes). Deterministic 80x24 and 120x40 screen tests updated.

## Current state

Ready. Owner decision 2026-08-15: logo stays with basic info; the rest of the startup text is noise.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- product renders at both sizes carry the logo, one basics line and the invitation with the six
  service lines gone; assurance shown in ? and /runtimes; reviewer mutation broke the key-map
  binding as expected; VISUAL_CONCEPT aligned by a disclosed CTO doc fix
