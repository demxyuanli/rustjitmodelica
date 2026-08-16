import { useCallback, useEffect, useRef, type PointerEvent as ReactPointerEvent } from "react";

export function ResizeHandle({
  axis,
  onDelta,
  label,
}: {
  axis: "x" | "y";
  onDelta: (delta: number) => void;
  label?: string;
}) {
  const dragging = useRef(false);
  const last = useRef(0);
  const onDeltaRef = useRef(onDelta);
  onDeltaRef.current = onDelta;

  const onPointerDown = useCallback(
    (ev: ReactPointerEvent<HTMLDivElement>) => {
      ev.preventDefault();
      dragging.current = true;
      last.current = axis === "x" ? ev.clientX : ev.clientY;
      ev.currentTarget.setPointerCapture(ev.pointerId);
      document.body.classList.add(axis === "x" ? "resizing-x" : "resizing-y");
    },
    [axis],
  );

  const onPointerMove = useCallback(
    (ev: ReactPointerEvent<HTMLDivElement>) => {
      if (!dragging.current) return;
      const pos = axis === "x" ? ev.clientX : ev.clientY;
      const delta = pos - last.current;
      if (delta === 0) return;
      last.current = pos;
      onDeltaRef.current(delta);
    },
    [axis],
  );

  const endDrag = useCallback((ev: ReactPointerEvent<HTMLDivElement>) => {
    if (!dragging.current) return;
    dragging.current = false;
    try {
      ev.currentTarget.releasePointerCapture(ev.pointerId);
    } catch {
      /* ignore */
    }
    document.body.classList.remove("resizing-x", "resizing-y");
  }, []);

  useEffect(() => {
    return () => {
      document.body.classList.remove("resizing-x", "resizing-y");
    };
  }, []);

  return (
    <div
      className={`resize-handle resize-${axis}`}
      role="separator"
      aria-orientation={axis === "x" ? "vertical" : "horizontal"}
      aria-label={label}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
    />
  );
}
