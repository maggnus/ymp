import { test } from "node:test";
import assert from "node:assert/strict";
import { EffortObserver } from "./effort.js";
import type { StopHookInput } from "@anthropic-ai/claude-agent-sdk";

const stopped: StopHookInput = { hook_event_name: "Stop", session_id: "fixture", transcript_path: "fixture", cwd: "fixture", stop_hook_active: false };

test("effort observations ignore missing values and native children and deduplicate repeats", async () => {
  const seen: string[] = [];
  const observer = new EffortObserver(undefined, value => seen.push(value));
  await observer.hook(stopped);
  await observer.hook({ ...stopped, effort: { level: "xhigh" } });
  await observer.hook({ ...stopped, effort: { level: "xhigh" } });
  await observer.hook({ ...stopped, agent_id: "child", effort: { level: "low" } });
  await observer.hook({ ...stopped, effort: { level: "" } });
  assert.deepEqual(seen, ["xhigh"]);
  assert.doesNotThrow(() => observer.check());
});
