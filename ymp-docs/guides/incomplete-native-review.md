# Incomplete native reviews

ACP `max_tokens` means that the native response reached its output limit before
completion. It does not establish that the task passed review or that the whole
session exhausted its budget. See the [ACP prompt-turn protocol](https://agentclientprotocol.com/protocol/v1/prompt-turn).

Starting with ymp 0.4.1, this stop is recorded as `native_output_limit`. Reported
usage is retained, including partial-accounting flags. Streamed content remains
an incomplete response and cannot become an acceptance decision.

For `review`, `review_plan`, `final_review` and `review_memory`, the runtime may
request one fresh read-only review. The maximum is two invocations per review
request, reduced to one when the captured attempt allowance is one. Each attempt
requires normal admission, a new grant and its own usage record. The requested
model and effort stay unchanged. A failed native continuation is not reused.
The retry asks for a concise complete verdict using the retained evidence.

The automatic retry does not rerun the producer or the preceding local checks.
Repeated output exhaustion leaves the review incomplete. Other provider stops,
runtime deadlines/output ceilings, cancellation and budget denials do not take
this retry route. In particular, incomplete usage under a token ceiling still
stops further admission under the default `Stop` policy. A retry request event
does not imply that admission succeeded.

An explicit session resume is a new recovery request. It retains previous spend
and captured constraints, inspects existing state and leaves a submitted task in
review instead of replaying its producer. It may rerun recorded local checks to
assess current files. Resume cannot restore earlier file contents or guarantee
that a different effort will finish within a native response limit.

## Reported poker session

Session `0b24eea4-0c01-4d9e-b137-0f432cd7d2ac` stopped during independent GLM review,
after the engine implementation had been submitted. The failed response reported
8,192 output tokens, including 8,185 reasoning tokens; ACP labels these usage
figures as the last model request only. The engine and its 23 passing Node tests
remain in `/Users/maggnus/Downloads/_ymp2`. The welcome markup is still present:
the playable interface and browser verification tasks have not run.

The installed native catalog observed `glm-5.2` efforts `none`, `high` and `max`.
To explicitly replace GLM's default `max` for subsequent assignments in this
session, save this session-scoped override under application metadata:

```json
[
  {"agent_id":"glm","settings":{"model":"glm-5.2","effort":"none"}}
]
```

The local incident correction prepares that file at
`~/.ymp2/recovery/0b24eea4-review-settings.json`. Review it before running:

```sh
/Users/maggnus/Code/ymp2/target/release/ymp \
  -C /Users/maggnus/Downloads/_ymp2 \
  resume 0b24eea4-0c01-4d9e-b137-0f432cd7d2ac --headless \
  --assignment-settings ~/.ymp2/recovery/0b24eea4-review-settings.json
```

This command starts real provider work, including remaining implementation tasks
after review succeeds. It is not a test command. The override keeps the captured
agent IDs and any fixed settings; conflicting pins are rejected. Other agents'
settings remain inherited. Supplying assignment rules replaces the previous rule
list, so merge any rules you want to retain for another session. Historical
invocations are never relabeled or rewritten. The investigation did not resume
this session or invoke paid models.
