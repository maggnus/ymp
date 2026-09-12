// Incremental snapshots for one query() call. API message IDs deduplicate content
// blocks; the final modelUsage map supersedes partial stream/main-loop counters.
export interface Counts {
  input: number | null;
  output: number | null;
  cache_read: number | null;
  cache_write: number | null;
  reasoning: number | null;
}
export interface Snapshot {
  counts: Counts;
  finalized: boolean;
  partial: boolean;
  note: string | null;
  native_total: null;
}
type Data = Record<string, unknown>;
const object = (value: unknown): Data => value !== null && typeof value === "object" && !Array.isArray(value) ? value as Data : {};
const number = (value: unknown): number | null => typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
const empty = (): Counts => ({ input: null, output: null, cache_read: null, cache_write: null, reasoning: null });
const add = (a: number | null, b: number | null): number | null => a === null ? b : b === null ? a : a + b;
function sum(values: Counts[]): Counts {
  const result = empty();
  for (const value of values) for (const key of Object.keys(result) as (keyof Counts)[]) result[key] = add(result[key], value[key]);
  return result;
}
function apiCounts(raw: Data, outputKnown: boolean): Counts {
  const read = number(raw.cache_read_input_tokens);
  const write = number(raw.cache_creation_input_tokens);
  const uncached = number(raw.input_tokens);
  return {
    input: uncached === null ? null : uncached + (read ?? 0) + (write ?? 0),
    output: outputKnown ? number(raw.output_tokens) : null,
    cache_read: read, cache_write: write,
    reasoning: number(object(raw.output_tokens_details).thinking_tokens),
  };
}
function modelCounts(raw: Data): Counts {
  const read = number(raw.cacheReadInputTokens);
  const write = number(raw.cacheCreationInputTokens);
  const uncached = number(raw.inputTokens);
  return {
    input: uncached === null ? null : uncached + (read ?? 0) + (write ?? 0),
    output: number(raw.outputTokens), cache_read: read, cache_write: write, reasoning: null,
  };
}
const total = (counts: Counts): number => (counts.input ?? 0) + (counts.output ?? 0);

export class UsageTracker {
  private responses = new Map<string, { raw: Data; outputKnown: boolean }>();
  private lanes = new Map<string, string>();
  private snapshot: Snapshot | null = null;

  ingest(value: unknown): Snapshot | null {
    const message = object(value);
    if (message.type === "result") return this.finish(message);
    const lane = String(message.session_id ?? "") + ":" + String(message.parent_tool_use_id ?? "");
    let id: string | undefined;
    let raw: Data = {};
    let outputKnown = false;
    if (message.type === "stream_event") {
      const event = object(message.event);
      if (event.type === "message_start") {
        const start = object(event.message);
        if (typeof start.id !== "string") return null;
        id = start.id;
        this.lanes.set(lane, id);
        raw = object(start.usage);
      } else if (event.type === "message_delta") {
        id = this.lanes.get(lane);
        raw = object(event.usage);
        outputKnown = number(raw.output_tokens) !== null;
      } else return null;
    } else if (message.type === "assistant") {
      const assistant = object(message.message);
      if (typeof assistant.id !== "string") return null;
      id = assistant.id;
      raw = { ...object(assistant.usage) };
      // Complete assistant blocks repeat message-start usage. Their output_tokens
      // is a placeholder and must never replace a later message_delta count.
      delete raw.output_tokens;
    } else return null;
    if (!id || Object.keys(raw).length === 0) return null;
    const prior = this.responses.get(id);
    this.responses.set(id, { raw: { ...prior?.raw, ...raw }, outputKnown: outputKnown || (prior?.outputKnown ?? false) });
    this.snapshot = {
      counts: sum([...this.responses.values()].map(v => apiCounts(v.raw, v.outputKnown))),
      finalized: false, partial: true, note: "Streaming usage; final query totals pending", native_total: null,
    };
    return this.snapshot;
  }

  private finish(message: Data): Snapshot | null {
    const models = Object.values(object(message.modelUsage)).map(v => modelCounts(object(v)));
    const full = sum(models);
    if (models.length > 0 && (total(full) > 0 || !this.snapshot || total(this.snapshot.counts) === 0)) {
      const partial = models.some(counts => counts.input === null || counts.output === null);
      this.snapshot = { counts: full, finalized: true, partial, note: partial ? "Some model totals were not reported" : null, native_total: null };
      return this.snapshot;
    }
    const main = apiCounts(object(message.usage), true);
    if (this.snapshot && total(this.snapshot.counts) > total(main)) {
      this.snapshot = { ...this.snapshot, finalized: true, partial: true, note: "Final totals unavailable; retained observed usage" };
      return this.snapshot;
    }
    if (main.input === null && main.output === null) return this.snapshot;
    this.snapshot = { counts: main, finalized: true, partial: true, note: "Main-loop usage only; full model totals unavailable", native_total: null };
    return this.snapshot;
  }
}
