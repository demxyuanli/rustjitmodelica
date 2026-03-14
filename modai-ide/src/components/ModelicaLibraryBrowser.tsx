import { LibrariesBrowser, MODELICA_DRAG_TYPE } from "./LibrariesBrowser";

interface ModelicaLibraryBrowserProps {
  projectDir: string | null;
  readOnly?: boolean;
  onOpenType?: (typeName: string, libraryId?: string) => void;
}

export function ModelicaLibraryBrowser({
  projectDir,
  readOnly = false,
  onOpenType,
}: ModelicaLibraryBrowserProps) {
  return <LibrariesBrowser projectDir={projectDir} readOnly={readOnly} onOpenType={onOpenType} />;
}

export { MODELICA_DRAG_TYPE };
