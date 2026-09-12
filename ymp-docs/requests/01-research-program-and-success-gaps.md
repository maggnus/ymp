# Session findings: research request and success gaps

A request to the team, not a mandate. Every hypothesis below is a candidate for
rejection. Rejecting any or all of them — with reasons — is a fully valid outcome
and often the most valuable one. Find the few ideas worth doing; do not justify
all of them. Do not implement product changes beyond throwaway instrumentation
unless a hypothesis survives evidence and the owner approves.

This note compiles findings from an architecture review session (2026-09-12).
It has two parts: a prioritized research program (Part I) and the gaps that
determine whether the project succeeds at all (Part II, owner-level questions
that research alone will not close).

## Context

ymp is a terminal workspace where a small team of local AI agents (Codex, Claude
Code, GLM) plans, bids, executes, reviews, and verifies tasks directly in the
user's working directory. Selection uses Thompson sampling over Beta(1+S, 1+F)
reputation keyed by (agent_version, competence, difficulty). Deterministic shell
checks cannot be overruled by an approving reviewer.

Key entry points:

- Architecture: `ymp-docs/architecture/system.md`, `providers.md`, `storage.md`
- Team protocol: `ymp-docs/protocols/team.md`
- Engine: `ymp-rust/crates/ymp-runtime/src/engine.rs`
- Provider drivers: `ymp-rust/crates/ymp-providers/src/`
- Evaluation methodology: `ymp-docs/research/evaluation.md`
- Event journal: `turn_started`, `turn_completed`, `turn_failed`,
  `assignment_choice`, `check` events in the store

## Part I — Research hypotheses

### H1 — Infrastructure failure ≠ competence failure (reliability)

A provider timeout, process death, or protocol error currently terminal-blocks
the session as `blocked`, although the docs already state that process/protocol
failures must not count against an agent's task competence.

Question: does a bounded retry with backoff for infrastructure-class errors
(no reputation credit, no attempt increment, preserved cancellation) improve
completion rates without masking genuine content failures? Start from the event
journal (`turn_failed` events exist); prototype against a fault-injecting mock
provider.

### H2 — Ceremony cost and a difficulty-gated fast path

The protocol has fixed phases: every team member plans independently (N turns),
then bids (up to N turns), before any execution. Hypothesis: for simple tasks
the ceremony dominates total cost while contributing nothing to outcome quality.

Measure turns/tokens/latency per phase across task sizes using demo/mock runs
and existing ymp-evals scenarios. If confirmed, evaluate a fast path gated by
plan difficulty (e.g. single-proposal plan, no competitive bidding for `simple`)
that preserves the independent-review invariant.

### H3 — Effort tiers and the low-effort ensemble (one line of work)

There is currently no effort/reasoning knob: bids, plans, and final reviews all
run at the provider default.

Step 1 — verify per-turn controllability through existing drivers (Codex
`modelReasoningEffort`, Claude thinking, ACP/GLM thinking option).

Step 2 — test a role-mapped funnel: bid/plan → low, review/arbitration → medium,
`final_review` → xhigh, `execute` → by task difficulty. Critical detail:
reputation must then be keyed including effort, or observations across tiers
confound.

Step 3 — the ensemble question: the planning stage is already a blind ensemble
(independent proposals, adversarial review, arbitration); at equal token budget,
does a team of low-effort agents match one xhigh agent on success rate for tasks
with deterministic checks? Identify in which funnel stages cheap agents
contribute (generation, critique) and where they do not (final verification of
complex results).

Steps 1–2 use mock/demo; step 3 requires real providers: propose the protocol
(teams, tasks, budgets, metrics) for owner approval; do not spend provider quota
on your own initiative. Metric throughout: cost per verified outcome at equal
success rate.

### H4 — Cost-aware selection

Current selection optimizes success probability only; price is invisible, so
cheap agents reach the stage only through sampling variance.

Hypothesis: utility = sampled success − λ·normalized cost (usage is already
recorded per turn) increases aggregate verified outcomes under a fixed token
budget. Do a retrospective analysis of existing `assignment_choice` events
before proposing any interventional change.

