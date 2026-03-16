import { useState, useCallback } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { t } from "../i18n";
import type { MoTreeEntry } from "../hooks/useProject";
import { LibrariesBrowser } from "./LibrariesBrowser";
import { ContextMenu } from "./ContextMenu";

interface FileTreeProps {
  projectDir: string | null;
  moTree: MoTreeEntry | null;
  moFiles: string[];
  onOpenProject: () => void;
  onOpenFile: (relativePath: string) => void;
  recentProjects?: string[];
  onOpenRecentProject?: (path: string) => void;
  onNewModel?: () => void;
}

export function FileTree({
  projectDir,
  moTree,
  moFiles,
  onOpenProject,
  onOpenFile,
  recentProjects,
  onOpenRecentProject,
  onNewModel,
}: FileTreeProps) {
  const [rootMenuVisible, setRootMenuVisible] = useState(false);
  const [rootMenuPosition, setRootMenuPosition] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [nodeMenuVisible, setNodeMenuVisible] = useState(false);
  const [nodeMenuPosition, setNodeMenuPosition] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [nodeInfo, setNodeInfo] = useState<{ kind: "file" | "folder"; path?: string; name: string } | null>(null);

  const handleContextMenu = useCallback((event: React.MouseEvent) => {
    if (event.defaultPrevented) return;
    event.preventDefault();
    setRootMenuPosition({ x: event.clientX, y: event.clientY });
    setRootMenuVisible(true);
  }, []);

  const handleFileContextMenu = useCallback(
    ({ path, name, event }: { path: string; name: string; event: React.MouseEvent }) => {
      event.preventDefault();
      setNodeMenuPosition({ x: event.clientX, y: event.clientY });
      setNodeInfo({ kind: "file", path, name });
      setNodeMenuVisible(true);
    },
    []
  );

  const handleFolderContextMenu = useCallback(
    ({ name, event }: { name: string; event: React.MouseEvent }) => {
      event.preventDefault();
      setNodeMenuPosition({ x: event.clientX, y: event.clientY });
      setNodeInfo({ kind: "folder", name });
      setNodeMenuVisible(true);
    },
    []
  );

  return (
    <div onContextMenu={handleContextMenu}>
      <LibrariesBrowser
        projectDir={projectDir}
        moTree={moTree}
        moFiles={moFiles}
        variant="embedded"
        onOpenProject={onOpenProject}
        onOpenFile={onOpenFile}
        recentProjects={recentProjects}
        onOpenRecentProject={onOpenRecentProject}
        onFileContextMenu={handleFileContextMenu}
        onFolderContextMenu={handleFolderContextMenu}
      />
      <ContextMenu
        visible={rootMenuVisible}
        x={rootMenuPosition.x}
        y={rootMenuPosition.y}
        onClose={() => setRootMenuVisible(false)}
        items={[
          {
            id: "open-project",
            label: t("contextOpenProject"),
            onClick: () => {
              onOpenProject();
            },
          },
          {
            id: "new-model",
            label: t("contextNewModel"),
            disabled: !projectDir || !onNewModel,
            onClick: () => {
              if (!projectDir || !onNewModel) return;
              onNewModel();
            },
          },
          {
            id: "refresh",
            label: t("contextRefresh"),
            disabled: true,
            onClick: () => {
              // placeholder for future file tree refresh
            },
          },
        ]}
      />
      <ContextMenu
        visible={nodeMenuVisible}
        x={nodeMenuPosition.x}
        y={nodeMenuPosition.y}
        onClose={() => setNodeMenuVisible(false)}
        items={
          nodeInfo && nodeInfo.kind === "file" && nodeInfo.path
            ? [
                {
                  id: "open-file",
                  label: t("contextFileOpen"),
                  onClick: () => {
                    onOpenFile(nodeInfo.path!);
                  },
                },
                {
                  id: "reveal",
                  label: t("contextRevealInExplorer"),
                  disabled: !projectDir,
                  onClick: () => {
                    if (!projectDir || !nodeInfo.path) return;
                    const normalizedDir = projectDir.replace(/[/\\]+$/, "");
                    const normalizedRel = nodeInfo.path.replace(/^[/\\]+/, "");
                    const fullPath = `${normalizedDir}\\${normalizedRel}`;
                    void revealItemInDir(fullPath).catch(() => {});
                  },
                },
                {
                  id: "copy-path",
                  label: t("contextCopyPath"),
                  onClick: () => {
                    void navigator.clipboard.writeText(nodeInfo.path!);
                  },
                },
                {
                  id: "copy-model-name",
                  label: t("contextCopyModelName"),
                  disabled: !/\.mo$/i.test(nodeInfo.path),
                  onClick: () => {
                    if (!nodeInfo.path) return;
                    const withoutExt = nodeInfo.path.replace(/\.mo$/i, "");
                    const modelName = withoutExt.replace(/[/\\]+/g, ".");
                    void navigator.clipboard.writeText(modelName);
                  },
                },
              ]
            : nodeInfo && nodeInfo.kind === "folder"
              ? [
                  {
                    id: "copy-folder-name",
                    label: t("contextCopyFolderName"),
                    onClick: () => {
                      void navigator.clipboard.writeText(nodeInfo.name);
                    },
                  },
                ]
              : []
        }
      />
    </div>
  );
}
