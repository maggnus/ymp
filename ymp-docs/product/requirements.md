# Product intent

The owner-authored [intent.md](../../intent.md) is the concise product definition. This document states its goals and principles in English. Definitions belong in [Domain terminology](entities.md), and operating details in the [team and effort policy](../architecture/team-and-effort-policy.md).

The owner approved the final intent on 2026-09-12 and requested the [delivery plan](../tasks/plan.md). The approved source and its digest are recorded in the task register. Product goals and protocol invariants are settled; executable contracts and implementation are tracked separately. Historical design discussions do not override the approved wording.

YMP is a console TUI utility for autonomously solving user tasks using available AI agents.

## Goals

- Increase the probability of solving difficult tasks compared with one strong agent under a comparable budget.
- Reduce resource use at comparable result quality.
- Reduce the time to a verified result where useful parallel execution is possible.
- Reduce the need for user intervention.
- Improve efficiency on later tasks through accumulated verified experience.

These are product goals, not measured achievements. The [value proposition](value-proposition.md) explains the comparisons and evidence needed to assess them.

## Principles

1. The person supplies the goal and constraints without managing the team's internal work.
2. Each user task has its own session; clarifications within that task retain its history.
3. Form the initial team from the eligible pool for the task and allow changes within recorded bounds.
4. The user may pin team size or roster, models and effort.
5. Agents have no permanent hierarchy: roles and permissions last for one assignment and do not carry into later assignments.
6. Agents propose and negotiate work; the trusted runtime commits assignments, permissions and final acceptance.
7. Active concurrency depends on useful work and available resources.
8. Select roles, coordination and solution method for the task under user constraints.
9. Agents may propose subtasks, accept responsibility and revise assignment proposals.
10. Choose executors, models and effort for each assignment using complexity, risk, resources and verified experience.
11. Each assignment uses settings permitted by the provider, model and user constraints.
12. The plan, team membership and settings may be revised based on work results.
13. A failure prompts review of its cause and the work method; it does not require increasing resources.
14. Agents communicate with the team or an addressed participant through the shared board.
15. Task results undergo independent verification before final acceptance.
16. Record the verification basis. An accepted result without confirmation remains unconfirmed and does not increase reputation.
17. An individual agent's failure must not destroy completed and reviewed work.
18. YMP controls the shared budget, including coordination and verification costs.
19. Initial work receives a bounded allowance; retain resources for result verification.
20. Self-assessment and unsupported claims do not increase reputation.
21. Persist data, events, decisions and knowledge incrementally.
22. Session history must allow the decision process to be reconstructed.
23. General knowledge and reputation remain available in later sessions.
24. Knowledge retains provenance, applicability and links to supporting results.
25. Correct or supersede wrong or outdated knowledge using newer verified evidence.
26. Team work remains observable and explainable without requiring user intervention.

## Local and universal

The executable is ymp, implemented in Rust with a Ratatui interface, a common chat and slash commands. The task domain is universal: software, documents, data, research, planning and other work supported by the installed agents. Git and source code are not prerequisites; verification must fit the requested result.

Use the installed agents' native authentication and tools. Application metadata lives under ~/.ymp2, including sessions, participants, decisions, messages, usage, knowledge and experience. Native providers retain their own authentication and storage; credentials are never copied into ymp. Deliverables must reach the user's selected destination. Workspace execution and publication must follow the applicable workspace policy; the team/effort decision does not choose a new file-isolation mechanism.

## Delivery rule

Agent identity, native model/effort controls, autonomous selection within constraints, independent verification with explicit confirmation status, assignment-scoped authority, a common budget and incremental experience are product capabilities. Manual internal assignment is not a prerequisite. Comparative experiments assess particular policies separately; they are not claims that proposed features already exist.

[ADR 0002](../adr/0002-agent-identity-and-reasoning.md) summarizes the design direction. The [project tasks](../tasks/README.md) distinguish documentation, executable-contract design and product delivery. Edit intent.md only as authorized by the owner; derived documentation, code comments and interface strings remain in English. All UI work uses Claude Opus 5 with max thinking.
