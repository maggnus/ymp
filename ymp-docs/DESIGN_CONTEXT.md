# Design context

This is the short product handoff for Claude Design. Read it together with
[`USER_JOURNEY.md`](USER_JOURNEY.md). The journey defines what the user must accomplish; this file
defines what ymp is and which product boundaries the design must preserve. It does not prescribe
screens, layout, components, colours or navigation.

## Product essence

ymp is a local terminal product that turns an ordinary software goal into a working, checked result
through an autonomous team of coding agents.

The user writes what they want, for example:

> Сделай игру «Морской бой»

ymp handles project analysis, requirement and check preparation, team formation, task negotiation,
implementation, candidate creation and verification. The user receives a result, sees what was and
was not checked, runs it in its own environment, asks for changes and explicitly applies the chosen
version to the project.

The primary experience is about the result. It is not a protocol administration console and not an
opaque single-agent chat.

## What makes ymp different

ymp may use multiple providers, models and agents in one bounded run. The system forms the team and
participants may recruit, negotiate, challenge, revise and contribute competing candidates within
the allowed pool and limits.

The interface must show only participants and activity actually observed in the current run. Until
multi-participant execution is proven on the primary path, it must honestly show a single participant
or an unavailable recruitment state rather than illustrating a hypothetical team.

The user does not manually assign every task, but the team is not hidden. The product keeps a compact
factual picture of:

- how many participants are active;
- provider, model and profile for each participant;
- declared work and current state;
- material board activity;
- movement of artifacts and candidates;
- verification state, elapsed time and known resource use.

Detailed team, task, board and artifact provenance remain inspectable on demand.

## User control

The user controls consequences and boundaries, not the internal choreography.

- Providers can be enabled or disabled. Enabling is consent to send project data to that provider
  and starts a background request for available models. A catalogue error leaves the provider
  enabled but contributes no models until retry succeeds.
- The pool defines which provider/model/profile entries may be used for future participant starts
  and carries available ceilings. A current run keeps its frozen pool snapshot.
- `/agents` concerns participants working in the current run.
- `/pool` concerns resources eligible for future runs.
- `/board` exposes scoped messages and communication facts. A message written by the user is an
  attributed human intervention, not a hidden task assignment or authority transfer.
- The user can cancel, inspect the result and evidence, request a change, and explicitly apply or
  externally publish a selected result.

Requirement and check preparation may appear as dim, collapsible system trace in the main
conversation. It must not become a contract-approval ceremony or require typing internal IDs.

## Result boundary

ymp does not embed the generated application or manual result testing in its own TUI.

- Automated checks run through their isolated product or verifier path.
- ymp shows the exact external run/open command or path, status and evidence.
- The user plays or evaluates the result in its own browser, separate terminal or process.
- Feedback returns to the main ymp conversation.
- Applying files to the target project is an explicit consequence-bearing decision and may be
  combined with accepting the selected version, but never happens implicitly.

The single-executable invariant applies to the ymp distribution. A generated result may contain an
ordinary multi-file project.

## Internal mechanisms

Contracts, capabilities, attestations, verifier internals, leases, fencing, private workspaces and
low-level accounting remain inside the product. They may appear in advanced diagnostics when needed
to explain a failure, but they are not normal user prerequisites.

## Non-negotiable semantics

- Do not rank or grade agents, models, messages or candidates.
- Show only declared work and observed state; do not infer semantic roles, intelligence or quality.
- Multiple participants, messages or task changes do not prove useful coordination or
  self-organization.
- Publication, availability, delivery, citation, revision and later action are distinct facts.
- A board message is inert data and cannot itself invoke an action or transfer authority.
- Citation and revision create no action, capability, agreement, influence, listening or task value.
- Separate automatically checked facts from what remains for human evaluation.
- Name unavailable POC behavior as a gap instead of drawing it as functional.
- Every failure state must say what happened and what the user can do next.

## Honest POC boundary

The desired journey is ahead of some current implementation:

- Russian free-form goals are not yet normalized into structured checks;
- managed provider sign-in is incomplete;
- pool ceilings are not fully conducted through every start, recruitment and spend path;
- user-facing board reading is unfinished and human board writing is absent;
- several-participant execution is not yet proven end to end on the primary product path;
- current result storage and continuation remain limited;
- money and time are not yet complete enforceable product limits.

Design these as honest unavailable, empty, blocked or diagnostic states. Do not present them as
working controls.

## Design task

Claude Design should derive the information architecture and interaction model from
`USER_JOURNEY.md` and this context. It is free to propose the composition and visual language.

Do not use the existing `VISUAL_CONCEPT.md`, `COLLECTIVE-TUI.md`, HTML/PDF screens or rejected
candidate `893a7d1` as design authority. They preserve an older system-centred interface and are
historical reference only.
