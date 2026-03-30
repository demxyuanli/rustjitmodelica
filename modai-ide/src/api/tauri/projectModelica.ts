import { invoke } from "@tauri-apps/api/core";
import type {
  ComponentLibrary,
  ComponentLibraryTypeQueryResult,
  ComponentTypeInfo,
  ComponentTypeRelationGraph,
  ComponentTypeSource,
  GraphicalDocumentModel,
  InstantiableClass,
  LibrarySuggestion,
} from "../../types";
import { agentDebugLog } from "../../debug/agentDebugLog";


// --- Project / Modelica files ---

export async function openProjectDir(): Promise<string | null> {
  return invoke<string | null>("open_project_dir");
}

export async function reopenProjectDir(path: string): Promise<string> {
  return invoke<string>("reopen_project_dir", { path });
}

export async function pickComponentLibraryFolder(): Promise<string | null> {
  return invoke<string | null>("pick_component_library_folder");
}

export async function pickComponentLibraryFiles(): Promise<string[]> {
  return invoke<string[]>("pick_component_library_files");
}

export async function listMoTree(projectDir: string) {
  return invoke("list_mo_tree", { projectDir });
}

export async function listMoFiles(projectDir: string) {
  return invoke<string[]>("list_mo_files", { projectDir });
}

export async function listInstantiableClasses(projectDir?: string | null): Promise<InstantiableClass[]> {
  return invoke<InstantiableClass[]>("list_instantiable_classes", { projectDir: projectDir ?? undefined });
}

export async function queryComponentLibraryTypes(params: {
  projectDir?: string | null;
  libraryId?: string | null;
  scope?: string | null;
  enabledOnly?: boolean;
  query?: string;
  offset?: number;
  limit?: number;
}): Promise<ComponentLibraryTypeQueryResult> {
  return invoke<ComponentLibraryTypeQueryResult>("query_component_library_types", {
    projectDir: params.projectDir ?? undefined,
    libraryId: params.libraryId ?? undefined,
    scope: params.scope ?? undefined,
    enabledOnly: params.enabledOnly ?? true,
    query: params.query ?? "",
    offset: params.offset ?? 0,
    limit: params.limit ?? 100,
  });
}

export async function listComponentLibraries(projectDir?: string | null): Promise<ComponentLibrary[]> {
  return invoke<ComponentLibrary[]>("list_component_libraries", { projectDir: projectDir ?? undefined });
}

export async function addComponentLibrary(params: {
  projectDir?: string | null;
  scope: string;
  kind: string;
  sourcePath: string;
  displayName?: string;
}): Promise<ComponentLibrary> {
  return invoke<ComponentLibrary>("add_component_library", {
    projectDir: params.projectDir ?? undefined,
    scope: params.scope,
    kind: params.kind,
    sourcePath: params.sourcePath,
    displayName: params.displayName,
  });
}

export async function removeComponentLibrary(params: {
  projectDir?: string | null;
  scope: string;
  libraryId: string;
}): Promise<void> {
  await invoke("remove_component_library", {
    projectDir: params.projectDir ?? undefined,
    scope: params.scope,
    libraryId: params.libraryId,
  });
}

export async function setComponentLibraryEnabled(params: {
  projectDir?: string | null;
  scope: string;
  libraryId: string;
  enabled: boolean;
}): Promise<ComponentLibrary> {
  return invoke<ComponentLibrary>("set_component_library_enabled", {
    projectDir: params.projectDir ?? undefined,
    scope: params.scope,
    libraryId: params.libraryId,
    enabled: params.enabled,
  });
}

export async function installThirdPartyLibraryFromGit(params: {
  projectDir?: string | null;
  scope: string;
  url: string;
  refName?: string | null;
  displayName?: string | null;
}): Promise<ComponentLibrary> {
  return invoke<ComponentLibrary>("install_third_party_library_from_git", {
    projectDir: params.projectDir ?? undefined,
    scope: params.scope,
    url: params.url,
    refName: params.refName ?? undefined,
    displayName: params.displayName ?? undefined,
  });
}

export async function syncThirdPartyLibrary(params: {
  projectDir?: string | null;
  scope: string;
  libraryId: string;
}): Promise<void> {
  await invoke("sync_third_party_library", {
    projectDir: params.projectDir ?? undefined,
    scope: params.scope,
    libraryId: params.libraryId,
  });
}

export async function syncAllThirdPartyLibraries(projectDir?: string | null): Promise<number> {
  return invoke<number>("sync_all_third_party_libraries", {
    projectDir: projectDir ?? undefined,
  });
}

export async function suggestLibraryForMissingType(typeName: string): Promise<LibrarySuggestion | null> {
  return invoke<LibrarySuggestion | null>("suggest_library_for_missing_type", { typeName });
}

export async function getComponentTypeDetails(
  projectDir: string | null | undefined,
  typeName: string,
  libraryId?: string | null,
): Promise<ComponentTypeInfo> {
  return invoke<ComponentTypeInfo>("get_component_type_details", {
    projectDir: projectDir ?? undefined,
    typeName,
    libraryId: libraryId ?? undefined,
  });
}

export async function readComponentTypeSource(
  projectDir: string | null | undefined,
  typeName: string,
  libraryId?: string | null,
): Promise<ComponentTypeSource> {
  return invoke<ComponentTypeSource>("read_component_type_source", {
    projectDir: projectDir ?? undefined,
    typeName,
    libraryId: libraryId ?? undefined,
  });
}

