import type { MoTreeEntry } from "../hooks/useProject";
import { LibrariesBrowser } from "./LibrariesBrowser";

interface FileTreeProps {
  projectDir: string | null;
  moTree: MoTreeEntry | null;
  moFiles: string[];
  onOpenProject: () => void;
  onOpenFile: (relativePath: string) => void;
}

export function FileTree({
  projectDir,
  moTree,
  moFiles,
  onOpenProject,
  onOpenFile,
}: FileTreeProps) {
  return (
    <LibrariesBrowser
      projectDir={projectDir}
      moTree={moTree}
      moFiles={moFiles}
      variant="embedded"
      onOpenProject={onOpenProject}
      onOpenFile={onOpenFile}
    />
  );
}
