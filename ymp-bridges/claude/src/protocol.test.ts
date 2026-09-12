import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm, chmod } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { createInterface } from "node:readline";

async function run(settings: object, method = "run", overrides: object = {}) {
  const cwd = await mkdtemp(join(tmpdir(), "ymp-claude-settings-"));
  const command = resolve("tests/fixtures/claude.py");
  await chmod(command, 0o700);
  const child = spawn(process.execPath, [resolve("dist/index.js")], { stdio: ["pipe", "pipe", "pipe"] });
  const events: Record<string, any>[] = [];
  child.stderr.resume();
  try {
    const result = new Promise<Record<string, any>>((resolve, reject) => {
      child.on("error", reject);
      child.on("exit", code => reject(new Error(`Bridge exited ${code}`)));
      const input = createInterface({ input: child.stdout });
      input.on("line", line => { const value = JSON.parse(line); events.push(value); if (value.id === 1) resolve(value); });
    });
    child.stdin.write(JSON.stringify({ id: 1, method, params: { settings, profile: { model: "stale-profile", instructions: "fixture" }, provider: { command }, cwd, prompt: "offline", read_only: true, ...overrides } }) + "\n");
    const timeout = setTimeout(() => child.kill(), 10_000);
    let response: Record<string, any>;
    try { response = await result; } finally { clearTimeout(timeout); }
    const wire = (await readFile(join(cwd, "native.jsonl"), "utf8")).trim().split("\n").map(l => JSON.parse(l));
    return { response, wire, events };
  } finally { child.kill(); await rm(cwd, { recursive: true, force: true }); }
}

test("physical SDK transport receives assignment settings, and metadata precedes prompt", async () => {
  const { response, wire, events } = await run({ model: "opus", effort: "max" });
  assert.equal(response.result?.text, "done", JSON.stringify(response));
  const argv = wire[0].argv as string[];
  assert.equal(argv[argv.indexOf("--model") + 1], "opus");
  assert.equal(argv[argv.indexOf("--effort") + 1], "max");
  assert.ok(wire.findIndex(q => q.type === "control_request") < wire.findIndex(q => q.type === "user"));
  const reported = events.find(e => e.params?.reported)?.params.reported;
  assert.equal(reported.model, "claude-opus-5"); assert.equal(reported.effort, "max");
});

test("unsupported model effort never releases a user prompt", async () => {
  for (const settings of [{ model: "small", effort: "max" }, { model: "opus", effort: "high" }, { model: "unknown", effort: "max" }, { model: "opus[other]", effort: "max" }]) {
    const { response, wire } = await run(settings);
    assert.ok(response.error); assert.ok(!wire.some(q => q.type === "user"));
  }
});

test("metadata-only discovery never releases a prompt and native defaults remain omitted", async () => {
  const { response, wire } = await run({}, "capabilities");
  assert.equal(response.result.source.kind, "native_metadata");
  assert.ok(!wire.some(q => q.type === "user"));
  assert.ok(!wire[0].argv.includes("--model")); assert.ok(!wire[0].argv.includes("--effort"));
  const defaults = await run({});
  assert.equal(defaults.response.result.text, "done");
  assert.ok(defaults.events.some(e => e.params?.sent?.model === null && e.params?.reported?.model === "claude-opus-5"));
});


test("plain Opus 5 retains its exact version pin against a native 1m catalog row", async () => {
  const { response, wire, events } = await run({ model: "claude-opus-5", effort: "max" });
  assert.equal(response.result?.text, "done", JSON.stringify(response));
  const argv = wire[0].argv as string[];
  assert.equal(argv[argv.indexOf("--model") + 1], "claude-opus-5");
  assert.equal(events.find(e => e.params?.reported)?.params.reported.model, "claude-opus-5");
});

test("an incomplete native list does not forbid an unlisted explicit model", async () => {
  const { response, wire } = await run({ model: "custom-model" });
  assert.equal(response.result?.text, "done", JSON.stringify(response));
  assert.ok(wire.some(q => q.type === "user"));
});


test("native resume receives a fresh MCP credential and read tool restriction each time", async () => {
  for (const token of ["synthetic-assignment-one", "synthetic-assignment-two"]) {
    const { response, wire } = await run({}, "run", {
      resume: "saved-conversation-context",
      mcp: { command: "ymp-fixture", args: ["mcp", "--socket", "current.sock"], token },
    });
    assert.equal(response.result?.text, "done", JSON.stringify(response));
    const argv = wire[0].argv as string[];
    assert.ok(argv.includes("--resume=saved-conversation-context"));
    assert.equal(argv[argv.indexOf("--permission-mode") + 1], "default");
    assert.equal(argv[argv.indexOf("--tools") + 1], "Read,Glob,Grep");
    const config = JSON.parse(argv[argv.indexOf("--mcp-config") + 1]);
    assert.equal(config.mcpServers.ymp.env.YMP_MCP_TOKEN, token);
  }
});
