import type { HookInput, SyncHookJSONOutput } from "@anthropic-ai/claude-agent-sdk";

/** Observe the main invocation's applied effort without exposing hook inputs. */
export class EffortObserver {
  private previous: string | undefined;
  private mismatched = false;

  constructor(private expected: string | undefined, private emit: (level: string) => void) {}

  readonly hook = async (input: HookInput): Promise<SyncHookJSONOutput> => {
    // Native subagents can use other models/efforts; they are not the outer actor.
    if (input.agent_id) return {};
    const level = input.effort?.level;
    if (!level?.trim()) return {};
    if (level !== this.previous) {
      this.previous = level;
      this.emit(level);
    }
    if (this.expected && level !== this.expected) {
      this.mismatched = true;
      return { continue: false, stopReason: "Claude reported a different effort than requested" };
    }
    return {};
  };

  check(): void {
    if (this.mismatched) throw new Error("Claude reported a different effort than requested");
  }
}
