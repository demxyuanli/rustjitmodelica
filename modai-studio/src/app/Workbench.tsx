import { StudioCanvas } from "../canvas/StudioCanvas";
import { CanvasFindBar } from "../canvas/CanvasFindBar";
import { DashboardPanel } from "../dashboard/DashboardPanel";
import { DictionaryPanel } from "../dictionary/DictionaryPanel";
import { t } from "../i18n";
import { LibraryPanel } from "../library/LibraryPanel";
import { OutlinePanel } from "../outline/OutlinePanel";
import { PropertiesPanel } from "../properties/PropertiesPanel";
import { EdgePropertiesPanel } from "../properties/EdgePropertiesPanel";
import { SelectionToolsPanel } from "../properties/SelectionToolsPanel";
import { ResultsDock } from "../results/ResultsDock";
import { ExportWizard } from "../export/ExportWizard";
import { SettingsDialog } from "../settings/SettingsDialog";
import { ConfigDialog } from "../sim/ConfigDialog";
import { DEFAULT_EXPORT_PREFS } from "../settings/studioSettings";
import { ActivityBar } from "../shell/ActivityBar";
import { ResizeHandle } from "../shell/ResizeHandle";
import { StatusBar } from "../shell/StatusBar";
import { TitleBar } from "../shell/TitleBar";
import { ToolBar } from "../shell/ToolBar";
import { WelcomeOverlay } from "../shell/WelcomeOverlay";
import type { StudioSession } from "./useStudioSession";

