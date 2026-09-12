import { createInterface } from "node:readline";
import { nativeSettings, validateSettings, validateReportedModel, modelCatalog, type Settings } from "./settings.js";
import { UsageTracker } from "./usage.js";
import { query, type Options, type Query, type SDKUserMessage } from "@anthropic-ai/claude-agent-sdk";

interface Request {
  id: number;
  method: string;
  params: {
    profile: { model?: string; instructions: string };
    settings?: Settings;
    provider: { command: string };
    cwd: string;
    prompt: string;
    read_only: boolean;
    resume?: string;
    mcp?: { command: string; args: string[]; token: string };
  };
}

function send(value: unknown): void { process.stdout.write(`${JSON.stringify(value)}\n`); }
let active: Query | undefined;
let running = false;

async function execute(request: Request): Promise<void> {
  if (request.method !== "run" && request.method !== "capabilities") throw new Error("Unknown method");
  if (running) throw new Error("Only one turn may run per bridge");
  running = true;
  const p = request.params;
  const env = { ...process.env };
  delete env.CLAUDECODE;
  delete env.CLAUDE_CODE_ENTRYPOINT;
  const options: Options = {
    cwd: p.cwd,
    pathToClaudeCodeExecutable: p.provider.command,
    settingSources: ["user", "project", "local"],
    systemPrompt: { type: "preset", preset: "claude_code", append: `${p.profile.instructions}\nAll responses, documentation, comments, and artifacts in ymp must be in English.` },
    // Native plan mode prohibits even authorized team-chat tools. Limit the
    // filesystem tool surface instead, while allowing the local coordination API.
    permissionMode: p.read_only ? "default" : "bypassPermissions",
    ...(p.read_only ? { tools: ["Read", "Glob", "Grep"] } : {}),
    allowDangerouslySkipPermissions: true,
    includePartialMessages: true,
    persistSession: true,
    env,
    ...nativeSettings(p.settings ?? {}),
    ...(p.resume ? { resume: p.resume } : {}),
    ...(p.mcp ? { mcpServers: { ymp: { type: "stdio" as const, command: p.mcp.command, args: p.mcp.args, env: { YMP_MCP_TOKEN: p.mcp.token } } } } : {}),
    // Team tools are allowed even during read-only planning; they cannot edit files.
    allowedTools: ["mcp__ymp"],
    canUseTool: async (name, input) => {
      if (name.startsWith("mcp__ymp__") || !p.read_only) {
        return { behavior: "allow", updatedInput: input };
      }
      return { behavior: "deny", message: "This turn permits reading files and communicating with the ymp team only." };
    },
    stderr: () => {},
  };
  let result: { text: string; session_id: string; usage: unknown } | undefined;
  const accounting = new UsageTracker();
  // Initialize metadata before releasing a user message. supportedModels()
  // uses the native control channel; no model prompt is sent during discovery.
  let release!: (run: boolean) => void;
  const ready = new Promise<boolean>(resolve => { release = resolve; });
  async function* messages(): AsyncGenerator<SDKUserMessage> {
    if (await ready) yield { type: "user", session_id: "", message: { role: "user", content: p.prompt }, parent_tool_use_id: null };
  }
  active = query({ prompt: messages(), options });
  send({ method: "execution", params: {
    sent: { model: options.model ?? null, effort: options.effort ?? null, permission_mode: options.permissionMode ?? null },
  } });
  try {
    const models = await active.supportedModels();
    const catalog = modelCatalog(models);
    send({ method: "capabilities", params: catalog });
    validateSettings(p.settings ?? {}, models);
    if (request.method === "capabilities") {
      send({ jsonrpc: "2.0", id: request.id, result: catalog });
      return;
    }
    release(true);
    for await (const event of active) {
      const usage = accounting.ingest(event);
      if (usage) send({ method: "usage", params: usage });
      if (event.type === "system" && event.subtype === "init") {
        send({ method: "session", params: { id: event.session_id } });
        send({ method: "execution", params: {
          sent: { model: options.model ?? null, effort: options.effort ?? null, permission_mode: options.permissionMode ?? null },
          reported: { model: event.model, effort: event.effort ?? null, permission_mode: event.permissionMode },
          native_session_id: event.session_id,
          native_version: event.claude_code_version,
        } });
        validateReportedModel(options.model, event.model, models);
        if (options.effort && event.effort != null && options.effort !== event.effort) throw new Error("Claude reported a different effort than requested");
      }
      if (event.type === "assistant" && event.message.model) {
        send({ method: "execution", params: { reported: { model: event.message.model, effort: null, permission_mode: null } } });
        validateReportedModel(options.model, event.message.model, models);
      }
      if (event.type === "stream_event" && event.event.type === "content_block_delta" && event.event.delta.type === "text_delta") {
        send({ method: "delta", params: { text: event.event.delta.text } });
      }
      if (event.type === "result") {
        if (event.subtype !== "success" || event.is_error) throw new Error(`Claude turn failed (${event.subtype})`);
        result = { text: event.result, session_id: event.session_id, usage: { usage: event.usage, modelUsage: event.modelUsage } };
      }
    }
    if (!result) throw new Error("Claude exited without a final result");
    send({ jsonrpc: "2.0", id: request.id, result });
  } finally {
    release(false);
    active?.close();
    active = undefined;
    running = false;
  }
}

const input = createInterface({ input: process.stdin });
input.on("line", line => {
  let request: Request;
  try { request = JSON.parse(line) as Request; }
  catch { send({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "Invalid JSON" } }); return; }
  void execute(request).catch(error => send({ jsonrpc: "2.0", id: request.id, error: { code: -32000, message: error instanceof Error ? error.message : "Bridge failure" } }));
});
input.on("close", () => { active?.close(); });
process.on("SIGTERM", () => { active?.close(); process.exit(0); });
