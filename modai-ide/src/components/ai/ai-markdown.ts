import React from "react";

export function escapeHtml(s: string): string {
  return s
    .split("&").join("&amp;")
    .split("<").join("&lt;")
    .split(">").join("&gt;")
    .split('"').join("&quot;")
    .split("'").join("&#039;");
}

export function renderMarkdownToHtml(md: string): string {
  const lines = md.split("\r\n").join("\n").split("\n");
  let html = "";
  let inCode = false;
  let codeLang = "";
  let codeLines: string[] = [];
  let inList = false;
  let codeBlockId = 0;

  const closeList = () => {
    if (inList) {
      html += "</ul>";
      inList = false;
    }
  };

  for (const rawLine of lines) {
    const line = rawLine ?? "";
    const trimmed = line.trimEnd();

    if (trimmed.startsWith("```")) {
      if (!inCode) {
        closeList();
        inCode = true;
        codeLang = trimmed.slice(3).trim();
        codeLines = [];
      } else {
        const id = `ai-code-block-${codeBlockId++}`;
        const langLabel = codeLang || "text";
        html += `<div class="ai-code-block-wrapper" data-code-id="${id}">`;
        html += `<div class="ai-code-block-header">`;
        html += `<span class="ai-code-block-lang">${escapeHtml(langLabel)}</span>`;
        html += `<button class="ai-code-block-copy" data-copy-target="${id}" title="Copy" type="button">`;
        html += `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`;
        html += `</button>`;
        html += `</div>`;
        html += `<pre class="ai-md-pre"><code id="${id}" class="ai-md-code" data-lang="${escapeHtml(codeLang)}">`;
        html += codeLines.join("\n");
        html += `</code></pre></div>`;
        inCode = false;
        codeLang = "";
        codeLines = [];
      }
      continue;
    }

    if (inCode) {
      codeLines.push(escapeHtml(line));
      continue;
    }

    const headingMatch = trimmed.match(/^(#{1,3})\s+(.*)$/);
    if (headingMatch) {
      closeList();
      const level = headingMatch[1].length;
      const text = escapeHtml(headingMatch[2] ?? "");
      html += `<h${level} class="ai-md-h">${text}</h${level}>`;
      continue;
    }

    const listMatch = trimmed.match(/^-\s+(.*)$/);
    if (listMatch) {
      if (!inList) {
        html += '<ul class="ai-md-ul">';
        inList = true;
      }
      const itemText = escapeHtml(listMatch[1] ?? "")
        .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
        .replace(/`([^`]+?)`/g, '<code class="ai-md-inline">$1</code>');
      html += `<li class="ai-md-li">${itemText}</li>`;
      continue;
    }

    if (!trimmed) {
      closeList();
      html += '<div class="ai-md-spacer"></div>';
      continue;
    }

    closeList();
    const escaped = escapeHtml(trimmed)
      .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
      .replace(/`([^`]+?)`/g, '<code class="ai-md-inline">$1</code>');
    html += `<p class="ai-md-p">${escaped}</p>`;
  }

  closeList();
  if (inCode) {
    const id = `ai-code-block-${codeBlockId++}`;
    const langLabel = codeLang || "text";
    html += `<div class="ai-code-block-wrapper" data-code-id="${id}">`;
    html += `<div class="ai-code-block-header">`;
    html += `<span class="ai-code-block-lang">${escapeHtml(langLabel)}</span>`;
    html += `<button class="ai-code-block-copy" data-copy-target="${id}" title="Copy" type="button">`;
    html += `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2-2v1"/></svg>`;
    html += `</button>`;
    html += `</div>`;
    html += `<pre class="ai-md-pre"><code id="${id}" class="ai-md-code" data-lang="${escapeHtml(codeLang)}">`;
    html += codeLines.join("\n");
    html += `</code></pre></div>`;
  }
  return html;
}

export function splitAnswerAndDiff(text: string | null): { answer: string; diff: string } {
  const src = (text ?? "").split("\r\n").join("\n");
  if (!src.trim()) return { answer: "", diff: "" };

  const fenced = src.match(/```diff\s*\n([\s\S]*?)\n```/i);
  if (fenced && fenced[1]) {
    const diff = fenced[1].trimEnd();
    const answer = src.replace(fenced[0], "").trim();
    return { answer, diff };
  }

  const idx = src.indexOf("diff --git ");
  if (idx >= 0) {
    const answer = src.slice(0, idx).trim();
    const diff = src.slice(idx).trimEnd();
    return { answer, diff };
  }

  return { answer: src.trim(), diff: "" };
}

export function extractFirstCodeBlock(text: string | null): { lang: string; content: string } | null {
  if (!text?.trim()) return null;
  const match = text.match(/```(\w*)\s*\n([\s\S]*?)```/);
  if (!match) return null;
  return { lang: (match[1] ?? "").trim(), content: (match[2] ?? "").trimEnd() };
}

export function suggestMoPathFromModelCode(code: string): string {
  const m = code.match(/\bmodel\s+(\w+)/);
  return m ? `TestLib/${m[1]}.mo` : "TestLib/NewModel.mo";
}

export function parseNewFileDiff(diff: string): { path: string; content: string } | null {
  const lines = diff.split("\n");
  let path: string | null = null;
  let startCollect = false;
  const contentLines: string[] = [];
  for (const line of lines) {
    if (line.startsWith("+++ b/")) {
      if (path) break;
      path = line.slice(6).trim();
      startCollect = true;
      continue;
    }
    if (startCollect && line.startsWith("--- ")) break;
    if (startCollect && line.startsWith("+") && !line.startsWith("+++")) {
      contentLines.push(line.slice(1));
    }
  }
  if (!path || contentLines.length === 0) return null;
  return { path, content: contentLines.join("\n") };
}

export function renderInlineDiff(diff: string): React.ReactElement | null {
  if (!diff.trim()) return null;
  const lines = diff.split("\n");
  return React.createElement(
    "pre",
    { className: "agent-diff-block" },
    lines.map((line: string, idx: number) => {
      let cls = "agent-diff-line";
      if (line.startsWith("+")) cls += " agent-diff-line-added";
      else if (line.startsWith("-")) cls += " agent-diff-line-removed";
      return React.createElement("div", { key: idx, className: cls }, line);
    })
  );
}

export function formatTimestamp(ts: number): string {
  const d = new Date(ts);
  const h = d.getHours();
  const m = d.getMinutes().toString().padStart(2, "0");
  const ampm = h >= 12 ? "PM" : "AM";
  const hour = h % 12 || 12;
  return `${hour}:${m} ${ampm}`;
}

export interface ChatMessage {
  id: number;
  role: "user" | "assistant";
  text: string;
}

export interface ChunkInfo {
  id: number;
  fileId: number;
  lineStart: number;
  lineEnd: number;
  content: string;
  contextLabel: string | null;
  contentHash: string;
  filePath: string;
}
