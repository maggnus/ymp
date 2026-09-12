# Assignment authority

YMP-120 implements the permission boundary in the [runtime contract](runtime-contract.md). The existing local team server remains the authority boundary for coordination tools.

## Admission and expiry

`TeamServer::admit` creates a fresh capability for one session, agent, assignment and invocation. A typed `GrantRecord` lists its allowed `TeamOperation` values. `Store::begin_invocation_with_grants` commits that record, assignment, invocation, usage ordinal and typed events together. Duplicate grant IDs, occupied agent claims, competing task executors, stale task attempts and mismatched bindings fail without a partial grant. Grant metadata is versioned JSON under `team_grant:v1:<id>` in the existing SQLite key/value table; schema 3 remains compatible.

The returned random token stays in the runtime's active map and current native process configuration. It is never included in application records, events or debug output. A durable grant ID is an audit reference, not a credential. The native adapter already runs in a fresh outer process for each invocation; compatible native conversation history can still be resumed.

Completion, failure, cancellation, timeout and dropped invocation futures remove the live capability through the invocation guard. Their durable revocation events commit with terminal invocation state. A task state, attempt or assignee transition interrupts its still-running assignments and revokes their grants in the same task transaction. The storage request path revalidates current assignment/task state, so an old token is rejected even if a task transition occurs outside the server helper. Server shutdown clears its live map and closes its remaining assignments as interrupted. Process crashes leave no trusted live map; session recovery closes uncertain invocations before fresh inspection admission.

## Team operations

The server accepts only board read/post, task-list/proposal and memory-search/proposal operations allowed by the current grant. Caller identity comes from the runtime map. Unknown tool properties, including supplied session/agent/assignment/permission/status fields, are rejected. Recipients must be members of the captured session team; they need not have an active assignment.

Proposals create pending board messages or proposed knowledge. They do not create assignments, grant permissions, accept tasks, activate knowledge, confirm outcomes or update reputation. Those transitions remain runtime-only APIs and retain their independent validation requirements.

The stdio bridge adds a request identity scoped to its process and MCP request ID. Authorization, replay detection and the operation run under the same active-map lock. A storage transaction validates the durable grant and writes the effect and linked event together. The same request can commit at most once; even a failed request ID cannot be reused within its grant. A request racing revocation either commits before the terminal transaction or is rejected. There is no promise of exactly-once delivery when a client creates a new request ID after losing a response. Each grant admits at most 10,000 request identities. The socket reads at most 1 MiB per request and retains the existing five-second input deadline.

## Native permission boundary

Codex receives the current sandbox, approval policy and explicit MCP configuration on both `thread/start` and `thread/resume`. Its MCP configuration references `YMP_MCP_TOKEN` from the current process environment; the secret is absent from the thread RPC configuration. Claude SDK invocations provide current MCP credentials, resume identity, permission mode and read tool restrictions each time. ACP `session/new` and `session/load` receive the current MCP configuration, followed by an explicitly selected advertised permission mode before the prompt. A read assignment selects an advertised read-only, plan or default mode; approval requests are denied. An unavailable compatible ACP mode fails before inference. A provider request for an unknown guarantee or a permission mode inconsistent with its read/write assignment is rejected.

These checks do not create OS containment. Claude tool restrictions and ACP mode/approval callbacks cannot guarantee isolation from the host filesystem or processes. Codex uses its actual native sandbox. Every invocation records a typed observation describing the boundary. Native user/project configuration, installed tools and the selected working-directory policy still govern host access. An agent with host write authority could reach files outside the team API; the team token is not a security boundary against a malicious process running as the same OS user. No provider credential is read, copied or persisted by this mechanism.

## Offline evidence

`cargo test -p ymp-runtime mcp::tests` covers terminal states, rotation, cross-session/assignment arguments, restricted operations, self-grant attempts, restart/drop, rollback after an insert failure, concurrent claims, replay and cancellation races. The pre-change controls reproduced successful stale-token writes, accepted forged binding arguments and two effects from one concurrent request ID. Restart and unknown runtime-method rejection were already passing controls and remain covered. A deliberate mutation that reused a process-global active map made the restart control fail by accepting the old token; restoring an empty map made it pass again.

`cargo test -p ymp-providers --test authority` covers native continuation with current MCP configuration, ACP mode reset, unsupported guarantees and redacted debug output. Before correction, the fixtures observed no Codex resume MCP override, no ACP read-mode reset and an accepted unsupported permission guarantee. Claude's offline SDK protocol fixture verifies fresh credentials and read restrictions while keeping the saved conversation identity. No real provider inference or provider credential access is part of these checks.