export async function getComponentTypeRelationGraph(
  projectDir: string | null | undefined,
  typeName: string,
  libraryId?: string | null,
): Promise<ComponentTypeRelationGraph> {
  return invoke<ComponentTypeRelationGraph>("get_component_type_relation_graph", {
    projectDir: projectDir ?? undefined,
    typeName,
    libraryId: libraryId ?? undefined,
  });
}

export const GRAPHICAL_DOCUMENT_LOAD_TIMEOUT_MS = 45_000;

export class DiagramLoadTimeoutError extends Error {
  override readonly name = "DiagramLoadTimeout";
  constructor(message = "Diagram load timed out") {
    super(message);
  }
}

export function isDiagramLoadTimeout(err: unknown): boolean {
  return err instanceof DiagramLoadTimeoutError;
}

function withTimeout<T>(promise: Promise<T>, ms: number, onTimeout: () => void): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      onTimeout();
      reject(new DiagramLoadTimeoutError());
    }, ms);
    promise.then(
      (v) => {
        clearTimeout(timer);
        resolve(v);
      },
      (e) => {
        clearTimeout(timer);
        reject(e);
      },
    );
  });
}

export type GetGraphicalDocumentFromSourceOptions = {
  /** When set, reject if the invoke does not complete in time (default: none). */
  timeoutMs?: number;
  /** Called once when the timeout fires (e.g. to mark load as cancelled). */
  onTimeout?: () => void;
};

export async function getGraphicalDocumentFromSource<TAnnotation = unknown, TComponent = unknown, TConnection = unknown>(
  source: string,
  projectDir?: string | null,
  relativePath?: string | null,
  options?: GetGraphicalDocumentFromSourceOptions,
): Promise<GraphicalDocumentModel<TAnnotation, TComponent, TConnection>> {
  // #region agent log
  const t0 = performance.now();
  // #endregion
  const invokeOnce = () =>
    invoke<GraphicalDocumentModel<TAnnotation, TComponent, TConnection>>("get_graphical_document_from_source", {
      source,
      projectDir: projectDir ?? undefined,
      relativePath: relativePath ?? undefined,
    });

  try {
    // #region agent log
    agentDebugLog({
      location: "tauri:getGraphicalDocumentFromSource",
      message: "before invoke",
      data: { sourceLen: source.length },
      hypothesisId: "I",
    });
    // #endregion
    const timeoutMs = options?.timeoutMs;
    const out =
      timeoutMs != null && timeoutMs > 0 ?
        await withTimeout(invokeOnce(), timeoutMs, () => options?.onTimeout?.())
      : await invokeOnce();
    // #region agent log
    agentDebugLog({
      location: "tauri:getGraphicalDocumentFromSource",
      message: "invoke ok",
      data: { ms: Math.round(performance.now() - t0) },
      hypothesisId: "H",
    });
    // #endregion
    return out;
  } catch (e) {
    // #region agent log
    agentDebugLog({
      location: "tauri:getGraphicalDocumentFromSource",
      message: "invoke err",
      data: { ms: Math.round(performance.now() - t0) },
      hypothesisId: "H",
    });
    // #endregion
    throw e;
  }
}

export type ApplyGraphicalDocumentEditsResult = {
  newSource: string;
  /** Set when `.modai/diagram-state.json` could not be written (`.mo` still updated). */
  warning?: string;
};

export async function applyGraphicalDocumentEdits<TAnnotation = unknown, TComponent = unknown, TConnection = unknown>(
  source: string,
  document: GraphicalDocumentModel<TAnnotation, TComponent, TConnection>,
  projectDir?: string | null,
  relativePath?: string | null,
): Promise<ApplyGraphicalDocumentEditsResult> {
  // #region agent log
  const t0 = performance.now();
  // #endregion
  try {
    // #region agent log
    agentDebugLog({
      location: "tauri:applyGraphicalDocumentEdits",
      message: "before invoke",
      data: { sourceLen: source.length, componentCount: document.components?.length ?? -1 },
      hypothesisId: "I",
    });
    // #endregion
    const out = await invoke<ApplyGraphicalDocumentEditsResult>("apply_graphical_document_edits", {
      source,
      document,
      projectDir: projectDir ?? undefined,
      relativePath: relativePath ?? undefined,
    });
    // #region agent log
    agentDebugLog({
      location: "tauri:applyGraphicalDocumentEdits",
      message: "invoke ok",
      data: { ms: Math.round(performance.now() - t0) },
      hypothesisId: "H",
    });
    // #endregion
    return out;
  } catch (e) {
    // #region agent log
    agentDebugLog({
      location: "tauri:applyGraphicalDocumentEdits",
      message: "invoke err",
      data: { ms: Math.round(performance.now() - t0) },
      hypothesisId: "H",
    });
    // #endregion
    throw e;
  }
}

export async function readProjectFile(projectDir: string, relativePath: string): Promise<string> {
  return invoke<string>("read_project_file", { projectDir, relativePath });
}

export async function writeProjectFile(projectDir: string, relativePath: string, content: string): Promise<void> {
  await invoke("write_project_file", { projectDir, relativePath, content });
}