### H5 — Context hygiene (three cheap measurements; suitable as a standalone small task)

(a) `follow_up` injects the full file listing of the working directory into the
prompt, unbounded — measure token cost on a large repo and the effect of a
bounded/truncated listing on answer quality.

(b) The per-turn context window is the last 12 messages — is that adequate for
reviews of long executions?

(c) The native resume key omits purpose, so one agent's plan, review, and execute
turns share a native provider thread — does separating them improve quality or
cost?

Each is a small A/B with a mock spot-check plus at most one real confirmation run.

### H6 — Check-command gate, minimal scope

Plan-supplied shell checks execute in the user's directory with full access.
Within the current trust model (no sandboxing, no verifier plane, no redesign):
would a first-run user confirmation or a prefix allowlist (cargo, npm, pytest, …)
reduce the obvious injection surface without breaking autonomy? Evaluate friction
and failure modes honestly; if it cannot be done lightly, reject it.

### H7 — Concentration in engine.rs and tui/state.rs: cost or aesthetics?

`ymp-runtime/src/engine.rs` (~1250 lines) holds orchestration, planning,
bidding, reputation, learning, checks, follow-up policy, and all prompt
templates as inline `format!` strings; `ymp-tui/src/state.rs` (~1770 lines)
shows the same pattern.

Before proposing any split, establish whether this concentration is an active
cost: what fraction of protocol changes currently require touching engine.rs;
do prompt edits and orchestration edits conflict in practice; is there evidence
of review latency or regression risk attributable to the file size (git history,
churn, incident notes in ymp-docs/work if present). If the evidence supports
action, evaluate this minimal decomposition in priority order, each step landing
independently:

1. Extract all prompt templates into a dedicated module — mechanical, no
   behavior change, immediately releasable.
2. Extract selection (choose/observe/bid) with its reputation logic.
3. Extract learning (learn + memory review).

If the evidence shows the module is cohesive and churn is low, rejecting the
refactor with that analysis is the preferred outcome — do not split for
aesthetics.

### Explicit exclusions

Do not propose reviving the retired predecessor's machinery: three-plane trust
topology, capability brokers, contracts/escrow/obligation trees, per-attempt
private workspaces with immutable content-addressed candidates, hash-linked
event journals, strict Linux isolation, or crash continuation. The
direct-working-directory model stays. Also out of scope: storage rewrites,
daemon/server modes, and anything that adds mandatory ceremony.

## Part II — Success gaps (owner-level, not research tasks)

These are the gaps that determine whether the project succeeds. They are
recorded here so the team sees the whole picture; closing them requires owner
decisions and sustained use, not a single investigation.

1. **Measured proof of the core hypothesis — gap #1.** The value claim "team +
   memory + adaptive assignment beats a single agent" has zero measured data.
   The `--no-memory` / `--no-adaptive` flags exist exactly for this experiment
   and have never been used; ymp-evals has scenarios but no results. The
   evaluation methodology (`ymp-docs/research/evaluation.md`) already defines
   the four team modes and honest metrics — what is missing is execution.
   The predecessor died with 134k lines and no measurements; do not repeat
   that pattern. First falsifiable claims to run: solo vs team at equal budget
   on 10–20 tasks; independent review catches defect classes that solo misses.

2. **The task class where a team honestly wins.** For most tasks one strong
   agent is cheaper and faster (see H2: 6–8 ceremony turns before execution).
   A team plausibly wins on independent verification, long multi-file changes,
   and cumulative project context. Name this class explicitly — it determines
   design, evaluation scenarios, and any future positioning. Otherwise this is
   a tool in search of a problem.

3. **Reliability of real runs.** One `blocked` status caused by a provider
   timeout destroys trust in the tool permanently (see H1). Mock tests do not
   catch what happens with a real Codex on turn 40. Dogfooding — running ymp
   on ymp's own development — is the only way to accumulate both reliability
   fixes and reputation/memory data. The flywheel turns only after weeks of
   real use.

