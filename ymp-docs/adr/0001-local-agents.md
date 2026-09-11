# ADR 0001: use installed agent runtimes

Status: accepted.

## Context

Users already have agent programs, tools, authentication, and model settings. Replacing these with direct model calls would require implementing tool execution, context handling, and credential management again.

## Decision

Use Codex App Server directly from Rust, the official Claude Agent SDK through a small TypeScript bridge, and ACP for an installed GLM agent. Keep orchestration, storage, and the TUI in Rust. Resolve credentials through each native runtime.

The TypeScript bridge is a deliberate additional dependency: it keeps version-sensitive Claude protocol behavior behind a supported SDK. It does not require a running Paseo instance.

## Consequences

Provider adapters have different native protocols but expose one internal operation contract. Compatibility must be checked against actual local CLI versions. Native session data remains provider-owned; `~/.ymp2` contains ymp metadata and references to those sessions.
