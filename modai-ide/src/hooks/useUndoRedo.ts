import { useRef, useCallback, useState } from "react";

export interface UndoRedoSnapshot<T> {
  data: T;
  timestamp: number;
}

export interface UseUndoRedoResult<T> {
  push: (snapshot: T) => void;
  undo: () => T | null;
  redo: () => T | null;
  canUndo: boolean;
  canRedo: boolean;
  clear: () => void;
}

const MAX_STACK = 50;
const MERGE_WINDOW_MS = 300;

export function useUndoRedo<T>(maxStack: number = MAX_STACK): UseUndoRedoResult<T> {
  const undoStack = useRef<UndoRedoSnapshot<T>[]>([]);
  const redoStack = useRef<UndoRedoSnapshot<T>[]>([]);
  const [revision, setRevision] = useState(0);

  const push = useCallback((snapshot: T) => {
    const now = Date.now();
    const top = undoStack.current[undoStack.current.length - 1];
    if (top && now - top.timestamp < MERGE_WINDOW_MS) {
      top.data = snapshot;
      top.timestamp = now;
    } else {
      undoStack.current.push({ data: snapshot, timestamp: now });
      if (undoStack.current.length > maxStack) {
        undoStack.current.shift();
      }
    }
    redoStack.current = [];
    setRevision((r) => r + 1);
  }, [maxStack]);

  const undo = useCallback((): T | null => {
    const item = undoStack.current.pop();
    if (!item) return null;
    redoStack.current.push(item);
    setRevision((r) => r + 1);
    const prev = undoStack.current[undoStack.current.length - 1];
    return prev?.data ?? null;
  }, []);

  const redo = useCallback((): T | null => {
    const item = redoStack.current.pop();
    if (!item) return null;
    undoStack.current.push(item);
    setRevision((r) => r + 1);
    return item.data;
  }, []);

  const clear = useCallback(() => {
    undoStack.current = [];
    redoStack.current = [];
    setRevision((r) => r + 1);
  }, []);

  void revision;

  return {
    push,
    undo,
    redo,
    canUndo: undoStack.current.length > 1,
    canRedo: redoStack.current.length > 0,
    clear,
  };
}
