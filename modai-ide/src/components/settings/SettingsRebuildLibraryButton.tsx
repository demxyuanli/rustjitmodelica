import { useState, useCallback } from "react";
import { RotateCw } from "lucide-react";
import { rebuildComponentLibraryIndex } from "../../api/tauri";
import { t } from "../../i18n";

export function SettingsRebuildLibraryButton() {
  const [status, setStatus] = useState<"idle" | "running" | "done">("idle");
  const handleClick = useCallback(async () => {
    setStatus("running");
    try {
      await rebuildComponentLibraryIndex();
      setStatus("done");
      const tm = setTimeout(() => setStatus("idle"), 2000);
      return () => clearTimeout(tm);
    } catch {
      setStatus("idle");
    }
  }, []);
  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={status === "running"}
      className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 disabled:opacity-40 text-[var(--text-muted)]"
      title={status === "running" ? t("indexRefreshing") : status === "done" ? t("saved") : t("indexRebuild")}
      aria-label={status === "running" ? t("indexRefreshing") : status === "done" ? t("saved") : t("indexRebuild")}
    >
      <RotateCw size={16} className={status === "running" ? "animate-spin" : undefined} />
    </button>
  );
}
