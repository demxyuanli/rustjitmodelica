import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { t } from "../../i18n";
import type { AppSettingsForm } from "./settingsTypes";
import { SettingsRow } from "./settingsPrimitives";

export interface SettingsMslPackSectionProps {
  appSettings: AppSettingsForm;
  onAppSettingsChange: (s: AppSettingsForm) => void;
}

interface MslCacheStatus {
  msl_version: string | null;
  tree_digest: string | null;
  msl_root: string | null;
  pack_dirs: string;
  matched_pack_dir: string | null;
  hotness_path: string;
}

export function SettingsMslPackSection({ appSettings, onAppSettingsChange }: SettingsMslPackSectionProps) {
  const [statusText, setStatusText] = useState<string>("");
  const [busy, setBusy] = useState<string | null>(null);
  const [lastRemote, setLastRemote] = useState<string>("");

  const refresh = useCallback(async () => {
    try {
      const s = await invoke<MslCacheStatus>("msl_cache_status");
      setStatusText(
        [
          s.msl_version ? `MSL ${s.msl_version}` : "MSL version: —",
          s.tree_digest ? `tree ${s.tree_digest}` : "tree: —",
          s.matched_pack_dir ? `pack: ${s.matched_pack_dir}` : "pack: —",
        ].join(" | "),
      );
    } catch (e) {
      setStatusText(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return (
    <section id="settings-group-msl-pack" className="mt-6">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">
        {t("settingsSectionMslPack")}
      </h3>
      <SettingsRow title={t("settingsMslPackManifestUrl")} description={t("settingsMslPackManifestUrlDesc")}>
        <input
          type="text"
          placeholder="https://..."
          value={appSettings.extensions?.mslPackManifestUrl ?? ""}
          onChange={(e) =>
            onAppSettingsChange({
              ...appSettings,
              extensions: { ...(appSettings.extensions ?? {}), mslPackManifestUrl: e.target.value },
            })
          }
          className="min-w-[200px] max-w-[480px] flex-1 bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono"
        />
      </SettingsRow>
      <SettingsRow title={t("settingsMslPackStatus")} description={statusText}>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            className="px-2.5 py-1.5 text-sm rounded border border-border bg-[var(--surface)]"
            onClick={() => void refresh()}
          >
            {t("settingsMslPackRefresh")}
          </button>
          <button
            type="button"
            className="px-2.5 py-1.5 text-sm rounded border border-border bg-[var(--surface)]"
            disabled={busy !== null}
            onClick={async () => {
              setBusy("clear");
              try {
                await invoke("msl_cache_clear");
                await refresh();
              } catch (e) {
                setStatusText(String(e));
              } finally {
                setBusy(null);
              }
            }}
          >
            {t("settingsMslPackClear")}
          </button>
          <button
            type="button"
            className="px-2.5 py-1.5 text-sm rounded border border-border bg-[var(--surface)]"
            disabled={busy !== null}
            onClick={async () => {
              setBusy("check");
              try {
                const j = await invoke<Record<string, unknown>>("msl_cache_check_update");
                setLastRemote(JSON.stringify(j));
                await refresh();
              } catch (e) {
                setLastRemote(String(e));
              } finally {
                setBusy(null);
              }
            }}
          >
            {t("settingsMslPackCheckUpdate")}
          </button>
          <button
            type="button"
            className="px-2.5 py-1.5 text-sm rounded border border-border bg-[var(--surface)]"
            disabled={busy !== null}
            onClick={async () => {
              setBusy("download");
              try {
                const j = await invoke<Record<string, unknown>>("msl_cache_download_update");
                setLastRemote(JSON.stringify(j));
                await refresh();
              } catch (e) {
                setLastRemote(String(e));
              } finally {
                setBusy(null);
              }
            }}
          >
            {t("settingsMslPackDownload")}
          </button>
          <button
            type="button"
            className="px-2.5 py-1.5 text-sm rounded border border-border bg-[var(--surface)]"
            disabled={busy !== null || !(appSettings.extensions?.modelicaStdlibPath ?? "").trim()}
            onClick={async () => {
              const root = (appSettings.extensions?.modelicaStdlibPath ?? "").trim();
              if (!root) return;
              setBusy("rebuild");
              try {
                const p = await invoke<string>("msl_cache_rebuild_local", { mslRoot: root, outName: null });
                setLastRemote(p);
                await refresh();
              } catch (e) {
                setLastRemote(String(e));
              } finally {
                setBusy(null);
              }
            }}
          >
            {t("settingsMslPackRebuildLocal")}
          </button>
        </div>
      </SettingsRow>
      {lastRemote ? (
        <p className="text-xs font-mono text-[var(--text-muted)] mt-2 break-all">{lastRemote}</p>
      ) : null}
      {busy ? <p className="text-xs text-[var(--text-muted)] mt-1">{busy}…</p> : null}
    </section>
  );
}