4. **A recovery story for the working directory.** Writing directly to the
   user's directory means one bad run can damage real work. Today only the
   user's git saves them — and it may not exist. Minimum honest contract:
   warn on a dirty git status at start; gate check commands (see H6); give
   `/diff` a clear "what to revert by hand" path. Not isolation — just an
   honest recovery story.

5. **Memory usefulness, not just existence.** FTS5 keyword retrieval is naive;
   junk memory stays active forever (no decay; `supersedes` exists as a field
   but has no mechanics); each learning step spends turn budget. Worse, bad
   memory pollutes the context of every turn — it can silently degrade quality,
   and that needs separate measurement.

6. **Economics.** Usage is collected but is never a constraint. No budget
   ceiling in tokens or cost per session or task — without it, equal-budget
   comparisons (H3/H4) and everyday use are equally blind. A small feature
   with large strategic effect: it turns agent selection into an economy.

7. **Path to other users (if external users are part of "success").** macOS-only
   (unix sockets, libc), the Claude bridge is compiled into the checkout path,
   npm build required, pre-authenticated CLIs assumed. Fine for a personal
   tool; a decision is needed on whether the install story ever matters.

## Method

Ground every claim in the actual code and event journal; cite file paths.
Prefer mock/demo runs and retrospective analysis; real provider runs require
explicit owner approval with a stated budget. Report honest negatives — a clean
rejection with evidence is a deliverable, not a failure. Documentation in
English under `ymp-docs/research/`.

## Deliverable

A research note with, per hypothesis: verdict (investigate now / park / reject),
supporting evidence from the journal or experiments, and where applicable a
proposed experiment protocol with its budget. End with a single prioritized
list of the few changes worth making, each small enough to land independently.

## Appendix — H8: Outcome cache with verification-grounded reuse

Added after the initial request; the team may treat it with the same freedom of
rejection as Part I.

### Origin: first real measurements

Three near-identical real requests ("create a simple html page") in the same
project produced:

| Run | Turns | Input tokens | Cache-read | Time |
| --- | --- | --- | --- | --- |
| 1 | 20 | 838k | ~240k | 19.5 min |
| 2 | 13 | 260k | — | 6.7 min |
| 3 | 15 | 1050k | ~857k | 13.6 min |

Expectation was that repeats get cheaper; run 3 was the most expensive. The
journal shows why: memory changed the task text (the plan became "verify the
existing index.html without overwriting") but not the ceremony length —
a completed, checked outcome was re-derived from scratch at full protocol cost.
Also observed: cost-blind selection (one agent consumed 89% of tokens across
25x-different per-turn prices — direct evidence for H4), purpose-blind native
session reuse making trivial turns pay full thread history (hard evidence for
H5c), and a session stuck in `running` after all turns completed (see bug
note below). These runs are the evidence base for H2/H4/H5; use them.

### Hypothesis

A repeat request that semantically matches an existing verified outcome can be
answered from the record for ~one cheap turn instead of a full protocol run.

### Design direction: reuse decided on evidence, not similarity

A prompt hash is not an option: the three real prompts were lexically
different and one was in Russian. A classic embedding semantic cache is also
the wrong tool here: ymp has no direct model-API access by design (providers
own their credentials), and threshold-based answer substitution is the one
genuinely dangerous part of semantic caching. What ymp uniquely has is a
verified outcome and measurable file state. Therefore:

1. **Gate turn before full ceremony**: one cheap read-only turn that receives
   the user prompt plus the project's completed sessions (id, prompt, summary,
   check status) and current file state. It returns the existing
   `{"action":"answer"|"task"}` contract — the same fork `follow_up` already
   implements.
2. **Reuse decided on checkable facts, not closeness**: session completed,
   acceptance checks green, and the workspace fingerprint unchanged since
   verification (SHA-256 fingerprints already exist in ymp-workspace;
   comparison is local and cheap).
3. **Degradation, not silent staleness**: if files changed after verification,
   the gate degrades to an honest "the result existed, but the tree has
   changed; verification needed" answer rather than a stale cached one.
