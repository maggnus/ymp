# Evaluation stage boundaries

## Status

This record fixes the scientific boundary accepted after the plan audit at
[2051194](https://github.com/maggnus/ymp/commit/20511944e30a110ea1edd2d200e8dc5c8d9aba14).
It changes no frozen primary outcome, threshold, strong-single baseline, corpus assignment, seed,
or budget. No coordination or causal result has yet been observed.

## Decision

### Calibration ends at L3

L1-L3 test profile operation and protected-oracle discrimination only. They never enter a weak
single-participant, independent best-of-2, or coordinated arm. The calibration source already calls
them an engineering instrument outside the POC corpus and requires the next level to add
repository-scale decomposition
([`cal-001-calibration.md`](https://github.com/maggnus/ymp/blob/20511944e30a110ea1edd2d200e8dc5c8d9aba14/ymp-docs/research/cal-001-calibration.md#L3-L30)).

### Held-out L4+ tasks precede weak diagnosis

`W1-EXP-01e` owns a separately frozen development task set containing decomposable and
sequential/null strata. Before any model call under `weak-diagnostic-v1`, it freezes the exact task
identifiers, task count, contract digests, and protected-oracle digests. This record does not invent
those inputs. The requirement follows the existing project boundary that fixes task strata,
negative controls, and oracles before comparison
([`ROADMAP.md`](https://github.com/maggnus/ymp/blob/20511944e30a110ea1edd2d200e8dc5c8d9aba14/ymp-docs/ROADMAP.md#L43-L55),
[`PROJECT-CONTRACT.md`](https://github.com/maggnus/ymp/blob/20511944e30a110ea1edd2d200e8dc5c8d9aba14/ymp-docs/PROJECT-CONTRACT.md#L139-L151)).

The weak diagnostic retains its single, blinded best-of-2, and coordinated conditions, separate
diagnostic seed namespace, and existing per-condition budget. Its total schedule becomes knowable
only when `W1-EXP-01e` freezes the task count
([`prt-001-weak-diagnostic.md`](https://github.com/maggnus/ymp/blob/20511944e30a110ea1edd2d200e8dc5c8d9aba14/ymp-docs/research/prt-001-weak-diagnostic.md#L3-L32)).

### Development evidence remains outside the primary comparison

The diagnostic manifest, scheduler, selector, compliance record, and evidence are development
artifacts. They cannot satisfy or alter `W1-EVL-04a`, whose outcome remains the frozen strong-single,
independent best-of-n, and coordinated primary comparison
([`W1-EVL-04a.md`](https://github.com/maggnus/ymp/blob/20511944e30a110ea1edd2d200e8dc5c8d9aba14/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04a.md#L28-L79)).

### Stronger-profile transfer precedes interventions

A qualifying weak result next repeats on the held-out task set with a stronger admitted profile.
Failure to transfer records a bounded weak-only result and stops before message interventions.
Only after transfer may eligible stronger-profile or primary traces supply episodes for absence,
neutral replacement, shuffling, direct-evidence, delay, false-finding, or participant-removal
interventions. The primary comparison and its causal decision remain governed by the preregistered
boundaries
([`map-001-mechanism-map.md`](https://github.com/maggnus/ymp/blob/20511944e30a110ea1edd2d200e8dc5c8d9aba14/ymp-docs/research/map-001-mechanism-map.md#L30-L55),
[`ROADMAP.md`](https://github.com/maggnus/ymp/blob/20511944e30a110ea1edd2d200e8dc5c8d9aba14/ymp-docs/ROADMAP.md#L177-L223)).

### Headless execution does not depend on TUI rendering

The experiment requires agent-facing collaboration and recruitment plus a two-participant managed
run. The collaboration tools are accepted and explicitly hand off to recruitment and the diagnostic
runner
([`W1-COR-03z.md`](https://github.com/maggnus/ymp/blob/20511944e30a110ea1edd2d200e8dc5c8d9aba14/ymp-docs/work/waves/W1/W1-COR-03/tasks/W1-COR-03z.md#L79-L88));
the recruitment tool owns the missing model-callable start path
([`W1-PRD-05j.1.md`](https://github.com/maggnus/ymp/blob/20511944e30a110ea1edd2d200e8dc5c8d9aba14/ymp-docs/work/waves/W1/W1-PRD-05/tasks/W1-PRD-05j/subtasks/W1-PRD-05j.1.md#L31-L74)).
TUI message rendering remains a product and operator-observatory obligation, but its declared scope
is presentation rather than agent execution
([`W1-COR-03e.2.md`](https://github.com/maggnus/ymp/blob/20511944e30a110ea1edd2d200e8dc5c8d9aba14/ymp-docs/work/waves/W1/W1-COR-03/tasks/W1-COR-03e/subtasks/W1-COR-03e.2.md#L31-L49)).

## Rejected alternatives

- Treating L1-L3 as three-arm observations is rejected because it mixes instrument calibration
  with mechanism evidence and asks expected-null elementary cases to establish coordination.
- Borrowing frozen primary tasks, seeds, thresholds, or budgets for development diagnosis is
  rejected because it contaminates the preregistered comparison.
- Running message interventions immediately after a weak-only effect is rejected because it can
  establish a mechanism that does not transfer to the profile used for the product claim.
- Making TUI rendering a headless-run prerequisite is rejected because it blocks execution on a
  presentation layer. Removing the TUI obligation from the POC is also rejected: operator
  observability remains a separate product acceptance concern.
- Naming L4+ task instances here is rejected because only their owning pre-model freeze can make
  those identifiers, counts, contracts, and oracles admissible.

## Falsifiers and stop consequences

- Missing task-set freeze, an oracle that fails its negative control, an unadmitted profile, or an
  unverifiable budget stops the diagnostic before its first model call.
- A diagnostic runner that cannot prove assignment, blinding, equal opportunity, usage, and
  terminal accounting makes the observation noncompliant; it is not repaired by selective reruns.
- A weak cohort that does not repeatedly exceed both controls under its frozen rule records an
  explicit negative stop.
- A weak effect that fails held-out transfer to the stronger admitted profile remains a weak-only
  result and supplies no intervention episode.
- A transferred effect whose intervention changes no receiver action supplies no listening claim;
  action change without task value supplies no collective-reasoning claim.
- Oracle-integrity loss invalidates the affected experiment instead of becoming candidate failure.

## Exact plan implications

1. `W1-EXP-01e` owns the held-out L4+ task and oracle freeze.
2. A diagnostic-runner child separate from `W1-EVL-04a` owns the fake-runtime dry run, blinded
   best-of-2 selector, compliance record, and diagnostic stop enforcement.
3. A separate stronger-profile transfer child owns the pre-intervention transfer or negative stop.
4. `W1-EVL-04a` retains only the frozen primary comparison and its existing strong-single baseline,
   outcome, thresholds, seeds, and budgets.
5. The headless runner depends on accepted collaboration tools, model-callable recruitment, and a
   two-participant managed path; `W1-COR-03e.2` remains parallel TUI work rather than an execution
   prerequisite.

## Evidence status and unknowns

The stage boundary is accepted research design, not execution evidence. Exact L4+ task instances,
their count and digests, the executable diagnostic aggregation and minimum signal, the
stronger-profile transfer schedule, and every resulting observation remain unknown until their
owning freezes and executable checks exist.
