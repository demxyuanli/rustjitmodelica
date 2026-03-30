import type { MouseEvent } from "react";
import { t } from "../../i18n";
import type { ComponentTypeInfo, InstantiableClass } from "../../types";
import { SectionIcon } from "./ComponentLibraryTypeTreePane";

export interface ComponentLibraryTypeDetailPaneProps {
  selectedClass: InstantiableClass | null;
  detail: ComponentTypeInfo | null;
  detailBusy: boolean;
  onOpenType?: (typeName: string, libraryId?: string) => void;
  onContentContextMenu: (event: MouseEvent) => void;
}

export function ComponentLibraryTypeDetailPane({
  selectedClass,
  detail,
  detailBusy,
  onOpenType,
  onContentContextMenu,
}: ComponentLibraryTypeDetailPaneProps) {
  return (
    <section className="min-w-0 flex-1 min-h-0 border-r border-border bg-surface flex flex-col">
      <div className="panel-header-bar flex items-center justify-between border-b border-border">
        <div className="flex items-center gap-2 text-sm font-medium">
          <SectionIcon letter="D" bg="bg-violet-500/85" title={t("libraryDetailsTitle")} />
          {t("libraryDetailsTitle")}
        </div>
        {selectedClass && onOpenType && (
          <button
            type="button"
            className="rounded border border-border px-2 py-1 text-xs hover:bg-[var(--surface-hover)]"
            onClick={() => onOpenType(selectedClass.qualifiedName, selectedClass.libraryId)}
          >
            {t("libraryOpenReadOnly")}
          </button>
        )}
      </div>
      <div
        className="min-h-0 flex-1 overflow-auto px-4 py-4"
        onContextMenu={onContentContextMenu}
      >
        {!selectedClass ? (
          <div className="text-sm text-[var(--text-muted)]">{t("libraryNoSelection")}</div>
        ) : detailBusy ? (
          <div className="text-sm text-[var(--text-muted)]">{t("loading")}</div>
        ) : detail ? (
          <div className="space-y-5 max-w-full">
            <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
              <div className="text-lg font-semibold">{detail.qualifiedName ?? detail.name}</div>
              <div className="mt-2 flex flex-wrap gap-3 text-xs text-[var(--text-muted)]">
                <span>{detail.libraryName}</span>
                <span>{detail.libraryScope}</span>
                <span>{detail.kind}</span>
              </div>
              {detail.summary && <div className="mt-3 text-sm text-[var(--text)]">{detail.summary}</div>}
              {detail.path && (
                <div className="mt-2 text-xs text-[var(--text-muted)]">
                  {t("librarySourcePath")}: {detail.path}
                </div>
              )}
              <div className="mt-2 text-xs text-[var(--text-muted)]">
                {t("libraryMetadataSource")}: {detail.metadataSource}
              </div>
            </section>

            <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
              <div className="text-sm text-[var(--text-muted)]">
                {detail.description ? (
                  detail.description.trim().startsWith("<") ? (
                    <div
                      className="prose prose-sm max-w-none prose-invert"
                      dangerouslySetInnerHTML={{ __html: detail.description }}
                    />
                  ) : (
                    <div className="whitespace-pre-wrap">{detail.description}</div>
                  )
                ) : (
                  <span>{t("libraryDetailsEmpty")}</span>
                )}
              </div>
            </section>

            <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
              <div className="text-sm font-medium">{t("libraryExtends")}</div>
              <div className="mt-2 flex flex-wrap gap-2">
                {(detail.extendsNames ?? []).length > 0 ? (
                  (detail.extendsNames ?? []).map((name) => (
                    <span key={name} className="rounded bg-[var(--surface)] px-2 py-1 text-xs text-[var(--text-muted)]">
                      {name}
                    </span>
                  ))
                ) : (
                  <span className="text-xs text-[var(--text-muted)]">{t("none")}</span>
                )}
              </div>
            </section>

            <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
              <div className="text-sm font-medium">{t("libraryParameters")}</div>
              <div className="mt-2 space-y-2">
                {detail.parameters.length > 0 ? (
                  detail.parameters.map((parameter) => (
                    <div key={parameter.name} className="rounded border border-border bg-[var(--surface)] px-3 py-2">
                      <div className="flex flex-wrap items-center gap-2 text-sm">
                        <span className="font-medium">{parameter.name}</span>
                        <span className="text-[var(--text-muted)]">{parameter.typeName}</span>
                        {parameter.defaultValue && (
                          <span className="text-[var(--text-muted)]">= {parameter.defaultValue}</span>
                        )}
                      </div>
                      {parameter.description && (
                        <div className="mt-1 text-xs text-[var(--text-muted)]">{parameter.description}</div>
                      )}
                    </div>
                  ))
                ) : (
                  <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                )}
              </div>
            </section>

            <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
              <div className="text-sm font-medium">{t("libraryConnectors")}</div>
              <div className="mt-2 space-y-2">
                {(detail.connectors ?? []).length > 0 ? (
                  (detail.connectors ?? []).map((connector) => (
                    <div key={connector.name} className="rounded border border-border bg-[var(--surface)] px-3 py-2">
                      <div className="flex flex-wrap items-center gap-2 text-sm">
                        <span className="font-medium">{connector.name}</span>
                        <span className="text-[var(--text-muted)]">{connector.typeName}</span>
                        <span className="rounded bg-[var(--surface-hover)] px-2 py-0.5 text-[11px] text-[var(--text-muted)]">
                          {connector.direction}
                        </span>
                      </div>
                      {connector.description && (
                        <div className="mt-1 text-xs text-[var(--text-muted)]">{connector.description}</div>
                      )}
                    </div>
                  ))
                ) : (
                  <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                )}
              </div>
            </section>

            <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
              <div className="text-sm font-medium">{t("libraryExamples")}</div>
              <div className="mt-2 space-y-2">
                {(detail.examples ?? []).length > 0 ? (
                  (detail.examples ?? []).map((example) => (
                    <div key={`${example.title}-${example.modelPath ?? ""}`} className="rounded border border-border bg-[var(--surface)] px-3 py-2">
                      <div className="text-sm font-medium">{example.title}</div>
                      {example.description && (
                        <div className="mt-1 text-xs text-[var(--text-muted)]">{example.description}</div>
                      )}
                      {example.modelPath && (
                        <div className="mt-1 text-[11px] text-[var(--text-muted)]">{example.modelPath}</div>
                      )}
                      {example.usage && (
                        <div className="mt-2 whitespace-pre-wrap text-xs text-[var(--text-muted)]">{example.usage}</div>
                      )}
                    </div>
                  ))
                ) : (
                  <div className="text-xs text-[var(--text-muted)]">{t("libraryExamplesEmpty")}</div>
                )}
              </div>
            </section>

            <section className="rounded border border-border bg-[var(--bg-elevated)] p-4">
              <div className="text-sm font-medium">{t("libraryUsageHelp")}</div>
              <div className="mt-2 whitespace-pre-wrap text-sm text-[var(--text-muted)]">
                {detail.usageHelp || t("libraryUsageHelpEmpty")}
              </div>
            </section>
          </div>
        ) : (
          <div className="text-sm text-[var(--text-muted)]">{t("libraryDetailsEmpty")}</div>
        )}
      </div>
    </section>
  );
}
