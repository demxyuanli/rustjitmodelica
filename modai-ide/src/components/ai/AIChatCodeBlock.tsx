import { useState, useCallback } from "react";
import { t } from "../../i18n";

interface AIChatCodeBlockProps {
  code: string;
  lang?: string;
}

export function AIChatCodeBlock({ code, lang }: AIChatCodeBlockProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [code]);

  const langLabel = lang || "text";

  return (
    <div className="ai-code-block-wrapper">
      <div className="ai-code-block-header">
        <span className="ai-code-block-lang">{langLabel}</span>
        <button
          type="button"
          className="ai-code-block-copy"
          onClick={handleCopy}
          title={copied ? t("copied") : t("copyMessage")}
        >
          {copied ? (
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="20 6 9 17 4 12" />
            </svg>
          ) : (
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
            </svg>
          )}
        </button>
      </div>
      <pre className="ai-md-pre">
        <code className="ai-md-code" data-lang={lang}>
          {code}
        </code>
      </pre>
    </div>
  );
}
