import { t } from "../i18n";
import { EquationGraphView } from "./EquationGraphView";

interface LibraryRelationGraphPaneProps {
  code: string | null;
  modelName: string | null;
  projectDir?: string | null;
}

export function LibraryRelationGraphPane({ code, modelName, projectDir }: LibraryRelationGraphPaneProps) {
  return (
    <div className="flex h-full min-h-[220px] flex-col">
      <div className="border-b border-border px-3 py-2 text-xs font-medium text-[var(--text-muted)]">
        {t("libraryInternalRelationGraph")}
      </div>
      <div className="flex-1 overflow-auto">
        {!code || !modelName ? (
          <div className="flex h-full items-center justify-center text-sm text-[var(--text-muted)]">
            {t("libraryNoSelection")}
          </div>
        ) : (
          <EquationGraphView code={code} modelName={modelName} projectDir={projectDir} />
        )}
      </div>
    </div>
  );
}
