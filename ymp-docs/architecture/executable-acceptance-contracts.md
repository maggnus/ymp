# Executable access to trusted acceptance contracts

YMP-125 exposes the existing typed acceptance contracts through `acceptance_contracts` in the selected application home's `config.toml`. CLI and TUI both pass this configuration to the same runtime. The runtime captures the complete contract set with the new session in one storage transaction, before any model invocation. It uses the existing checker, result-version, independent-review, reputation and knowledge interfaces.

## Activation and configuration

The configured set applies to **every new team session** started with that configuration: `ymp run`, the TUI, `ymp demo` / `ymp demo --tui`, and execution-enabled [public MCP](public-mcp.md) starts. It also applies when a conversation explicitly starts a distinct task in a new session. Use a dedicated `--home` configuration when checks belong to one project or workflow; no project file is discovered or trusted automatically. Public MCP fixes its project and configuration at process launch; tool arguments cannot install contracts, and edits to configuration require restarting the facade. `ask`, diagnostics and read-only inspection do not create team acceptance authority.

Append this typed contract to `config.toml` to check the built-in greeting demo. The byte array is UTF-8 `Hello from ymp` followed by a newline.

```toml
[[acceptance_contracts]]
task_title = "Create a greeting"
artifacts = ["greeting.txt"]
inputs = []

[[acceptance_contracts.criteria]]
id = "greeting-content"
description = "The greeting file contains exactly the requested greeting"

[[acceptance_contracts.checks]]
id = "exact-greeting-v1"
criterion_ids = ["greeting-content"]

[acceptance_contracts.checks.assertion]
kind = "exact_bytes"
artifact = "greeting.txt"
expected = [72, 101, 108, 108, 111, 32, 102, 114, 111, 109, 32, 121, 109, 112, 10]
```

Run `ymp --home /path/to/demo-state -C /path/to/project demo` after adding the contract to that home's existing configuration. This demonstration uses deterministic mock agents and makes no model requests. `ymp trace SESSION_ID`, `ymp reputation`, and `ymp -C /path/to/project memory greeting` expose the shared records.

The existing assertion variants remain `exact_bytes`, `matches_input` (declared artifact and input paths), and `command` (an absolute program path, argument array and absolute `verifier_files`). Evidence paths are relative to the selected working directory and must remain within it. Inputs must exist at capture. Command arguments are passed directly; `{workdir}` expands to the selected directory. The user declares the command's check meaning and all verifier dependencies. Verifier files and the program are pinned by their initial digests. No model command can install a trusted check. Unknown fields within contracts, criteria, checks and assertions are rejected.

## Binding and resumption

The runtime sends only target titles, criterion IDs/descriptions and declared artifact/input paths to agents. The projection contains no assertions, expected byte arrays, reference file contents or verifier code. Planning and every revision must include exactly one task for each exact target title. Missing or ambiguous matches cannot commit a plan or reach production. Saved task bindings are revalidated before resumed inspection and at scheduling boundaries. Duplicate contract targets and invalid captures are rejected before session creation.

This context filtering is not filesystem isolation: agents execute directly in the selected directory under the MVP workspace policy. Keep authoritative configuration and verifier code outside the production directory. Native agents retain their installed access, and runtime file-version checks prevent changed inputs or verifiers from becoming fresh confirmation authority.

On resume, an explicitly configured contract set must equal the captured definitions, including criteria, paths, assertions and check bindings. Its target order may differ. A changed set, an explicit empty array replacing existing checks, or new contracts for a formerly qualitative session are rejected before another invocation. Omitting `acceptance_contracts` means use the existing capture; it does not erase the session's obligations. The runtime never rereads input bytes or verifier code to replace that initial capture. To use revised criteria, start a new session.

With no contracts, or with incomplete objective coverage, independent acceptance remains unconfirmed and produces no competence credit. Failing applicable checks override approving agent reviews. File drift preserves historical evidence and prevents current confirmed delivery or supported retrieval according to the existing confirmation rules. Confirmation retains its existing meaning; this feature adds no automatic interpretation of natural-language checks.

## Verification

The real executable integration tests in `ymp-cli/tests/acceptance_contracts.rs` use temporary application homes and the built-in mock provider, pinned to low effort for `run`. They prove confirmed output, one qualified observation, supported outcome retrieval in a later session, absent/partial/failing contracts, missing/duplicate targets, immutable resume, and demo configuration forwarding. Runtime tests exercise sanitized prompts at the provider boundary, omitted/ambiguous initial and revised plans, changed saved plan bindings, changed inputs/artifacts/verifiers, and capture-only resume. Storage tests force a failure in the second contract and verify rollback of the entire session capture.

The positive executable test failed on the starting revision with `Unconfirmed` instead of `Confirmed`. Three additional controls remove binding validation, resume immutability and requirements context one at a time; each causes its corresponding test to fail. Compact results are recorded in [executable-contract-ingress.json](../research/evidence/executable-contract-ingress.json).

Evidence and task acceptance are tracked in [YMP-125](../tasks/README.md#ymp-125). The original independent audit is retained in [executable-confirmation-gap.json](../research/evidence/executable-confirmation-gap.json). These mock/scripted results establish executable integration and authority controls, not model quality improvement. Existing CLI/TUI views consume the shared records; this change adds no TUI behavior.
