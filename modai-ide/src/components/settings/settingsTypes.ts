import type { AiConfig } from "../../api/tauri";

export interface IndexActionState {
  running: boolean;
  action: "refresh" | "rebuild" | null;
  done: number;
  total: number;
}

export type DefaultWorkspace = "modelica" | "component-library" | "compiler-iterate" | "regression";

export interface IndexCacheSettingsForm {
  componentLibraryIndexEnabled?: boolean;
  repoIndexRefreshOnJitLoad?: boolean;
  gitStatusThrottleMs?: number;
}

export interface IndexingSettingsForm {
  indexAutoNewFolders?: boolean;
  indexAutoNewFoldersMaxFiles?: number;
  indexRepoForGrep?: boolean;
}

export interface DependencyGraphSettingsForm {
  fullTimeoutSec?: number;
  autoDowngradeFromFull?: boolean;
  downgradeTarget?: "compact" | "top-level";
  defaultGraphMode?: string;
  preferStructuralFirst?: boolean;
}

export interface ValidationSettingsForm {
  defaultTier?: string;
}

export interface AppSettingsForm {
  storage?: { indexPathPolicy?: string; allowProjectWrites?: boolean };
  resources?: { librarySearchPaths?: string[]; packageCacheDir?: string };
  documentation?: { helpBaseUrl?: string; showWelcomeOnFirstLaunch?: boolean };
  extensions?: { pluginDir?: string; modelicaStdlibPath?: string };
  indexCache?: IndexCacheSettingsForm;
  indexing?: IndexingSettingsForm;
  dependencyGraph?: DependencyGraphSettingsForm;
  validation?: ValidationSettingsForm;
  ai?: AiConfig;
}
