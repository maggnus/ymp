# Native catalog configuration-lock integration finding

2026-09-13 — **R3(6/10) RETURN** for the remaining YMP-127 first-use/update integration. Native naming, pool selection and the R2 execution-version correction remain accepted; the owner application home has not yet been refreshed.

On main source byte-identical to `314dc8ad09e191b29ed78d791933490edd42b1e6`, the required workspace, formatting, Clippy and bridge checks passed. The separately required complete native fixture run failed **101**, with nine tests passing and the Claude SDK fixture failing at its initial `Config::save`:

```text
Another client is saving configuration; retry the edit
Resource temporarily unavailable (os error 35)
```

Command: `cargo test -p ymp-cli --test native_catalog -- --include-ignored`; retained full output: `/tmp/ymp-main-native127-native-fixtures.log`. The failed call is `ymp-cli/tests/native_catalog.rs:503`. The fixture owns a fresh temporary application home. No real model inference or owner configuration update occurred.

The new configuration and scan locks still use raw `File` ownership and implicit close: `ymp-core/src/config.rs:291` and `ymp-providers/src/discovery/catalog.rs:54`. The already accepted StoreLock correction demonstrates why an inherited open description can outlive that apparent owner. Apply the same explicit ownership-release discipline to these new locks, with deterministic inherited-descriptor controls for both paths and preservation of live/replacement-owner exclusion. The particular retaining child is not identified by this failed log; the mechanism must be confirmed by the new controls rather than by repeated successful runs.

Scope is bounded: correct release of configuration/scan ownership, retain concurrent-edit rejection and serialized publication, and rerun the actual ten native fixtures plus required checks. No catalog naming, version formula, historical data or UI change is needed. The parent authorizes a bounded two-return extension for this exact lifecycle condition; prior reports and failures remain historical evidence. The real application home stays untouched until corrected integration is verified.