export function Workbench({ session: s }: { session: StudioSession }) {
  const dict = s.manifest.paramDictionary ?? [];
  const propsDashboard = (
    <>
      <SelectionToolsPanel
        nodes={s.diagram.nodes}
        selectedIds={s.selectedIds}
        catalog={s.catalog}
        onApplyNodes={s.onApplyNodes}
      />
      {s.selectedEdge && s.selectedIds.length === 0 ? (
        <EdgePropertiesPanel
          edge={s.selectedEdge}
          diagram={s.diagram}
          onChange={s.onEdgePropChange}
        />
      ) : (
        <PropertiesPanel
          node={s.selectedIds.length >= 2 ? null : s.selectedNode}
          catalog={s.catalog}
          dictionary={dict}
          onChange={s.onNodePropChange}
          onDictionary={(next) =>
            s.setManifest({ ...s.manifest, paramDictionary: next })
          }
          multiSelectHint={s.selectedIds.length >= 2}
          onAddVariant={s.doAddVariant}
          onSetActiveVariant={s.doSetActiveVariant}
          onRenameVariant={s.doRenameVariant}
          onRemoveVariant={s.doRemoveVariant}
        />
      )}
      {s.showDashboard ? (
        <DashboardPanel
          nodes={s.diagram.nodes}
          catalog={s.catalog}
          dictionary={dict}
          onChangeParam={s.onDashboardParamChange}
          onChangeDict={(name, value) => {
            const next = dict.map((e) => (e.name === name ? { ...e, value } : e));
            s.setManifest({ ...s.manifest, paramDictionary: next });
          }}
          onApplyRun={() => void s.doRun()}
          busy={s.busy}
        />
      ) : null}
    </>
  );

  return (
    <div className="studio-shell vscode-shell">
      <TitleBar
        settings={s.settings}
        menuActions={s.menuActions}
        title={`${s.manifest.modelName}${s.dirty ? " *" : ""} — ${t("appTitle")}`}
        onOpenSettings={() => s.openSettings("general")}
        onRequestClose={s.requestClose}
      />
      <div className="studio-body">
        <ActivityBar
          activity={s.layout.activity}
          showSidebar={s.layout.showSidebar}
          showPanel={s.layout.showPanel}
          showSecondary={s.layout.showSecondary}
          onActivity={s.setActivity}
          onTogglePanel={() => s.patchLayout((l) => ({ showPanel: !l.showPanel }))}
          onToggleSecondary={() => s.patchLayout((l) => ({ showSecondary: !l.showSecondary }))}
          onOpenSettings={() => s.openSettings("general")}
        />
        {s.layout.showSidebar ? (
          <>
            <aside className="primary-sidebar">
              {s.layout.activity === "data" ? (
                <DictionaryPanel
                  manifest={s.manifest}
                  onManifest={s.setManifest}
                  onLoggedVars={s.selectVars}
                />
              ) : (
                <div className="left-column">
                  <div className="left-pane-library">
                    <LibraryPanel
                      catalog={s.catalog}
                      filter={s.filter}
                      onFilter={s.setFilter}
                      selectedId={s.pending?.id ?? null}
                      onSelect={s.setPending}
                    />
                  </div>
                  <ResizeHandle
                    axis="y"
                    label={t("resizeLibrary")}
                    onDelta={(d) => {
                      const col = document.querySelector(".left-column") as HTMLElement | null;
                      const h = col?.clientHeight || 1;
                      s.patchLayout((l) => ({
                        libraryPct: l.libraryPct + (d / h) * 100,
                      }));
                    }}
                  />
                  <div className="left-pane-outline">
                    <OutlinePanel
                      nodes={s.diagram.nodes}
                      selectedIds={s.selectedIds}
                      onSelect={s.selectOutlineNode}
                    />
                  </div>
                </div>
              )}
            </aside>
            <ResizeHandle
              axis="x"
              label={t("resizeLeft")}
              onDelta={(d) => s.patchLayout((l) => ({ leftWidth: l.leftWidth + d }))}
            />
          </>
        ) : null}
        <div className="editor-column">
          {s.viewStack.length > 1 ? (
            <div className="editor-tabs breadcrumb">
              {s.crumbLabels.map((label, i) => (
                <button
                  key={`${s.viewStack[i]}_${i}`}
                  type="button"
                  onClick={() => s.navigateTo(i)}
                >
                  {label}
                  {i < s.crumbLabels.length - 1 ? " / " : ""}
                </button>
              ))}
            </div>
          ) : null}
          <ToolBar
            settings={s.settings}
            actions={{
              onSave: () => void s.doSave(),
              onUpdate: () => void s.doUpdate(),
              onRun: () => void s.doRun(),
              onStop: () => void s.doStop(),
              onPause: () => void s.menuActions.onPause(),
              onResume: () => void s.menuActions.onResume(),
              onStep: () => void s.menuActions.onStep(),
              paused: s.menuActions.paused,
              onAlign: s.doAlign,
              onReroute: s.doReroute,
              onRotate: s.doRotateSelection,
              snapEnabled: s.snapEnabled,
              onSnap: s.setSnapEnabled,
              showDashboard: s.showDashboard,
              onDashboard: s.setShowDashboard,
              fastRestart: s.fastRestart,
              onFastRestart: s.setFastRestart,
              fullJitValidate: s.fullJitValidate,
              onFullJitValidate: s.setFullJitValidate,
              manifest: s.manifest,
              onManifest: s.setManifest,
              onConfig: () => s.setConfigOpen(true),
              busy: s.busy,
              bgValidate: s.bgValidate,
              runId: s.runId,
              canAlign: s.selectedIds.length >= 2,
              canRotate: s.selectedIds.length >= 1,
              status: s.status,
            }}
          />
          {s.busy ? (
            <div className="progress-track">
              <div
                className="progress-fill"
                style={{ width: `${Math.round((s.progressFraction ?? 0) * 100)}%` }}
              />
              <span className="progress-label">{s.progressLabel ?? t("running")}</span>
            </div>
          ) : s.bgValidate ? (
            <div className="progress-track progress-bg">
              <div className="progress-fill" />
              <span className="progress-label">{s.status}</span>
            </div>
          ) : (
            <div className="progress-track progress-idle" />
          )}
          <div className="editor-main">
            <CanvasFindBar
              open={s.findOpen}
              nodes={s.diagram.nodes}
              onClose={() => s.setFindOpen(false)}
              onFocus={(id) => {
                s.selectOutlineNode(id);
                s.setFindOpen(false);
              }}
            />
            <StudioCanvas
              diagram={s.diagram}
              diagramRev={s.diagramRev}
              selectedIds={s.selectedIds}
              selectedEdgeIds={s.selectedEdgeIds}
              snapEnabled={s.snapEnabled}
              onDiagramChange={s.updateDiagram}
              onSelectionChange={s.setSelection}
              pendingCatalog={s.pending}
              onConsumePendingCatalog={() => s.setPending(null)}
              focusNodeId={s.focusNodeId}
              catalog={s.catalog}
              onNodeDoubleClick={(id) => {
                const n = s.diagram.nodes.find((x) => x.id === id);
                if (n?.nodeKind === "subsystem") {
                  s.drillInto(id);
                  return;
                }
                s.selectOutlineNode(id);
                s.patchLayout(() => ({ showSecondary: true }));
              }}
              onConnectRejected={s.setStatus}
              onCreateSubsystem={s.doCreateSubsystem}
              onPlotSelection={s.doPlotSelection}
              onRotateSelection={s.doRotateSelection}
              onDeleteSelection={s.doDeleteSelection}
              displayValues={s.displayValues}
              edgeLabelsVisible={s.settings.edgeLabelsVisible}
              edgeLabelOffsetY={s.settings.edgeLabelOffsetY}
            />
          </div>
          {s.layout.showPanel ? (
            <>
              <ResizeHandle
                axis="y"
                label={t("resizeResults")}
                onDelta={(d) =>
                  s.patchLayout((l) => ({ resultsHeight: l.resultsHeight - d }))
                }
              />
              <div className="panel-area">
                <ResultsDock
                  tab={s.tab}
                  onTab={s.setTab}
                  result={s.result}
                  selectedVars={s.selectedVars}
                  onToggleVar={s.toggleSelectedVar}
                  report={s.report}
                  busy={s.busy}
                  progressText={s.progressLabel}
                  onMessageClick={s.onMessageClick}
                  runs={s.runs}
                  overlayRunIds={s.overlayRunIds}
                  onToggleOverlayRun={s.toggleOverlayRun}
                  xyX={s.xyX}
                  xyY={s.xyY}
                  onXyX={s.setXyX}
                  onXyY={s.setXyY}
                  diagram={s.effectiveRoot}
                  profile={s.lastProfile}
                  profileWallMs={s.profileWallMs}
                  profileRunId={s.profileRunId}
                  onSelectProfileRun={s.selectProfileRun}
                  emitHistory={s.emitHistory}
                />
              </div>
            </>
          ) : null}
        </div>
        {s.layout.showSecondary ? (
          <>
            <ResizeHandle
              axis="x"
              label={t("resizeRight")}
              onDelta={(d) => s.patchLayout((l) => ({ rightWidth: l.rightWidth - d }))}
            />
            <aside className="secondary-sidebar">{propsDashboard}</aside>
          </>
        ) : null}
      </div>
      <StatusBar
        status={s.status}
        projectLabel={s.projectLabel}
        busy={s.busy}
        bgValidate={s.bgValidate}
        onCancelValidate={s.cancelBgValidate}
        locale={s.settings.locale}
        onToggleLocale={() =>
          s.updateSettings({
            ...s.settings,
            locale: s.settings.locale === "zh" ? "en" : "zh",
          })
        }
      />
      <WelcomeOverlay
        visible={s.welcomeOpen}
        initialTab={s.welcomeTab}
        intent={s.welcomeIntent}
        seedName={s.manifest.modelName}
        seedSim={s.manifest.simulationDefaults}
        openSeq={s.welcomeSeq}
        recentProjects={s.recentProjects}
        onClose={() => s.setWelcomeOpen(false)}
        onOpenExample={(id) => s.doOpenExample(id)}
        onOpenProject={() => s.doOpen()}
        onOpenRecent={(path) => s.doOpenRecent(path)}
        onNewProject={(init) =>
          s.welcomeIntent === "saveAs" ? s.doSaveAs(init) : s.doNew(init)
        }
      />
      <ConfigDialog
        open={s.configOpen}
        defaults={s.manifest.simulationDefaults}
        onClose={() => s.setConfigOpen(false)}
        onApply={(next) => {
          s.setManifest({ ...s.manifest, simulationDefaults: next });
          if (next.loggedVariables?.length) s.selectVars(next.loggedVariables);
        }}
      />
      <SettingsDialog
        open={s.settingsOpen}
        settings={s.settings}
        onChange={s.updateSettings}
        onClose={() => s.setSettingsOpen(false)}
        tab={s.settingsTab}
        onTabChange={s.setSettingsTab}
      />
      <ExportWizard
        open={s.exportWizardOpen}
        modelName={s.manifest.modelName}
        prefs={s.settings.lastExport ?? DEFAULT_EXPORT_PREFS}
        presetMode={s.exportPreset}
        busy={s.busy}
        onClose={s.closeExportWizard}
        onExport={(opts) => {
          s.updateSettings({
            ...s.settings,
            lastExport: {
              mode: opts.mode,
              coderPackage: opts.coderPackage,
              modelIdentifier: opts.modelIdentifier ?? "",
              guid: opts.guid ?? "",
              lastOutDir: opts.outDir,
            },
          });
          void s.doEmitWithOptions(opts);
        }}
      />
    </div>
  );
}
