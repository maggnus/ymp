# Provider integration

Use native programmatic interfaces with the user's existing installations and authentication. Paseo was inspected as an example of lifecycle pitfalls; ymp runs independently of Paseo and does not import its runtime.

| Provider | Transport | Credential owner |
| --- | --- | --- |
| Codex | Rust JSON-RPC client to `codex app-server --stdio` | Installed Codex |
| Claude | Local JSON-RPC bridge to the official Claude Agent SDK, launching installed `claude` | Installed Claude Code |
| GLM | Rust ACP client to an installed `glm-acp-agent` | GLM agent credential store or its environment |

The Claude bridge uses the CLI's user, project, and local settings. For read-only turns it exposes Read, Glob, and Grep plus authorized ymp MCP tools. Native plan mode is avoided because it forbids team communication as well as file writes. It retains the native Claude Code preset and appends profile/team instructions. The application does not implement a competing Claude tool loop.

Codex team MCP tools have explicit approval configuration for the launched process. This matters because `approvalPolicy=never` can reject a custom MCP call that still requires approval. The override applies only to the `ymp` MCP server and does not edit the user's config file.

GLM discovery resolves an already cached local npm package to its entry point. It does not run `npx -y` or download an agent during startup. The discovered path is written to the application's provider configuration and can be replaced by the user.

Provider availability, native metadata and authentication are different checks. `doctor` locates executables; `catalog --refresh` queries native offerings without inference. `doctor --probe` attempts a real request for every enabled profile matching its optional provider filter; several profiles may use the same provider. Use a separate configuration with only the intended test actors to bound a probe. A successful configuration read or metadata scan does not establish valid authentication or remaining quota.

The backend [agent pool API](agent-pool.md) reads configured actors and stored native offerings without starting discovery. [Native catalog refresh](provider-named-agent-catalog.md) populates the actual pool and preserves the installation's model identifiers, display names, aliases and controls. Provider identity remains separate from selectable agent identity. Unknown or stale native metadata stays explicit; refresh does not add every offering to the starting team or rewrite historical session names/settings.

Model and effort settings are validated and transmitted [per assignment](assignment-settings.md). `ymp capabilities AGENT` queries native metadata without inference; `ymp ask --model/--effort` and `ymp run --assignment-settings` supply concrete choices.

## Local bridge protocol

The host sends one `run` or metadata-only `capabilities` JSON-RPC request per bridge process. The bridge emits `session` and `delta` notifications, then returns `text`, `session_id`, and provider-reported `usage`. Errors reject the request. Stdout carries protocol messages only; diagnostic stderr is drained without persisting raw output that might contain credentials.

The Rust host handles timeouts, cancellation, process ownership, and durable outcomes. The SDK bridge contains no team scheduling, reputation, or storage logic.

## References

- [Codex App Server](https://learn.chatgpt.com/docs/app-server)
- [Codex MCP configuration](https://learn.chatgpt.com/docs/extend/mcp)
- [Claude SDK and CLI features](https://code.claude.com/docs/en/agent-sdk/claude-code-features)
- [GLM ACP agent](https://github.com/stefandevo/glm-acp-agent)
