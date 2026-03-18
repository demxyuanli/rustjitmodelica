import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import { createPortal } from "react-dom";
import { t } from "../../i18n";
import { AppIcon } from "../Icon";
import type { AgentMode } from "../../hooks/useAI";
import { getAgentModeLabel } from "./AIChatHeader";
import { filterEnabledModels } from "../../constants/aiModels";

interface AIChatInputProps {
  aiPrompt: string;
  setAiPrompt: (v: string) => void;
  sendDisabled: boolean;
  aiLoading: boolean;
  agentMode: AgentMode;
  setAgentMode?: (m: AgentMode) => void;
  model: string;
  setModel: (m: string) => void;
  setMode: (m: "chat" | "code") => void;
  useContext: boolean;
  setUseContext: (v: boolean) => void;
  contextChunks: Array<{ id: number }>;
  contextLoading: boolean;
  projectDir?: string | null;
  repoRoot?: string | null;
  dailyTokenUsed: number;
  dailyTokenLimit: number;
  onSend: () => void;
  /** If set, only these model IDs are shown in the dropdown. Undefined = all built-in. */
  enabledModelIds?: string[] | null;
}

export function AIChatInput({
  aiPrompt,
  setAiPrompt,
  sendDisabled,
  aiLoading,
  agentMode,
  setAgentMode,
  model,
  setModel,
  setMode,
  useContext,
  setUseContext,
  contextChunks,
  contextLoading,
  projectDir,
  repoRoot,
  dailyTokenUsed,
  dailyTokenLimit,
  onSend,
  enabledModelIds,
}: AIChatInputProps) {
  const modelOptions = useMemo(() => filterEnabledModels(enabledModelIds), [enabledModelIds]);
  const [modeMenuOpen, setModeMenuOpen] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const modeTriggerRef = useRef<HTMLButtonElement | null>(null);
  const [modeMenuRect, setModeMenuRect] = useState<{ bottom: number; left: number } | null>(null);
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const modelTriggerRef = useRef<HTMLButtonElement | null>(null);
  const [modelMenuRect, setModelMenuRect] = useState<{ bottom: number; left: number } | null>(null);

  const adjustInputHeight = useCallback(() => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = "0px";
    const max = 6 * 20;
    const next = Math.min(el.scrollHeight, max);
    el.style.height = `${next}px`;
  }, []);

  const handleInputKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        if (!sendDisabled) {
          onSend();
        }
      }
    },
    [onSend, sendDisabled]
  );

  const handleInputInput = useCallback(() => {
    adjustInputHeight();
  }, [adjustInputHeight]);

  useEffect(() => {
    if (!modeMenuOpen) {
      setModeMenuRect(null);
      return;
    }
    const el = modeTriggerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    setModeMenuRect({ bottom: window.innerHeight - rect.top + 6, left: rect.left });
  }, [modeMenuOpen]);

  useEffect(() => {
    if (!modeMenuOpen) return;
    const onPointer = (e: MouseEvent) => {
      const el = modeTriggerRef.current;
      const target = e.target as Node;
      if (el?.contains(target)) return;
      const menu = document.getElementById("agent-mode-menu");
      if (menu?.contains(target)) return;
      setModeMenuOpen(false);
    };
    window.addEventListener("mousedown", onPointer, true);
    return () => window.removeEventListener("mousedown", onPointer, true);
  }, [modeMenuOpen]);

  useEffect(() => {
    if (!modelMenuOpen) {
      setModelMenuRect(null);
      return;
    }
    const el = modelTriggerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    setModelMenuRect({ bottom: window.innerHeight - rect.top + 6, left: rect.left });
  }, [modelMenuOpen]);

  useEffect(() => {
    if (!modelMenuOpen) return;
    const onPointer = (e: MouseEvent) => {
      const el = modelTriggerRef.current;
      const target = e.target as Node;
      if (el?.contains(target)) return;
      const menu = document.getElementById("agent-model-menu");
      if (menu?.contains(target)) return;
      setModelMenuOpen(false);
    };
    window.addEventListener("mousedown", onPointer, true);
    return () => window.removeEventListener("mousedown", onPointer, true);
  }, [modelMenuOpen]);

  const tokenPercent = dailyTokenLimit > 0 ? Math.min(100, (dailyTokenUsed / dailyTokenLimit) * 100) : 0;
  const tokenBarColor = tokenPercent > 80 ? "var(--danger-text)" : tokenPercent > 50 ? "var(--warning-text)" : "var(--accent)";

  const currentModel = useMemo(() => {
    const found = modelOptions.find((o) => o.id === model);
    return found ?? modelOptions[0] ?? { id: "deepseek-chat", label: "deepseek-chat", provider: "deepseek" as const };
  }, [modelOptions, model]);

  useEffect(() => {
    if (modelOptions.length === 0) return;
    if (!modelOptions.some((o) => o.id === model)) setModel(modelOptions[0].id);
  }, [modelOptions, model, setModel]);

  return (
    <div className="agent-input-shell">
      <div className="agent-input-textarea-wrap">
        <textarea
          ref={inputRef}
          placeholder={t("aiInputPlaceholder")}
          value={aiPrompt}
          onChange={(e) => setAiPrompt(e.target.value)}
          onKeyDown={handleInputKeyDown}
          onInput={handleInputInput}
          className="w-full bg-transparent text-sm resize-none text-[var(--text)] placeholder:text-[var(--text-muted)] outline-none agent-input-textarea"
          rows={1}
        />
      </div>

      <div className="agent-input-hint">
        {t("enterToSend")}
      </div>

      <div className="agent-input-toolbar">
        <div className="flex items-center gap-2">
          <div className="relative">
            <button
              ref={modeTriggerRef}
              type="button"
              onClick={() => setModeMenuOpen((v) => !v)}
              className="agent-input-mode-btn"
            >
              <span>{getAgentModeLabel(agentMode)}</span>
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="6 9 12 15 18 9" />
              </svg>
            </button>
            {modeMenuOpen &&
              modeMenuRect &&
              createPortal(
                <div
                  id="agent-mode-menu"
                  className="agent-mode-menu-portal"
                  style={{
                    position: "fixed",
                    bottom: modeMenuRect.bottom,
                    left: modeMenuRect.left,
                    zIndex: 10000,
                  }}
                >
                  {(["explain", "edit-selection", "edit-file", "generate", "iterate"] as const).map((m) => (
                    <button
                      key={m}
                      type="button"
                      className={`agent-mode-menu-item ${m === agentMode ? "agent-mode-menu-item-active" : ""}`}
                      onClick={() => {
                        if (setAgentMode) {
                          setAgentMode(m);
                        } else {
                          setMode(m === "explain" ? "chat" : "code");
                        }
                        setModeMenuOpen(false);
                      }}
                    >
                      {getAgentModeLabel(m)}
                    </button>
                  ))}
                </div>,
                document.body
              )}
          </div>
          <div className="relative">
            <button
              ref={modelTriggerRef}
              type="button"
              onClick={() => setModelMenuOpen((v) => !v)}
              className="agent-input-mode-btn"
              title={currentModel.label}
            >
              <span className="truncate max-w-[140px]">{currentModel.label}</span>
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="6 9 12 15 18 9" />
              </svg>
            </button>
            {modelMenuOpen &&
              modelMenuRect &&
              createPortal(
                <div
                  id="agent-model-menu"
                  className="agent-mode-menu-portal agent-model-menu-portal"
                  style={{
                    position: "fixed",
                    bottom: modelMenuRect.bottom,
                    left: modelMenuRect.left,
                    zIndex: 10000,
                  }}
                >
                  {([
                    ["deepseek", t("aiModelProviderDeepSeek")],
                    ["grok", t("aiModelProviderGrok")],
                    ["ollama", t("aiModelProviderOllama")],
                  ] as const).map(([provider, label]) => {
                    const items = modelOptions.filter((m) => m.provider === provider);
                    if (items.length === 0) return null;
                    return (
                      <div key={provider}>
                        <div className="agent-menu-section-label">{label}</div>
                        {items.map((m) => (
                          <button
                            key={m.id}
                            type="button"
                            className={`agent-mode-menu-item ${m.id === model ? "agent-mode-menu-item-active" : ""}`}
                            onClick={() => {
                              setModel(m.id);
                              setModelMenuOpen(false);
                            }}
                          >
                            {m.label}
                          </button>
                        ))}
                      </div>
                    );
                  })}
                </div>,
                document.body
              )}
          </div>
        </div>
        <div className="flex items-center gap-2">
          {(projectDir || repoRoot) && (
            <button
              type="button"
              onClick={() => setUseContext(!useContext)}
              className={`agent-ctx-toggle ${useContext ? "agent-ctx-toggle-on" : ""}`}
              title={t("useIndexContext")}
            >
              <AppIcon name="index" aria-hidden="true" className="w-3 h-3" />
              {contextChunks.length > 0 && (
                <span className="agent-ctx-count">{contextChunks.length}</span>
              )}
              {contextLoading && <span className="agent-ctx-loading" />}
            </button>
          )}
          <button
            type="button"
            onClick={onSend}
            disabled={sendDisabled}
            className="agent-send-btn"
            title={t("run")}
          >
            {aiLoading ? (
              <AppIcon name="spinner" className="w-4 h-4 animate-spin" />
            ) : (
              <AppIcon name="send" className="w-4 h-4" />
            )}
          </button>
        </div>
      </div>

      {dailyTokenLimit > 0 && (
        <div className="agent-token-bar-wrap">
          <div
            className="agent-token-bar"
            style={{ width: `${tokenPercent}%`, backgroundColor: tokenBarColor }}
          />
        </div>
      )}
    </div>
  );
}
