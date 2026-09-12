# YMP-123 public stdio verification

13/09 00:09 HKT — The public facade delivers project-scoped inspection and explicit
bounded execution through the common runtime. Development base: `e1a1519` (the four
accepted YMP-110/YMP-113 backend commits over `2695d69`). This report covers the facade;
it does not independently accept that combined backend ancestry or the final release.

## Outcome

`ymp -C PROJECT mcp [--client-id ID] [--allow-execution]` serves MCP 2025-06-18 over
stdio. Reads are the launch default. The five versioned tools are published in
[`public-mcp-tools-v1.json`](../../ymp-docs/architecture/public-mcp-tools-v1.json).
[`public-mcp.md`](../../ymp-docs/architecture/public-mcp.md) records the runnable client
configuration, framing/lifecycle, failure codes, limits, IDs and recovery semantics.

External identities never become assignment capabilities. Project/session references
are checked before reads/resume; knowledge source versions resolve through YMP-113.
A structured project/client/request tuple is hashed for an atomic metadata claim;
a normalized typed payload fingerprint rejects conflicting reuse. Engine accepts the
server-generated UUID through an insert-only session creation path. Existing session
history and captured spend are preserved on an attempted duplicate.

All providers used by these checks are mock/scripted. The real-process client test
uses isolated metadata/HOME/projects and fixed `mock` model, `low` effort. No installed
native default, authentication probe or real-provider inference is involved. The
internal bridge lifecycle check uses a synthetic string, not a live team capability.

## Commands and observed exits

| Command | Exit / evidence |
| --- | --- |
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `cargo test --workspace` | 0; 270 tests passed, 0 failed |
| `cargo test -p ymp-runtime mcp -- --nocapture` | 0; 18 relevant MCP/grant tests |
| `cargo test -p ymp-runtime public_mcp -- --nocapture` | 0; published-schema equality and duplicate identified start retaining spend |
| `cargo build -p ymp-cli` | 0 |
| `/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp` | 0; official MCP Python SDK 1.28.1 against actual executable |
| `npm ci --ignore-scripts` in `ymp-bridges/claude` | 0 |
| `npm run check` in `ymp-bridges/claude` | 0 |
| `npm test` in `ymp-bridges/claude` | 0; 13 tests passed, 0 failed |
| `git diff HEAD --check` | 0 |

The client walk negotiated the declared protocol and discovered the actual schemas.
It completed a mock team run and read the actual `greeting.txt`, accepted task states,
accepted/unconfirmed outcomes and evidence records. It checked invocation counts
before/after inspection and verified every stored invocation used fixed `mock/low`.
It also exercised duplicate starts, changed-payload conflicts, budget increases,
explicit cancellation/resume, project/client scope denial, stale knowledge versions,
graceful disconnect/reopen, SIGKILL with preserved spend and explicit recovery,
internal `--socket` bridge dispatch, malformed JSON/batches/methods, duplicate RPC IDs,
unknown tools and oversized frames. Stdout was consumed as protocol throughout.

Twenty-eight fixture knowledge entries each carried roughly 30,000 content characters.
The public client walked every cursor without duplicate IDs, observed 1,024-character
content projections and full-source versions, and resolved an exact version while
rejecting a stale one. The largest observed MCP response was **73,149 bytes**, including
both textual and structured content. Default evidence replies contain no `bytes`
properties. The internal socket memory test walks the same shared projection and
continuation over 28 large candidate rows; the 25-row reply stays below 52 KiB.

## Failing controls

The same real-client command ran against five deliberately broken builds. Every
control built successfully and the client command exited **1**; source was restored
between controls and the restored executable was rebuilt. Captured failure tails are
in [`failing-controls.json`](../mcp/failing-controls.json).

| Removed behavior | Observed distinguishing failure |
| --- | --- |
| Read-only launch execution guard | A denied start returned `isError: false` and a running handle; the client rejected it. |
| Session project check | The foreign-project session inspection returned saved state instead of an error. |
| Reused-key payload fingerprint check | Changed execution arguments returned the existing handle instead of `request_conflict`. |
| Public cancellation signal | The cancelled request finished `completed` instead of `paused`. |
| Knowledge content projection | The first oversized entry lacked its required truncation marker/content bound. |

These controls discriminate the facade's action/project boundary, durable request
conflict behavior, observable cancellation and tool-result projection. They do not
establish native-provider token guarantees, OS containment or model quality. The
unchanged internal grant-revocation tests cover assignment capability lifetime.

## Limits and review boundary

The public v1 API returns lossy metadata with explicit truncation; it does not export
full truncated strings or captured bytes. Database candidate loading can still scan
and deserialize more data than a response returns. Pagination is not a frozen query
snapshot. Neither an 8,000-character automatic memory prompt allowance nor the 48 KiB
projected page budget is described as a native model context/token guarantee.

A facade owns one run at a time. Cancellation does not roll back selected-directory
writes. A replacement process reports nonterminal foreign-process liveness as unknown
and requires explicit resume under a new operation ID. The request-claim/session-create
crash gap can leave a handle without a session, which retries do not silently execute.
Objective acceptance contracts are preserved on resume, but this v1 start tool does
not install new objective contracts. Qualitative accepted results remain unconfirmed.

Real model quality/cost and final combined-release interoperability remain YMP-121
work. Independent review is required before integration; no acceptance verdict is
claimed by this author report.
