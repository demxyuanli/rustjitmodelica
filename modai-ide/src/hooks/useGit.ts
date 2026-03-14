import { useState, useCallback, useEffect } from "react";
import { gitIsRepo, gitStatus, gitStage, gitUnstage, gitCommit, gitInit } from "../api/tauri";

export interface GitStatus {
  branch?: string;
  staged?: string[];
  modified?: string[];
  deleted?: string[];
  untracked?: string[];
  renamed?: { from: string; to: string }[];
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
      const repo = projectDir ? await gitIsRepo(projectDir) : false;
      setIsRepo(repo);
      if (!repo) {
        setStatus(null);
        onRefreshStatus?.();
        return;
      }
      const s = projectDir ? await gitStatus(projectDir) : null;
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
        await gitStage(projectDir, paths);
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
        await gitUnstage(projectDir, paths);
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
      await gitCommit(projectDir, commitMessage.trim());
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
      await gitInit(projectDir);
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
        if (status && commitMessage.trim() && (status.staged?.length ?? 0) > 0) commit();
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
