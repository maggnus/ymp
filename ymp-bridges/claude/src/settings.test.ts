import { test } from "node:test";
import assert from "node:assert/strict";
import { nativeSettings, validateSettings, modelCatalog } from "./settings.js";

test("Claude efforts use advertised model capabilities, including xhigh/max", () => {
  const models = [
    { value: "opus", resolvedModel: "claude-opus-fixture", displayName: "Opus", description: "fixture", supportedEffortLevels: ["low", "xhigh", "max"] as const },
    { value: "small", displayName: "Small", description: "fixture", supportsEffort: false },
  ].map(m => ({ ...m, supportedEffortLevels: m.supportedEffortLevels ? [...m.supportedEffortLevels] : undefined }));
  validateSettings({ model: "claude-opus-fixture", effort: "max" }, models);
  assert.throws(() => validateSettings({ model: "small", effort: "max" }, models));
  assert.throws(() => validateSettings({ model: "opus", effort: "high" }, models));
  assert.throws(() => nativeSettings({ effort: "on" }));
  assert.deepEqual(nativeSettings({}), {});
  assert.deepEqual(nativeSettings({ model: "opus", effort: "xhigh" }), { model: "opus", effort: "xhigh" });
  assert.equal((modelCatalog(models) as { source: { kind: string } }).source.kind, "native_metadata");
});
