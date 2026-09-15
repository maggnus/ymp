# Roadmap

The approved [self-organizing team model](self-organizing-team-domain-model.md)
owns architecture and scope. This iteration starts with a new implementation;
previous source material is not delivered functionality. The [wave proposal](waves_ideas.md)
explains the sequence, and the [task coverage map](task-coverage.md) links model
elements to concrete owners.

Task definitions, dependencies and status live only in
[tasks/records](tasks/records/). This roadmap describes outcomes without copying
status. Use the [task workflow](development-tasks.md) to select work; numeric IDs
identify tasks, while explicit dependencies determine execution order.

| Wave | Product outcome | Principal integration tasks |
| --- | --- | --- |
| W1 | A new accountable producer/reviewer session: task contract, resource limits, immutable result, evidence, recovery and an audited report through a simple terminal interface. | [Session integration](tasks/records/W1-0014.json), [initial TUI](tasks/records/W1-0015.json). Foundations include [durability](tasks/records/W1-0016.json), [execution and Scripted](tasks/records/W1-0017.json), and [Codex](tasks/records/W1-0018.json). |
| W2 | Independent hidden checks can distinguish relevant defects from correct behavior and justify the result's confirmation grade. | [Hidden design](tasks/records/W2-0001.json), [discriminating verification](tasks/records/W2-0004.json). |
| W3 | Agents initiate contributions, exchange context, take and transfer commitments, object, and adapt their team and plan; independent work can overlap. | [Agent operations](tasks/records/W3-0001.json), [voluntary allocation](tasks/records/W3-0003.json), [dynamic cooperation](tasks/records/W3-0008.json). |
| W4 | Independent attempts execute in isolated workspaces, are compared through a complete check matrix, and yield a justified selection and merge. | [Isolation](tasks/records/W4-0001.json), [attempts](tasks/records/W4-0002.json), [selection and escalation](tasks/records/W4-0003.json). |
| W5 | Qualified experience supports reputation and reusable knowledge, with controlled trials, attributable retrieval and correction after later consequences. | [Reputation](tasks/records/W5-0001.json), [knowledge trials](tasks/records/W5-0004.json), [consequences](tasks/records/W5-0005.json). |
| W6 | The complete model selects methods and contributions through replaceable policies and evaluates the conditions where self-organization is useful. | [Method routing](tasks/records/W6-0003.json), [complete session integration](tasks/records/W6-0004.json), [policy experiments](tasks/records/W6-0005.json). |

The core product question is useful self-organization, not a fixed task-solving
workflow. Key decision mechanisms remain interchangeable through explicit
interfaces. The TUI exposes their behavior and may evolve without imposing a
screen redesign on each wave. Comparisons report benefits, costs, limitations
and negative or inconclusive outcomes separately from implementation completion.
