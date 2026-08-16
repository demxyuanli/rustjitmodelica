import { useCallback, useMemo, useRef, useState } from "react";
import type { DiagramDocument, DiagramNode, OpenProjectResult } from "../api/types";
import {
  alignSelection,
  copySelection,
  extractInstanceHint,
  pasteClipboard,
  type AlignMode,
} from "../canvas/editOps";
import { createSubsystemFromSelection, syncAllSubsystemPorts } from "../canvas/subsystemOps";
import {
  addVariantChoice,
  removeVariantChoice,
  renameVariantChoice,
  setActiveVariant,
} from "../canvas/variantOps";
import { rerouteDiagramEdges, rerouteEdgesTouchingNodes } from "../canvas/edgePath";
import { t, tf } from "../i18n";
import { useDiagramHistory } from "../project/useDiagramHistory";

function sameIdList(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

export type StudioEditSaveSnapshot = {
  viewId: string;
  diagram: DiagramDocument;
  rootDiagram: DiagramDocument;
  subsystems: Record<string, DiagramDocument>;
};

export function useStudioEdit(
  bootDiagram: DiagramDocument,
  setStatus: (status: string) => void,
) {
  const [rootDiagram, setRootDiagram] = useState(bootDiagram);
  const [subsystems, setSubsystems] = useState<Record<string, DiagramDocument>>({});
  const [viewStack, setViewStack] = useState<string[]>(["root"]);
  const { diagram, commit, replace, undo, redo } = useDiagramHistory(bootDiagram);
  const [diagramRev, setDiagramRev] = useState(0);
  const [dirty, setDirty] = useState(false);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [selectedEdgeIds, setSelectedEdgeIds] = useState<string[]>([]);
  const [snapEnabled, setSnapEnabled] = useState(true);
  const [focusNodeId, setFocusNodeId] = useState<string | null>(null);
  const [showDashboard, setShowDashboard] = useState(true);
  const clipboard = useRef<{ nodes: DiagramNode[]; edges: import("../api/types").DiagramEdge[] } | null>(
    null,
  );

  const viewId = viewStack[viewStack.length - 1] ?? "root";

  const persistView = useCallback((d: DiagramDocument, id: string) => {
    if (id === "root") setRootDiagram(d);
    else setSubsystems((prev) => ({ ...prev, [id]: d }));
  }, []);

  const applyLoadedProject = useCallback(
    (p: OpenProjectResult) => {
      setRootDiagram(p.diagram);
      setSubsystems(p.subsystems ?? {});
      setViewStack(["root"]);
      replace(p.diagram);
      setDiagramRev((x) => x + 1);
      setDirty(false);
      setSelectedIds([]);
      setSelectedEdgeIds([]);
      setFocusNodeId(null);
    },
    [replace],
  );

  const updateDiagram = useCallback(
    (d: DiagramDocument) => {
      commit(d, true);
      persistView(d, viewId);
      setDirty(true);
    },
    [commit, persistView, viewId],
  );

  const replaceDiagram = useCallback(
    (d: DiagramDocument) => {
      commit(d, true);
      persistView(d, viewId);
      setDiagramRev((x) => x + 1);
      setDirty(true);
    },
    [commit, persistView, viewId],
  );

  const doUndo = () => {
    if (undo()) {
      setDiagramRev((x) => x + 1);
      setDirty(true);
      setStatus(t("statusUndo"));
    }
  };

  const doRedo = () => {
    if (redo()) {
      setDiagramRev((x) => x + 1);
      setDirty(true);
      setStatus(t("statusRedo"));
    }
  };

  const selectedNode: DiagramNode | null = useMemo(() => {
    const id = selectedIds[0];
    if (!id) return null;
    return diagram.nodes.find((n) => n.id === id) ?? null;
  }, [selectedIds, diagram.nodes]);

  const selectedEdge = useMemo(() => {
    if (selectedIds.length > 0) return null;
    const id = selectedEdgeIds[0];
    if (!id) return null;
    return diagram.edges.find((e) => e.id === id) ?? null;
  }, [selectedIds, selectedEdgeIds, diagram.edges]);

  const setSelection = useCallback((nodeIds: string[], edgeIds: string[] = []) => {
    setSelectedIds((prev) => (sameIdList(prev, nodeIds) ? prev : nodeIds));
    setSelectedEdgeIds((prev) => (sameIdList(prev, edgeIds) ? prev : edgeIds));
  }, []);

  const onNodePropChange = useCallback(
    (n: DiagramNode) => {
      replaceDiagram({
        ...diagram,
        nodes: diagram.nodes.map((x) => (x.id === n.id ? n : x)),
      });
    },
    [diagram, replaceDiagram],
  );

  const onApplyNodes = useCallback(
    (next: DiagramNode[]) => {
      replaceDiagram({ ...diagram, nodes: next });
    },
    [diagram, replaceDiagram],
  );

  const onEdgePropChange = useCallback(
    (e: import("../api/types").DiagramEdge) => {
      replaceDiagram({
        ...diagram,
        edges: diagram.edges.map((x) => (x.id === e.id ? e : x)),
      });
    },
    [diagram, replaceDiagram],
  );

  const onDashboardParamChange = useCallback(
    (nodeId: string, paramName: string, value: string) => {
      replaceDiagram({
        ...diagram,
        nodes: diagram.nodes.map((n) =>
          n.id !== nodeId
            ? n
            : {
                ...n,
                params: n.params.map((p) => (p.name === paramName ? { ...p, value } : p)),
              },
        ),
      });
    },
    [diagram, replaceDiagram],
  );

  const effectiveRoot = viewId === "root" ? diagram : rootDiagram;
  const effectiveSubs = viewId === "root" ? subsystems : { ...subsystems, [viewId]: diagram };

  const getSaveSnapshot = useCallback((): StudioEditSaveSnapshot => {
    const root = viewId === "root" ? diagram : rootDiagram;
    const subs = viewId === "root" ? subsystems : { ...subsystems, [viewId]: diagram };
    const synced = syncAllSubsystemPorts(root, subs);
    return {
      viewId,
      diagram: viewId === "root" ? synced.root : synced.subsystems[viewId] ?? diagram,
      rootDiagram: synced.root,
      subsystems: synced.subsystems,
    };
  }, [viewId, diagram, rootDiagram, subsystems]);

  const syncAfterSave = useCallback(
    (root: DiagramDocument, subs: Record<string, DiagramDocument>) => {
      setRootDiagram(root);
      setSubsystems(subs);
    },
    [],
  );

  const doCopy = () => {
    clipboard.current = copySelection(diagram, selectedIds);
    if (clipboard.current) {
      setStatus(tf("statusCopied", { n: clipboard.current.nodes.length }));
    }
  };

  const doPaste = () => {
    if (!clipboard.current) return;
    replaceDiagram(pasteClipboard(diagram, clipboard.current));
    setStatus(t("statusPasted"));
  };

  const doAlign = (mode: AlignMode) => {
    replaceDiagram(alignSelection(diagram, selectedIds, mode));
  };

  const doReroute = () => {
    const ids = selectedEdgeIds.length ? selectedEdgeIds : undefined;
    replaceDiagram(rerouteDiagramEdges(diagram, ids));
    setStatus(ids?.length ? t("statusRerouteSelected") : t("statusRerouteAll"));
  };

  const doRotateSelection = () => {
    if (!selectedIds.length) {
      setStatus(t("statusSelectBlocksFirst"));
      return;
    }
    const rotated = {
      ...diagram,
      nodes: diagram.nodes.map((n) =>
        selectedIds.includes(n.id) && n.nodeKind !== "draw"
          ? { ...n, rotation: (((n.rotation ?? 0) + 90) % 360) }
          : n,
      ),
    };
    replaceDiagram(rerouteEdgesTouchingNodes(rotated, selectedIds));
    setStatus(t("statusRotated"));
  };

  const doCreateSubsystem = () => {
    const res = createSubsystemFromSelection(diagram, selectedIds);
    if (!res) {
      setStatus(t("statusSelectBlocksFirst"));
      return;
    }
    setSubsystems((prev) => ({ ...prev, [res.child.diagramId]: res.child }));
    replaceDiagram(res.parent);
    setSelectedIds([res.subsystemNodeId]);
    setSelectedEdgeIds([]);
    setStatus(tf("statusCreated", { name: res.child.modelName }));
  };

  const doDeleteSelection = () => {
    const nodeIds = new Set(selectedIds);
    const edgeIds = new Set(selectedEdgeIds);
    if (!nodeIds.size && !edgeIds.size) return;
    replaceDiagram({
      ...diagram,
      nodes: diagram.nodes.filter((n) => !nodeIds.has(n.id)),
      edges: diagram.edges.filter(
        (e) =>
          !edgeIds.has(e.id) &&
          !nodeIds.has(e.sourceNodeId) &&
          !nodeIds.has(e.targetNodeId),
      ),
    });
    setSelectedIds([]);
    setSelectedEdgeIds([]);
  };

  const doAddVariant = () => {
    const id = selectedIds[0];
    const n = id ? diagram.nodes.find((x) => x.id === id) : null;
    if (!n || n.nodeKind !== "subsystem") {
      setStatus(t("statusSelectSubsystem"));
      return;
    }
    const res = addVariantChoice(diagram, subsystems, n.id);
    if (!res) {
      setStatus(t("statusVariantFailed"));
      return;
    }
    setSubsystems(res.subsystems);
    replaceDiagram(res.parent);
    setStatus(t("statusVariantAdded"));
  };

  const doSetActiveVariant = (variantId: string) => {
    const id = selectedIds[0];
    if (!id) return;
    const res = setActiveVariant(diagram, subsystems, id, variantId);
    if (!res) return;
    setSubsystems(res.subsystems);
    replaceDiagram(res.parent);
    setStatus(t("statusVariantSwitched"));
  };

  const doRenameVariant = (variantId: string, label: string) => {
    const id = selectedIds[0];
    if (!id) return;
    const next = renameVariantChoice(diagram, id, variantId, label);
    if (!next) return;
    replaceDiagram(next);
  };

  const doRemoveVariant = (variantId: string) => {
    const id = selectedIds[0];
    if (!id) return;
    const res = removeVariantChoice(diagram, subsystems, id, variantId);
    if (!res) {
      setStatus(t("statusVariantKeepOne"));
      return;
    }
    setSubsystems(res.subsystems);
    replaceDiagram(res.parent);
    setStatus(t("statusVariantRemoved"));
  };

  const drillInto = (nodeId: string) => {
    const n = diagram.nodes.find((x) => x.id === nodeId);
    if (!n || n.nodeKind !== "subsystem" || !n.subsystemRef) return;
    const child = subsystems[n.subsystemRef];
    if (!child) {
      setStatus(t("statusMissingSubsystem"));
      return;
    }
    persistView(diagram, viewId);
    setViewStack((s) => [...s, n.subsystemRef!]);
    replace(child);
    setDiagramRev((x) => x + 1);
    setSelectedIds([]);
    setSelectedEdgeIds([]);
    setStatus(tf("statusDrilledInto", { name: n.instanceName }));
  };

  const navigateTo = (index: number) => {
    if (index < 0 || index >= viewStack.length) return;
    persistView(diagram, viewId);
    const nextStack = viewStack.slice(0, index + 1);
    const targetId = nextStack[nextStack.length - 1] ?? "root";

    // Promote In/Out terminals from the diagram we leave onto its parent subsystem face.
    let nextRoot = viewId === "root" ? diagram : rootDiagram;
    let nextSubs =
      viewId === "root" ? { ...subsystems } : { ...subsystems, [viewId]: diagram };
    if (viewId !== "root") {
      const synced = syncAllSubsystemPorts(nextRoot, nextSubs);
      nextRoot = synced.root;
      nextSubs = synced.subsystems;
      setRootDiagram(nextRoot);
      setSubsystems(nextSubs);
    }

    const target = targetId === "root" ? nextRoot : nextSubs[targetId];
    if (!target) return;
    setViewStack(nextStack);
    replace(target);
    setDiagramRev((x) => x + 1);
    setSelectedIds([]);
    setSelectedEdgeIds([]);
  };

  const onMessageClick = (d: { message: string; nodeId?: string }) => {
    const id =
      (d.nodeId && diagram.nodes.some((n) => n.id === d.nodeId) ? d.nodeId : null) ??
      extractInstanceHint(d.message, diagram);
    if (id) {
      setSelectedIds([id]);
      setSelectedEdgeIds([]);
      setFocusNodeId(id);
      setStatus(tf("statusFocused", { id }));
    }
  };

  const selectOutlineNode = (id: string) => {
    setSelectedIds([id]);
    setSelectedEdgeIds([]);
    setFocusNodeId(id);
  };

  const bumpDiagramRev = useCallback(() => {
    setDiagramRev((x) => x + 1);
  }, []);

  return {
    diagram,
    diagramRev,
    dirty,
    setDirty,
    selectedIds,
    setSelectedIds,
    selectedEdgeIds,
    setSelection,
    selectOutlineNode,
    selectedNode,
    selectedEdge,
    onNodePropChange,
    onEdgePropChange,
    onApplyNodes,
    onDashboardParamChange,
    showDashboard,
    setShowDashboard,
    snapEnabled,
    setSnapEnabled,
    focusNodeId,
    updateDiagram,
    replaceDiagram,
    drillInto,
    navigateTo,
    viewStack,
    subsystems,
    effectiveRoot,
    effectiveSubs,
    getSaveSnapshot,
    syncAfterSave,
    applyLoadedProject,
    doUndo,
    doRedo,
    doCopy,
    doPaste,
    doAlign,
    doReroute,
    doRotateSelection,
    doCreateSubsystem,
    doDeleteSelection,
    doAddVariant,
    doSetActiveVariant,
    doRenameVariant,
    doRemoveVariant,
    onMessageClick,
    bumpDiagramRev,
  };
}
