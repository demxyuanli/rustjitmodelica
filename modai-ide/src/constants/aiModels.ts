export const BUILTIN_AI_MODELS: { id: string; label: string; provider: "deepseek" | "grok" | "ollama" }[] = [
  { id: "deepseek-chat", label: "deepseek-chat", provider: "deepseek" },
  { id: "deepseek-coder", label: "deepseek-coder", provider: "deepseek" },
  { id: "grok/grok-2", label: "grok-2", provider: "grok" },
  { id: "grok/grok-4-0709", label: "grok-4-0709", provider: "grok" },
  { id: "grok/grok-code-fast-1", label: "grok-code-fast-1", provider: "grok" },
  { id: "ollama/llama3.2", label: "llama3.2", provider: "ollama" },
  { id: "ollama/codellama", label: "codellama", provider: "ollama" },
  { id: "ollama/qwen2.5-coder", label: "qwen2.5-coder", provider: "ollama" },
  { id: "ollama/deepseek-r1", label: "deepseek-r1 (Ollama)", provider: "ollama" },
];

export function filterEnabledModels(
  modelIdsEnabled: string[] | null | undefined
): { id: string; label: string; provider: "deepseek" | "grok" | "ollama" }[] {
  if (!modelIdsEnabled || modelIdsEnabled.length === 0) return BUILTIN_AI_MODELS;
  const set = new Set(modelIdsEnabled);
  return BUILTIN_AI_MODELS.filter((m) => set.has(m.id));
}
