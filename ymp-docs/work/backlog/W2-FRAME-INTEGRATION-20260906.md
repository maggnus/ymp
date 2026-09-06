# Frame integration observations

Code candidate: [0af2354](https://github.com/maggnus/ymp/commit/0af2354b15c91abc728c82e1cf315be3c9dd2a24). English fixture candidate: [27112d8](https://github.com/maggnus/ymp/commit/27112d85c1be4f48b6c5909882d92ea6d22a400d). Both have scoped acceptance; combined integration remained pending.

1. Full workspace/all-target check in the original macOS temporary-root location failed in five agent-rpc tests with “path must be shorter than SUN_LEN”. The overly long disposable TMPDIR was corrected by creating a fresh short /tmp root; no Rust was changed.
2. Full check in /tmp/yic-m349vc0l passed agent-rpc and subsequent groups, then stopped at five claude_product_path failures. Seven live-provider tests were ignored as declared; no model use was authorized.
3. One exact failing test was run on pre-TUI baseline [7b2d5e9](https://github.com/maggnus/ymp/commit/7b2d5e99803a5867efe2696387d947b12cf4d28d), in /tmp/yib-l_53ac1v. It failed identically with managed_runtime_supervision_failed. This proves that specific failure predates the TUI change; it is not a green full-suite result.

Source diagnosis: the CLI fixture COORDINATED_INIT omits read_board/publish, while runtime-claude validates them in the current coordinated tool catalogue. Child e owns a test-only correction and controlled stale-catalogue rejection. No production gate is weakened to repair a fixture.

Source trees and command/exit logs are retained in the run checkpoint's frame-integration-check.json and frame-baseline-check.json plus their disposable exports. The full suite stopped early; later groups remained unverified and must be covered by the next combined integration after the fixture changes.
