# Evaluating accumulated experience

The implementation stores experience; its existence does not demonstrate better outcomes.

Use disjoint adaptation and evaluation tasks. Never add held-out expected answers to memory. Freeze provider versions, model identifiers, profile instructions, and test fixtures when comparing treatments. Use independent acceptance checks against the delivered artifacts, not the team's final claim.

Compare a single-agent baseline and four team configurations:

| Mode | Memory | Adaptive assignment |
| --- | --- | --- |
| Team baseline | Off | Off |
| Memory only | On | Off |
| Assignment only | Off | On |
| Combined | On | On |

`ymp run --no-memory --no-adaptive`, `--no-adaptive`, `--no-memory`, and the default invocation select these team modes. Use separate application homes per treatment. First accumulate experience on adaptation tasks, then evaluate on held-out fixtures with the same budget. Record resolved model IDs when the provider supplies them; aliases alone do not freeze a model.

Primary outcome: independently verified task success. Secondary outcomes: regressions, elapsed time, provider-reported usage, number of attempts, and coordination calls. Missing cost data is unknown, never zero. Compare repeated runs and report uncertainty; one successful demonstration establishes functionality only.

## Research foundations

- [Contract Net Protocol](https://cse-robotics.engr.tamu.edu/dshell/cs631/papers/smith80contract.pdf): task announcement and bidding.
- [A Tutorial on Thompson Sampling](https://arxiv.org/abs/1707.02038): balancing existing evidence and exploration.
- [Memp](https://arxiv.org/abs/2508.06433): procedural memory derived from experience.
- [Why Do Multi-Agent LLM Systems Fail?](https://arxiv.org/abs/2503.13657): coordination and verification failure modes.
- [Towards a Science of Scaling Agent Systems](https://arxiv.org/abs/2512.08296): team effectiveness depends on task structure.
