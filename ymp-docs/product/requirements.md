# Product intent

The owner-authored [intent.md](../../intent.md) is the concise product definition. This document states its goals and principles in English. Definitions belong in [Domain terminology](entities.md), and operating details in the [team and effort policy](../architecture/team-and-effort-policy.md).

The owner reopened the architecture on 2026-09-12 and subsequently requested the bounded dynamic team policy to be recorded. The documented direction does not resume implementation, establish an optimal policy or select a coordination topology. YMP-010 remains the broader architecture decision; YMP-011 tracks this documentation update.

YMP is a console TUI utility for autonomously solving user tasks using available AI agents.

## Goals

- Increase the probability of solving difficult tasks compared with one strong agent under a comparable budget.
- Reduce resource use at comparable result quality.
- Reduce the time to a verified result.
- Reduce the need for user intervention.
- Improve efficiency on later tasks through accumulated verified experience.

These are product goals, not measured achievements. The [value proposition](value-proposition.md) explains the comparisons and evidence needed to assess them.

## Principles

1. The person supplies the goal and constraints without managing the team's internal work.
2. Each user task has its own session; clarifications within that task retain its history.
3. Form the team from the eligible pool for the task; membership and size are dynamic by default within recorded limits.
4. The user may pin team size or roster, models and effort.
5. Active concurrency depends on useful work and available resources.
6. Select work roles, coordination and method for the task under the user's constraints; no coordination topology is prescribed by terminology alone.
7. Agents may propose subtasks, accept responsibility and revise assignments through the validated decision process.
8. Select team size, executors, models and effort jointly using complexity, risk, resources and verified experience.
9. Each assignment uses settings permitted by the provider, model and user constraints.
10. Reconsider the plan, membership and settings based on work results; failure does not automatically increase resource use.
11. Agents communicate with the team or an addressed participant through the shared board.
12. Independently verify task results before final acceptance.
13. An individual agent's failure must not destroy completed verified work.
14. YMP controls the shared budget, including coordination and verification costs, subject to the documented native enforcement and accounting limits.
15. Initial work receives a bounded allowance; retain resources for required verification.
16. Self-assessment and unsupported claims do not increase reputation.
17. Persist data, events, decisions and knowledge incrementally.
18. Session history must allow the decision process to be reconstructed.
19. General knowledge and reputation remain available in later sessions.
20. Knowledge retains provenance and links to supporting results.
21. Correct or supersede wrong or outdated knowledge using newer verified evidence.
22. Team work remains observable and explainable without requiring user intervention.

## Local and universal

The executable is ymp, implemented in Rust with a Ratatui interface, a common chat and slash commands. The task domain is universal: software, documents, data, research, planning and other work supported by the installed agents. Git and source code are not prerequisites; verification must fit the requested result.

Use the installed agents' native authentication and tools. Application metadata lives under ~/.ymp2, including sessions, participants, decisions, messages, usage, knowledge and experience. Native providers retain their own authentication and storage; credentials are never copied into ymp. Deliverables must reach the user's selected destination. Workspace execution and publication must follow the applicable workspace policy; the team/effort decision does not choose a new file-isolation mechanism.

## Delivery rule

Agent identity, native model/effort controls, autonomous selection within constraints, independent verification, a common budget and incremental experience are product capabilities. Manual internal assignment is not a prerequisite. Comparative experiments assess particular policies separately; they are not claims that proposed features already exist.

[ADR 0002](../adr/0002-agent-identity-and-reasoning.md) summarizes the design direction. The [project tasks](../tasks/README.md) distinguish documentation, pending architecture and product delivery. Edit intent.md only as authorized by the owner; derived documentation, code comments and interface strings remain in English. All UI work uses Claude Opus 5 with max thinking.
