# W1-EVL-04s prelive STOP

## Status and evidentiary boundary

`RUN-003` records the authorized W1-EVL-04s attempt on 2026-09-01. The attempt executed only its
deterministic preflight and stopped before the live command. Its scientific classification is
**`infrastructure-invalid / prelive process-cleanup conformance unproven`**.

No live consumer, model, provider, MCP probe, controller read-back, handle or attestation ran.
`model_ready` remained `false`. The attempt was not retried and cannot be continued under
W1-EVL-04s; any future live attempt requires a separate owner gate.

The frozen one-command rule and preflight STOP are specified by
[W1-EVL-04s](https://github.com/maggnus/ymp/blob/639b23c0781b25ed4650506779fb29bef57dcbbd/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04s.md#L28-L107).
The accepted diagnostic schema and its prohibition on inferred cleanup or provider state are
specified by
[W1-EVL-04r](https://github.com/maggnus/ymp/blob/639b23c0781b25ed4650506779fb29bef57dcbbd/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04r.md#L28-L118).
This record changes no task, code, manifest, protocol, budget, experimental arm, seed or outcome.

## Accepted temporary evidence pair

R2 accepted the following bounded, sanitized pair:

| Artifact | Schema | SHA-256 |
|---|---|---|
| `/private/tmp/ymp-live-probe-diagnostic-v2-evidence.XjzBW4/STOP.json` | `ymp-live-probe-diagnostic-v2-stop-v1` | `b99011120b7a7e8b8fc4d6587028261aa18195ceb1399a05923fc4455c50d694` |
| `/private/tmp/ymp-live-probe-diagnostic-v2-evidence.XjzBW4/STOP-CORRECTION.json` | `ymp-live-probe-diagnostic-v2-stop-correction` version 1 | `82493157368ee65add10ea556e0c0477755719ccd960aebb920b8d204c02de33` |

`STOP-CORRECTION.json` binds
`original_stop_sha256=b99011120b7a7e8b8fc4d6587028261aa18195ceb1399a05923fc4455c50d694`.
Its sanitization class is `bounded_no_raw_output_no_stderr_no_secrets`.
The files are temporary external evidence, not repository artifacts. All scientifically relevant,
non-secret fields are transcribed below so their later deletion does not erase the bounded result.
No raw output, raw stderr, prompt, token, credential or secret is copied.

## Attempt identity and zero-use accounting

The original STOP was recorded at `2026-09-01T22:39:24+08:00` with:

| Field | Exact value |
|---|---|
| Status | `STOP_PRELIVE_PROCESS_CLEANUP_UNPROVEN` |
| Repository | `https://github.com/maggnus/ymp.git` |
| Branch | `paseo/w1-evl-04s-one-live-probe-20260901` |
| Evidence HEAD | [`1431a6d0e9701b9baa0bc3944d1e082697cd293d`](https://github.com/maggnus/ymp/commit/1431a6d0e9701b9baa0bc3944d1e082697cd293d) |
| Namespace | `ymp-live-probe-diagnostic-v2` |
| Original provider request field | `confirmed_not_started` |
| Admission-v2 manifest SHA-256 | `d354f20c8482cd5df7e33fab70dcd267befb621ef09647d430dc40f3924b8ea2` |
| Codex executable SHA-256 | `98491713ffb196061003ee148636e743997cc31d76144ba7c53462269896891d` |

The candidate `ymp` executable digest was `null`; behavioral compatibility and probe-transport
digests were not verified. The exact accounting is:

| Dimension | Observed |
|---|---:|
| Live command count | 0 |
| Model calls | 0 |
| Provider requests | 0 (`not_started`) |
| Input tokens | 0 |
| Cached-input tokens | 0 |
| Output tokens | 0 |
| Reasoning tokens | 0 |
| Monetary cost | 0 |

No runtime, provider, model or MCP event exists. There is no trace, tool event, controller read-back,
handle or attestation, and no readiness transition. These zeros are direct consequences of stopping
before the live command, not missing terminal accounting from a launched model process.

## Deterministic preflight observations

| Command | Exit | Observation |
|---|---:|---|
| `env YMP_DECLARED_MODEL_CALLS=0 CARGO_TARGET_DIR=<disposable>/build cargo test --locked -p ymp-application --lib tool_host_probe::tests::` | 0 | 14 passed; 0 failed; 2 filtered |
| `env YMP_DECLARED_MODEL_CALLS=0 CARGO_TARGET_DIR=<disposable>/build cargo test --locked -p ymp-runtime-api --lib` | 101 | 11 passed; 4 failed; process-termination observation checks |

The failure phase is `deterministic_preflight_process_cleanup`; provider state is `not_started`.
The four failed runtime-API observations reported:

- process observation returned `Operation not permitted (os error 1)`;
- detached-process reparenting could not be confirmed in three checks;
- the initial subsequent cleanup observation also returned `Operation not permitted`.

Application's 14 checks therefore do not close the overall gate. The required product process
termination and reparenting conformance remained unproven, so the live command count stayed zero.

## Original STOP versus accepted correction

The original `process_cleanup_verified=true` field means only the later bounded observation
clarified by `STOP-CORRECTION.json`. It does **not** mean that the four product process-cleanup
checks passed. The accepted corrected facts are:

- `preflight_product_process_cleanup_conformance_proven=false`;
- `failed_termination_test_count=4`;
- the disposable root `ymp-live-probe-diagnostic-v2-root.FpNEwV` was removed after STOP;
- a later exact process scan found no residual matching process at the moment of observation;
- no attestation exists, `model_ready=false`, and retry is forbidden.

The bounded post-STOP observation was:

```text
env YMP_DECLARED_MODEL_CALLS=0 ps -axo pid=,ppid=,command= |
  rg 'ymp-live-probe-diagnostic-v2-root\.FpNEwV|ymp_runtime_api-[0-9a-f]+' |
  rg -v 'rg '
```

It returned no matching process lines. Its scope was limited to command lines matching the exact
disposable-root token or the `ymp_runtime_api` test-binary pattern, excluding the observer. This
supports `post_stop_residual_matching_processes_absent=true`; it cannot establish that the product
termination path or detached-process reparenting behaved correctly during the failed checks.

## Scientific classification and non-claims

The attempt demonstrates a discriminating prelive gate: required deterministic process-cleanup
evidence failed and prevented all live expenditure. It does not establish tool-host reachability or
model/tool ability. With no participant, sender, receiver, board, message, delivery, action,
candidate or task outcome, it supplies no communication, listening, task value, coordination or
self-organization evidence.

The following remain unverified:

- candidate `ymp` executable digest;
- installed Codex behavioral compatibility;
- live probe transport digest;
- before/after byte digest of the real `/Users/maggnus/.ymp`;
- product process-cleanup and detached-process reparenting conformance.

## STOP and future gate

W1-EVL-04s is terminal for this attempt: no retry, live command or provider request may be added
post hoc. Absence of matching processes after STOP does not repair the failed deterministic gate.
A new live task requires a separate owner decision, fresh namespace/root/budget/authorization, and
prelive evidence that every required process-termination observation can execute and pass. Any
unconfirmed cleanup or reparenting result must stop again before a model or provider call.
