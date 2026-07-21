import { useCallback, useRef, useState } from "react";
import type { DiagramDocument } from "../api/types";

const MAX = 80;

export function useDiagramHistory(initial: DiagramDocument) {
  const [diagram, setDiagramState] = useState(initial);
  const past = useRef<DiagramDocument[]>([]);
  const future = useRef<DiagramDocument[]>([]);

  const commit = useCallback((next: DiagramDocument, pushHistory: boolean) => {
    setDiagramState((prev) => {
      if (pushHistory) {
        past.current.push(structuredClone(prev));
        if (past.current.length > MAX) past.current.shift();
        future.current = [];
      }
      return next;
    });
  }, []);

  const replace = useCallback(
    (next: DiagramDocument) => {
      past.current = [];
      future.current = [];
      setDiagramState(next);
    },
    [],
  );

  const undo = useCallback(() => {
    const prev = past.current.pop();
    if (!prev) return false;
    setDiagramState((cur) => {
      future.current.push(structuredClone(cur));
      return prev;
    });
    return true;
  }, []);

  const redo = useCallback(() => {
    const n = future.current.pop();
    if (!n) return false;
    setDiagramState((cur) => {
      past.current.push(structuredClone(cur));
      return n;
    });
    return true;
  }, []);

  return { diagram, commit, replace, undo, redo, setDiagramState };
}
