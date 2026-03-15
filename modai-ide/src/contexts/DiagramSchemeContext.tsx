import { createContext, useCallback, useContext, useEffect, useState } from "react";
import {
  DIAGRAM_COLOR_SCHEMES,
  getActiveSchemeId,
  getSchemeById,
  setActiveSchemeId,
  type DiagramColorScheme,
} from "../utils/diagramColorSchemes";

interface DiagramSchemeContextValue {
  schemeId: string;
  scheme: DiagramColorScheme;
  setSchemeId: (id: string) => void;
}

const DiagramSchemeContext = createContext<DiagramSchemeContextValue | null>(null);

export function DiagramSchemeProvider({ children }: { children: React.ReactNode }) {
  const [schemeId, setSchemeIdState] = useState(getActiveSchemeId);
  const [, setRefresh] = useState(0);

  const scheme = getSchemeById(schemeId) ?? DIAGRAM_COLOR_SCHEMES[0];

  const setSchemeId = useCallback((id: string) => {
    if (!DIAGRAM_COLOR_SCHEMES.some((s) => s.id === id)) return;
    setActiveSchemeId(id);
    setSchemeIdState(id);
  }, []);

  useEffect(() => {
    const handler = () => {
      setSchemeIdState(getActiveSchemeId());
      setRefresh((r) => r + 1);
    };
    window.addEventListener("modai-diagram-scheme-change", handler);
    return () => window.removeEventListener("modai-diagram-scheme-change", handler);
  }, []);

  return (
    <DiagramSchemeContext.Provider value={{ schemeId, scheme, setSchemeId }}>
      {children}
    </DiagramSchemeContext.Provider>
  );
}

export function useDiagramScheme(): DiagramSchemeContextValue {
  const ctx = useContext(DiagramSchemeContext);
  if (!ctx) {
    const scheme = getSchemeById(getActiveSchemeId()) ?? DIAGRAM_COLOR_SCHEMES[0];
    return {
      schemeId: getActiveSchemeId(),
      scheme,
      setSchemeId: setActiveSchemeId,
    };
  }
  return ctx;
}
