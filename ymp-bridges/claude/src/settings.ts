import type { EffortLevel, ModelInfo, Options } from "@anthropic-ai/claude-agent-sdk";

export interface Settings { model?: string | null; effort?: string | null }
const levels: readonly string[] = ["low", "medium", "high", "xhigh", "max"];

export function nativeSettings(settings: Settings): Pick<Options, "model" | "effort"> {
  if (settings.effort != null && !levels.includes(settings.effort)) {
    throw new Error(`Unsupported Claude SDK effort: ${settings.effort}`);
  }
  return {
    ...(settings.model != null ? { model: settings.model } : {}),
    ...(settings.effort != null ? { effort: settings.effort as EffortLevel } : {}),
  };
}

// Claude documents [1m] as a context-window modifier, not a model revision.
// Match capabilities by the same model with that one known suffix removed;
// always transmit the user's exact identifier, including its context choice.
export function modelIdentity(id: string): string { return id.replace(/\[1m\]$/, ""); }

export function findModelInfo(id: string, models: ModelInfo[]): ModelInfo | undefined {
  return models.find(m => m.value === id || m.resolvedModel === id)
    ?? models.find(m => modelIdentity(m.value) === modelIdentity(id) || (m.resolvedModel != null && modelIdentity(m.resolvedModel) === modelIdentity(id)));
}

export function validateReportedModel(requested: string | undefined, reported: string, models: ModelInfo[]): void {
  if (!requested) return;
  const selected = findModelInfo(requested, models);
  if (modelIdentity(requested) !== modelIdentity(reported) && (selected?.resolvedModel == null || modelIdentity(selected.resolvedModel) !== modelIdentity(reported))) {
    throw new Error("Claude reported a different model than requested");
  }
}

export function validateSettings(settings: Settings, models: ModelInfo[]): void {
  if (settings.model == null && settings.effort == null) return;
  const model = settings.model == null
    ? models.find(m => m.value === "default")
    : findModelInfo(settings.model, models);
  if (!model) {
    if (settings.effort != null) throw new Error("Claude effort support is unknown for this model; select a model with observed native effort metadata");
    return; // An incomplete picker list does not forbid an explicit model.
  }
  if (settings.effort != null && model.supportedEffortLevels == null && model.supportsEffort !== false) {
    throw new Error("Claude effort support is unknown for this model");
  }
  if (settings.effort != null && !model.supportedEffortLevels?.some(v => v === settings.effort)) {
    throw new Error(`Unsupported effort ${settings.effort} for Claude model ${model.value}`);
  }
}

export function modelCatalog(models: ModelInfo[]): unknown {
  const entries = new Map<string, unknown>();
  for (const m of models) {
    const controls = m.supportedEffortLevels
      ? (m.supportedEffortLevels.length ? [{ id: "effort", values: { kind: "choices", options: m.supportedEffortLevels } }] : [])
      : m.supportsEffort === false ? [] : null;
    for (const id of [m.value, m.resolvedModel]) {
      if (id) entries.set(id, { id, controls });
    }
  }
  return {
    source: { kind: "native_metadata", method: "Claude Query.supportedModels()", observed_at: new Date().toISOString() },
    models_complete: false,
    ...(models.some(m => m.value === "default") ? { default_model: "default" } : {}),
    models: [...entries.values()],
  };
}