4. **Attributed answers**: the answer always cites the record ("already done
   in session X, checks green, path P"). An explicit redo request forces the
   full run through the same answer/task fork.
5. **Retrieval**: start with FTS5 or simply the last N sessions of the project;
   at a handful of sessions these are indistinguishable. Retrieval quality
   becomes a real question only at hundreds of sessions.

Separation of concerns: semantic matching is a cheap router that may err;
reuse is a decision on verified facts (checks + fingerprint). There is no
threshold-based answer substitution.

### Cost and failure modes

Expected cost: one low-price turn (3–10k input) instead of 260k–1050k. On the
three real runs above, runs 2 and 3 would have qualified for the gate.

Honest caveats: (a) the gate is itself ceremony for genuinely new tasks —
condition it on the project having at least one completed session; (b) prompts
that are semantically close but materially different ("now with a dark theme")
must resolve to `task`, not `answer` — this is the primary failure mode and
needs explicit test scenarios; (c) fingerprint equality proves files unchanged,
not intent satisfied — the gate answers "already delivered and verified", never
"close enough".

### Bug note (out of band)

Run 3 finished all 15 turns (synthesis completed 04:36) but the session status
stayed `running` — the final status save did not happen (likely host exit
before save). Worth a separate look: zombie `running` status after completed
turns.

## Appendix — H9: Effective tokens — honest usage accounting

Added after the initial request; same freedom of rejection as the rest.

### Origin: the displayed token number is not the real one

The three real runs (see H8 origin) show raw input sums of 838k / 260k /
1050k, but 82% of run 3's input was cache-read tokens. Cash cost is therefore
roughly 5x lower than the raw number suggests, while the user currently sees
only the raw sum. Two distortions in opposite directions:

- **Overstatement**: raw input counts cache-read tokens at full weight, so a
  cache-friendly protocol run looks ~5x more expensive than it is.
- **Understatement**: there is another usage stream in `token_usage` that
  disagrees with `events.turn_completed` — for codex/claude turns the event's
  `usage` payload recorded zeros while `token_usage` held real counts
  (native nested `usage.total.*` vs flat shapes). Any number derived from only
  one stream is wrong for part of the team.

Also: reasoning tokens are a subset of output (already handled in
ymp-core/src/usage.rs docstring — they must not be added on top), and provider
shapes differ (Codex `usage.total.inputTokens` vs GLM flat `inputTokens` vs
Claude bridge's own mapping).

### Hypothesis

A single "effective tokens" metric — and only that metric displayed — makes
cost legible: users see what a run actually costs, and H3/H4/H8 experiments
get a comparable denominator.

### Design direction

1. **Define effective input** = `input − cache_read − cache_write·k_cache` …
   decide the exact formula deliberately, not implicitly. A common
   approximation: `input_noncached + cache_read × 0.1 + cache_write × 1.25`.
   The coefficients are pricing-model assumptions; make them visible and
   configurable rather than hidden constants, and keep the raw fields intact
   alongside.
2. **One source of truth**: `token_usage` (or `events`) — pick one, and make
   the other derived or removed. The per-provider normalization belongs in
   ymp-providers at ingestion time (it already knows the native shape), so
   everything downstream sees one flat schema. The current situation —
   `turn_completed.usage` empty for two providers while `token_usage` is
   filled — must become impossible.
3. **Display both numbers, label them honestly**: "input 1.05M (857k cached,
   ~200k effective)" in `/limits` and the TUI. Never show the raw sum alone.
4. **Use effective tokens as the experiment denominator**: H3 (cost per
   verified outcome), H4 (λ·cost in selection), H8 (gate vs full-run cost)
   all silently change conclusions if computed on raw input. Any equal-budget
   protocol must state whether "budget" means raw or effective tokens.

### Cheap first step

Retrospective: recompute the three real runs in effective tokens from existing
`token_usage` rows (no model calls) and report the table. If the ranking of
runs changes versus raw sums, that alone justifies the metric.

### Caveat

Cache pricing differs per provider and over time; "effective" is a reporting
convention, not a bill. If the providers ever expose money directly
(Claude/Codex cost fields), prefer the money field and keep effective tokens
as its local fallback.
