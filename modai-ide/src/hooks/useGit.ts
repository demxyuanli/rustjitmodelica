import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface GitStatus {
  branch: string;
  staged: string[];
  modified: string[];
  deleted: string[];
  untracked: string[];
  renamed: { from: string; to: string }[];
}

export interface GitLogEntry {
  hash: string;
  subject: string;
  author: string;
  date: string;
}

export interface GitCommitFile {
  status: string;
  path: string;
}

export function useGit(projectDir: string | null, onRefreshStatus?: () => void) {
  const [isRepo, setIsRepo] = useState(false);
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [commitMessage, setCommitMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [initLoading, setInitLoading] = useState(false);

  const refresh = useCallback(async () => {
    if (!projectDir) {
      setIsRepo(false);
      setStatus(null);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const repo = (await invoke("git_is_repo", { projectDir })) as boolean;
      setIsRepo(repo);
      if (!repo) {
        setStatus(null);
        onRefreshStatus?.();
        return;
      }
      const s = (await invoke("git_status", { projectDir })) as GitStatus;
      setStatus(s);
      onRefreshStatus?.();
    } catch (e) {
      setError(String(e));
      setStatus(null);
    } finally {
      setLoading(false);
    }
  }, [projectDir, onRefreshStatus]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const stage = useCallback(
    async (paths: string[]) => {
      if (!projectDir || paths.length === 0) return;
      try {
        await invoke("git_stage", { projectDir, paths });
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [projectDir, refresh]
  );

  const unstage = useCallback(
    async (paths: string[]) => {
      if (!projectDir || paths.length === 0) return;
      try {
        await invoke("git_unstage", { projectDir, paths });
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [projectDir, refresh]
  );

  const commit = useCallback(async () => {
    if (!projectDir || !commitMessage.trim()) return;
    try {
      await invoke("git_commit", { projectDir, message: commitMessage.trim() });
      setCommitMessage("");
      await refresh();
      onRefreshStatus?.();
    } catch (e) {
      setError(String(e));
    }
  }, [projectDir, commitMessage, refresh, onRefreshStatus]);

  const initRepo = useCallback(async () => {
    if (!projectDir || initLoading) return;
    setInitLoading(true);
    setError(null);
    try {
      await invoke("git_init", { projectDir });
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setInitLoading(false);
    }
  }, [projectDir, initLoading, refresh]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        if (status && commitMessage.trim() && status.staged.length > 0) commit();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [status, commitMessage, commit]);

  return {
    isRepo, status, loading, error,
    commitMessage, setCommitMessage,
    initLoading,
    refresh, stage, unstage, commit, initRepo,
  };
}
