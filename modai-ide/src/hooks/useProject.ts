import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

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
        const isRepo = (await invoke("git_is_repo", { projectDir })) as boolean;
        if (cancelled) return;
        if (!isRepo) {
          setGitBranch(null);
          setGitStatus(null);
          return;
        }
        const status = (await invoke("git_status", { projectDir })) as {
          branch: string;
          modified: string[];
          staged: string[];
        };
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
      const dir = (await invoke("open_project_dir")) as string | null;
      if (!dir) return;
      setProjectDir(dir);
      const tree = (await invoke("list_mo_tree", { projectDir: dir })) as MoTreeEntry;
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
      const isRepo = (await invoke("git_is_repo", { projectDir })) as boolean;
      if (!isRepo) {
        setGitStatus(null);
        return;
      }
      const status = (await invoke("git_status", { projectDir })) as {
        modified: string[];
        staged: string[];
      };
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
