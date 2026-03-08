import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DiffEditor } from "@monaco-editor/react";
import { parseDiff, Diff, Hunk, type FileData } from "react-diff-view";
import "react-diff-view/style/index.css";
import { t } from "../i18n";

export interface DiffTarget {
  projectDir: string;
  relativePath: string;
  isStaged: boolean;
  revision?: string;
}

interface DiffViewProps {
  diffTarget: DiffTarget | null;
  currentFileContent: string | null;
  currentFilePath: string | null;
  onClose: () => void;
  onOpenInEditor?: (relativePath: string) => void;
}

type ViewType = "split" | "unified";

function FileDiffBlock({
  file,
  viewType,
}: {
  file: FileData;
  viewType: ViewType;
}) {
  return (
    <Diff
      key={`${file.oldRevision}-${file.newRevision}`}
      viewType={viewType}
      diffType={file.type}
      hunks={file.hunks}
    >
      {(hunks) => hunks.map((hunk) => <Hunk key={hunk.content} hunk={hunk} />)}
    </Diff>
  );
}

export function DiffView({
  diffTarget,
  currentFileContent,
  currentFilePath,
  onClose,
  onOpenInEditor,
}: DiffViewProps) {
  const [diffText, setDiffText] = useState<string | null>(null);
  const [original, setOriginal] = useState("");
  const [modified, setModified] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [viewType, setViewType] = useState<ViewType>("split");
  const [useMonacoFallback, setUseMonacoFallback] = useState(false);

  const isCurrentFile =
    diffTarget &&
    currentFilePath &&
    (currentFilePath === diffTarget.relativePath ||
      currentFilePath.replace(/\\/g, "/") === diffTarget.relativePath.replace(/\\/g, "/"));

  const load = useCallback(async () => {
    if (!diffTarget) {
      setDiffText(null);
      setOriginal("");
      setModified("");
      setError(null);
      setUseMonacoFallback(false);
      return;
    }
    setLoading(true);
    setError(null);
    const { projectDir, relativePath, isStaged, revision } = diffTarget;
    const pathForGit = relativePath.replace(/\\/g, "/");
    try {
      let diffOut: string;
      if (isStaged) {
        diffOut = (await invoke("git_diff_file_staged", {
          projectDir,
          relativePath: pathForGit,
        })) as string;
      } else {
        const base = revision ?? "HEAD";
        diffOut = (await invoke("git_diff_file", {
          projectDir,
          relativePath: pathForGit,
          base,
        })) as string;
      }
      const trimmed = diffOut.trim();
      if (trimmed) {
        setDiffText(trimmed);
        setUseMonacoFallback(false);
      } else {
        let orig = "";
        const rev = revision ?? "HEAD";
        try {
          orig = (await invoke("git_show_file", {
            projectDir,
            revision: rev,
            relativePath: pathForGit,
          })) as string;
        } catch {
          orig = "";
        }
        let mod = "";
        if (isCurrentFile && currentFileContent !== null) {
          mod = currentFileContent;
        } else {
          try {
            mod = (await invoke("read_project_file", {
              projectDir,
              relativePath: pathForGit,
            })) as string;
          } catch {
            mod = "";
          }
        }
        setOriginal(orig);
        setModified(mod);
        setDiffText(null);
        setUseMonacoFallback(true);
      }
    } catch (e) {
      setError(String(e));
      setDiffText(null);
      setOriginal("");
      setModified("");
    } finally {
      setLoading(false);
    }
  }, [diffTarget, isCurrentFile, currentFileContent]);

  useEffect(() => {
    load();
  }, [load]);

  if (!diffTarget) {
    return (
      <div className="flex flex-col h-full items-center justify-center text-sm text-[var(--text-muted)] p-4">
        {t("viewDiff")}
        <span className="text-xs mt-1">{t("openProjectFirst")}</span>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="flex flex-col h-full items-center justify-center text-sm text-[var(--text-muted)]">
        {t("running")}
      </div>
    );
  }

  const header = (
    <div className="shrink-0 flex items-center justify-between gap-2 px-2 py-1.5 border-b border-border">
      <span className="text-xs text-[var(--text-muted)] truncate flex-1 min-w-0" title={diffTarget.relativePath}>
        {diffTarget.relativePath}
      </span>
      {onOpenInEditor && (
        <button
          type="button"
          className="shrink-0 text-xs px-1.5 py-0.5 rounded text-[var(--text-muted)] hover:bg-white/10 hover:text-[var(--text)]"
          onClick={() => onOpenInEditor(diffTarget.relativePath)}
          title={t("openInEditor")}
        >
          {t("openInEditor")}
        </button>
      )}
      {!useMonacoFallback && (
        <div className="shrink-0 flex items-center gap-1">
          <button
            type="button"
            className={`text-xs px-1.5 py-0.5 rounded ${viewType === "split" ? "bg-white/15 text-[var(--text)]" : "text-[var(--text-muted)] hover:bg-white/5"}`}
            onClick={() => setViewType("split")}
          >
            {t("diffSplit")}
          </button>
          <button
            type="button"
            className={`text-xs px-1.5 py-0.5 rounded ${viewType === "unified" ? "bg-white/15 text-[var(--text)]" : "text-[var(--text-muted)] hover:bg-white/5"}`}
            onClick={() => setViewType("unified")}
          >
            {t("diffUnified")}
          </button>
        </div>
      )}
      <button
        type="button"
        className="shrink-0 text-xs text-[var(--text-muted)] hover:text-[var(--text)]"
        onClick={onClose}
      >
        ×
      </button>
    </div>
  );

  if (error) {
    return (
      <>
        {header}
        <div className="shrink-0 px-2 py-1 text-xs text-red-400">{error}</div>
      </>
    );
  }

  if (useMonacoFallback) {
    return (
      <div className="flex flex-col h-full min-h-0">
        {header}
        <div className="flex-1 min-h-0">
          <DiffEditor
            height="100%"
            original={original}
            modified={modified}
            language="plaintext"
            theme="vs-dark"
            options={{
              readOnly: true,
              renderSideBySide: true,
              minimap: { enabled: false },
              fontSize: 12,
            }}
          />
        </div>
      </div>
    );
  }

  const files = parseDiff(diffText!, { nearbySequences: "zip" });

  return (
    <div className="flex flex-col h-full min-h-0 diff-view-container">
      {header}
      <div className="flex-1 min-h-0 overflow-auto p-2 scroll-vscode">
        <div className="diff" style={{ fontFamily: "var(--font-mono, Consolas, monospace)", fontSize: 12 }}>
          {files.map((file, i) => (
            <FileDiffBlock key={i} file={file} viewType={viewType} />
          ))}
        </div>
      </div>
    </div>
  );
}
