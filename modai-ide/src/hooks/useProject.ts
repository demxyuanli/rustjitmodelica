import { useState, useCallback, useEffect } from "react";
import { gitIsRepo, gitStatus, openProjectDir, listMoTree } from "../api/tauri";

export interface MoTreeEntry {
  name: string;
  path?: string;
  children?: MoTreeEntry[];
  class_name?: string;
  extends?: string[];
}

function flattenMoTree(node: MoTreeEntry): string[] {
  if (node.path) return [node.path];
  return (node.children ?? []).flatMap(flattenMoTree);
}

export function pathToModelName(relativePath: string): string {
  return relativePath
    .replace(/\.mo$/i, "")
    .replace(/\\/g, "/")
    .split("/")
    .filter(Boolean)
    .join(".");
}

export function useProject() {
  const [projectDir, setProjectDir] = useState<string | null>(null);
  const [moFiles, setMoFiles] = useState<string[]>([]);
  const [moTree, setMoTree] = useState<MoTreeEntry | null>(null);

  const [gitBranch, setGitBranch] = useState<string | null>(null);
  const [gitStatus, setGitStatus] = useState<{ modified: string[]; staged: string[] } | null>(null);
  const [diffTarget, setDiffTarget] = useState<{
    projectDir: string;
    relativePath: string;
    isStaged: boolean;
    revision?: string;
  } | null>(null);

  useEffect(() => {
    if (!projectDir) {
      setGitBranch(null);
      setGitStatus(null);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const isRepo = await gitIsRepo(projectDir);
        if (cancelled) return;
        if (!isRepo) {
          setGitBranch(null);
          setGitStatus(null);
          return;
        }
        const status = await gitStatus(projectDir);
        if (!cancelled) {
          setGitBranch(status.branch ?? null);
          setGitStatus({ modified: status.modified ?? [], staged: status.staged ?? [] });
        }
      } catch {
        if (!cancelled) {
          setGitBranch(null);
          setGitStatus(null);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [projectDir]);

  const openProject = useCallback(async () => {
    try {
      const dir = await openProjectDir();
      if (!dir) return;
      setProjectDir(dir);
      const tree = (await listMoTree(dir)) as MoTreeEntry;
      setMoTree(tree);
      setMoFiles(flattenMoTree(tree));
    } catch {
      setMoTree(null);
      setMoFiles([]);
    }
  }, []);

  const refreshGitStatus = useCallback(async () => {
    if (!projectDir) return;
    try {
      const isRepo = await gitIsRepo(projectDir);
      if (!isRepo) {
        setGitStatus(null);
        return;
      }
      const status = await gitStatus(projectDir);
      setGitStatus({ modified: status.modified ?? [], staged: status.staged ?? [] });
    } catch {
      setGitStatus(null);
    }
  }, [projectDir]);

  return {
    projectDir, setProjectDir,
    moFiles, moTree,
    gitBranch, gitStatus, setGitStatus,
    diffTarget, setDiffTarget,
    openProject,
    refreshGitStatus,
  };
}
