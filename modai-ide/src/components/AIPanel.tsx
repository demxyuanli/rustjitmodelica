import { t } from "../i18n";

interface AIPanelProps {
  apiKey: string;
  setApiKey: (v: string) => void;
  apiKeySaved: boolean;
  onSaveApiKey: (key: string) => void;
  aiPrompt: string;
  setAiPrompt: (v: string) => void;
  aiLoading: boolean;
  aiResponse: string | null;
  onSend: () => void;
  onInsert: () => void;
  tokenEstimate: number;
  dailyTokenUsed: number;
  dailyTokenLimit: number;
  sendDisabled: boolean;
}

export function AIPanel({
  apiKey,
  setApiKey,
  apiKeySaved,
  onSaveApiKey,
  aiPrompt,
  setAiPrompt,
  aiLoading,
  aiResponse,
  onSend,
  onInsert,
  tokenEstimate,
  dailyTokenUsed,
  dailyTokenLimit,
  sendDisabled,
}: AIPanelProps) {
  return (
    <>
      <div className="text-sm font-medium text-[var(--text-muted)] mb-2">{t("aiCoding")}</div>
      {!apiKeySaved ? (
        <div className="flex gap-2 mb-2">
          <input
            type="password"
            placeholder="DeepSeek API key"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            className="flex-1 bg-[#3c3c3c] border border-gray-600 px-2 py-1 text-sm rounded"
          />
          <button type="button" onClick={() => onSaveApiKey(apiKey)} className="px-2 py-1 bg-primary hover:bg-blue-600 text-sm shrink-0 rounded">
            Save
          </button>
        </div>
      ) : (
        <div className="text-xs text-[var(--text-muted)] mb-2">{t("apiKeySaved")}</div>
      )}
      <div className="text-xs text-[var(--text-muted)] mb-1">
        {t("tokenEstimate")}: {tokenEstimate} &middot; {t("dailyUsed")}: {dailyTokenUsed} / {dailyTokenLimit}
      </div>
      <textarea
        placeholder="e.g. Generate a Modelica model: first-order system with time constant 1"
        value={aiPrompt}
        onChange={(e) => setAiPrompt(e.target.value)}
        className="w-full h-20 bg-[#3c3c3c] border border-gray-600 px-2 py-1 text-sm resize-none mb-2 rounded"
        rows={3}
      />
      <div className="flex gap-2 mb-2">
        <button type="button" onClick={onSend} disabled={sendDisabled} className="px-2 py-1 bg-primary hover:bg-blue-600 text-sm disabled:opacity-50 rounded">
          {aiLoading ? "..." : "Send"}
        </button>
        {aiResponse && (
          <button type="button" onClick={onInsert} className="px-2 py-1 bg-gray-600 hover:bg-gray-500 text-sm rounded">
            Insert
          </button>
        )}
      </div>
      {aiResponse && (
        <pre className="flex-1 min-h-0 overflow-auto text-xs bg-[#1e1e1e] p-2 whitespace-pre-wrap border border-gray-700 rounded">
          {aiResponse}
        </pre>
      )}
    </>
  );
}
