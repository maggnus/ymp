import type { Options } from "@anthropic-ai/claude-agent-sdk";

/** maxTurns bounds the native conversation loop, not token volume or retries. */
export function nativeResourceOptions(value?: { max_turns?: number }): Pick<Options, "maxTurns"> {
  if (value?.max_turns == null) return {};
  if (!Number.isSafeInteger(value.max_turns) || value.max_turns <= 0) throw new Error("Invalid native turn limit");
  return { maxTurns: value.max_turns };
}
