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


test("native labels and resolved aliases survive without duplicate selectable rows", () => {
  const catalog = modelCatalog([
    { value: "default", resolvedModel: "native-long-version", displayName: "Native Default · distinctive", description: "fixture", supportedEffortLevels: ["low", "max"] },
    { value: "second", displayName: "Second/Exact Name", description: "fixture", supportsEffort: false },
  ]) as any;
  assert.equal(catalog.default_model, "default");
  assert.equal(catalog.models.length, 2);
  assert.deepEqual(catalog.models[0], {
    id: "default", display_name: "Native Default · distinctive", resolved_model: "native-long-version",
    controls: [{ id: "effort", values: { kind: "choices", options: ["low", "max"] } }],
  });
  assert.equal(catalog.models[1].display_name, "Second/Exact Name");
});
