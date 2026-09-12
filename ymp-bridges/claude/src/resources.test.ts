import { test } from "node:test";
import assert from "node:assert/strict";
import { nativeResourceOptions } from "./resources.js";

test("native loop limit is passed to SDK without inventing token controls", () => {
  assert.deepEqual(nativeResourceOptions({ max_turns: 2 }), { maxTurns: 2 });
  assert.deepEqual(nativeResourceOptions(), {});
  for (const max_turns of [0, -1, 1.5, Infinity]) assert.throws(() => nativeResourceOptions({ max_turns }));
});
