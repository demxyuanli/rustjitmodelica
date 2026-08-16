# Changelog

All notable changes to this project are documented in this file.

Format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Studio StateGraph catalog F95 (2026-08-16)

- Library: `Modelica.StateGraph` InitialStep / Step / Transition / StepWithSignal / TransitionWithSignal on the existing diagram (no JSON statechart).
- emit_mo injects `inner Modelica.StateGraph.StateGraphRoot stateGraphRoot` when graph nodes are present.
- Bundled example `examples/stategraph_first` (FirstExample-style two-step loop).
- Canvas minimap draws custom blocks (flow nodes now carry width/height) and opens pinned by default.

### Studio discrete-logic host (2026-08-16)

- Spec: sequential/mode logic stays Modelica (`when`/Logical, same-canvas `Modelica.StateGraph` blocks). No Stateflow Chart, no JSON statechart, no StateGraph2 host, no MLS chapter-17 editor in Studio. JIT chapter 17 is not StateGraph. MSL 4 still ships `StateGraph.mo`. F95 catalog is in this Unreleased section.

### Modai Studio P4 main-path feel (2026-08-15)

- P4-A: Library HTML5 drag onto canvas; `screenToFlowPosition` placement; four-pane Library+Properties; connect banner instead of `alert`; context menu; double-click semantics; Pause/Step on the sim bar.
- P4-B: numeric time axis + dataZoom; variable tree (`instance.port`); Data table; Run param snapshot/diff; Results checkboxes write `loggedVariables`; Dashboard pins + Apply and Run.
- P4-17: integrator output rows feed `LiveSeriesHandle`; `studio-sim-progress` pushes incremental `liveTime`/`liveSeries` every 250ms so Results grows during Run (Pause stops new points).
- P4-18: Simulation Configuration dialog (tolerance, step limits, solver descriptions; CVODE/IDA disabled without sundials).
- P4-19: Messages split Compiler / Advisor / Connection; advisor rules and fix hints are i18n; click focuses the block.
- P4-22: Catalog params carry `unit`/`description`; Properties shows Ohm next to `Resistor.R`; Dashboard knobs show the same unit.
- P4-20: Welcome page lists the last 8 projects; window close and project switch confirm when dirty.
- P4-21: Opening an example paints the diagram first; Validate runs in the background and can be cancelled.
- Welcome is an 800x600 modal (hero image + example cards); toolbar Home reopens it.
- Welcome bottom tabs: Examples, Open (browse + recents), New (name, parent folder auto-creates `{parent}/{ModelName}`, blank or example template, initial sim config).
- Non-toolbar buttons show icon + text; menu/tool bars stay icon-only. Dialog corner close stays an X.
- New project wizard: Ctrl+N and File > New open the New tab instead of picking a folder immediately; refuse overwrite of an existing Studio project or a non-empty folder.
- Save with no project folder reuses the same wizard (name + parent auto-creates `{parent}/{ModelName}`); the current canvas is written, templates are hidden.
- Dragging a block and releasing reroutes wires attached to the moved block(s) (same orthogonal router as Align / Reroute).
- Unsaved sessions: Run / Update / Export open the save wizard instead of simulating with no project folder.
- Connecting a wire highlights compatible ports and uses a larger snap radius.
- Properties Name / Type / size / label-below / Ref labels are i18n.
- Rotate 90 degrees moves ports to the new sides and reroutes attached wires (no CSS-only spin).
- Title bar File / Edit / Export / Simulation are real dropdown menus.
- Dirty projects auto-save to disk every 30s when a folder exists; a local crash draft can be restored after an unclean exit.
- Bundled templates expanded to 21 (keep the previous 9, add first-order/PID/TF/limiter/switch/goto, RL/RLC, rotational drive/gear, mass-damper, StateGraph first). Welcome Examples and New-project template list share the same set, grouped by domain (signal / electrical / mechanical / multibody).
- Scrollbars follow the Tauri window theme: `fluentOverlay`, `color-scheme`, and `setTheme` on theme change.
- Fill empty bundled example diagrams: RotSpringDamper, TransSensorControl, TransSpringDamper (stubs had `nodes: []`; trans models were emit-empty).
- Canvas wire legend, sample-time legend, and minimap dock to the edge: hover or click the tab to show; pane click hides them back to the arrow.
- Drop the root canvas breadcrumb title; align tools stay enabled after selecting 2+ blocks (Ctrl+click / Shift+drag).
- Fix canvas height collapse after hiding the root breadcrumb (`editor-column` no longer uses a fixed 6-row grid).
- Shift+drag box-select no longer rebuilds the React Flow graph on every hover (that unmounted the UI).
- Connector ports expose both source and target handles so flange / flange_a wires no longer trip React Flow error 008.
- Canvas graph is derived (no useNodesState/useEdgesState sync effects), so React Flow StoreUpdater no longer hits Maximum update depth exceeded.
- UI type scale: body uses `--studio-font-size`; titles are one pixel larger (`--studio-font-title`).

### Fix Multibody pendulum rest (Frames / Axis / gravity) (2026-08-14)

- Flatten: `Types.Axis` expands as `Real[3]` (was collapsed to scalar `Real`, so `n={0,0,1}` vanished). `TypeReal` aliases split as primitive arrays.
- Flatten: vector calls (`resolve1`, `planarRotation`, `gravityAcceleration`, `normalize`, ...) index the result (`f(args)[i]`), not each argument.
- Flatten: do not const-fold `Real[3]` literals to the first element (was `{0,-0.5,0}` → `0`, killing rod length and axis).
- BLT: rewrite whole-array names in Calls when all components alias (`resolve1(rod_frame_a_R, r)` follows `frame_b.R`).
- BLT: Frame `R` / `r_0` Variable=Variable copies alias onto Call/If-assigned kinematics; substitute aliases inside `if` so `absoluteRotation(frame_a.R)` follows `world.frame_b.R`. Do not alias Call-assigned `R` away (that left `resolve1` at 0).
- Flatten: expand `zeros(n)=rhs` / `ones(n)=rhs` into scalar `rhs[i]=0|1` (FixedTranslation force/torque balance was a Call LHS skipped by JIT).
- Connect: potential equalities emit `b.member = a.member` so the second argument is assigned from the first.
- Analyze: fold `if Connections.rooted` / `isRoot` / const `if not(0)` into the taken Simple equations so BLT orders `frame_b.R = absoluteRotation(...)` before `r_0 = resolve1(R, r)` (those Ifs were lowered to algorithms and ran after `resolve1`, leaving `body.r_0` at the identity).
- BLT: `resolve1(R, r)` / `absoluteRotation(R1, R2)` mention array bases; expand those unknowns to `R_1..R_n` so orientation producers run first.
- Analyze: `der(algebraic)` of Frames `resolve1`/`resolve2` becomes `cross(w, r)` using `planarRotation`/`absoluteRotation` `R.w` (`e * der(angle)`); second `der` is `cross(der(w), r)` so gravity torque enters `der(w)`. `angularVelocity2` no longer returns quaternion components.
- Flatten: `I*z` 3x3 tensor-vector product indexes by row (`(I*z)[i] = I_i·z`), not element-wise `I[i]*z[i]` (was `I_33=0`, killing revolute gravity torque).
- Analyze: expand Real[3] `v1*v2` to `e · t`; when `constantTorque.tau = 0`, keep `0 = e · t` as a residual so Newton/BLT assign axis torque from the rod, not `revolute.tau`. Expand `cross(r, f)[i]` to scalars so the lever arm constrains `f` / `der(w)`.
- Analyze: scalarize `Frames.resolve1`/`resolve2` with the quaternion formula so `a_0 = der(v_0)` carries `der(w)` into `f = m(a-g)` (was compiling nested `resolve1[...]` as 0, dumping gravity into `I_33*der(w)`). Bind `internalAxis(tau=tau)` so the flange sum sees the joint scalar.
- Refactor: extract BLT SCC emit (`sort_graph`), Frames helpers (`map_if_eq` / `walk_expr_any`), flatten composite finish (`flattener_instance.rs`), and `NotFound` alias-load recovery (`flattener_load.rs`) without changing algorithms.
- Refactor: split every `jit-compiler` / Studio backend Rust file that was >=800 lines into sibling modules (compiler options, flatten expand, compile entry phases, JIT call/policy/blocks, loader scan, CLI/driver/sundials/native/FMI, catalog/commands) without changing algorithms.

### Fix Multibody preview empty result curves (2026-08-14)

- Example `multibody_preview`: use a gravity-driven `Revolute` pendulum (`phi(start=0.5)`) instead of a `Prismatic` along z (orthogonal to gravity, rest at zero).
- Studio plotHints: match compiler flattened names (`revolute.phi` → `revolute_phi`, `body.r_0[1]` → `body_r_0_1`) so Results actually draws the hinted series.
- emit_mo: nested modifiers `phi` + `(start = 0.5)` emit as `phi(start = 0.5)`.

### Fix Multibody index-2 selection path (2026-07-25)

- Classify: demote Multibody Body absolute pose/velocity when joint DOF states exist (MSL joint prefer / body avoid); also demote nested torque-source `phi`. Disable body demote with `RUSTMODLICA_MB_STATE_DEMOTE=0`.
- Classify: recognize short Multibody joint names (`b1_phi`, `cylinder_s`, `bearing_*`, `inertia_*`) and demote sensor `der*`/`_u` intermediates; in planar cut loops keep the lowest-index `revolute*` DOF.
- Classify: free-flying bodies (no joint DOF) demote redundant Euler `phi`/`phi_d` when quaternions `Q` are selected, and demote nested Body pose duplicates under BodyShape.
- Classify: register demoted body vars and their `der_*` as algebraic outputs.
- BLT alias: do not collapse Call/If RHS into aliases; do not rewrite `der_*` / `der(x)` into `der(Frames....)` (was F2-1 on DoublePendulum).
- JIT: resolve `der_x` from algebraic outputs when `x` is no longer a state (dummy-derivative / demoted path).
- BLT index reduction: prefer Body absolute / `r_0`/`v_0`/`w_a` as dummy-derivative candidates over joint DOF states; demote selected states out of the integrator set and sync `layout.state_vars`.
- BLT: continue dummy-derivative rounds while states are demoted (was stopping when unassigned count did not drop).
- CLI: default `--index-reduction-method` to `dummyDerivative` (was `none`) so Multibody constraints are reduced without a manual flag.
- BLT matching: prefer `0 = f(u)` residuals (planar cut / force) before greedy `u = Call(...)` LHS assignment so loop closures keep free unknowns.
- BLT: unmatched residuals with no state dependence are classified as algebraic loop closures (index-1), not differential index-2.
- Classify: recognize RobotR3-style `rN_phi`/`rN_w` joint DOFs; demote `q`/`qd` array mirrors, sensor flange/speed, and gear/motor mechanical DOFs when Multibody revolutes are selected.
- BLT: on dummy-derivative matching regression, restore the prior equation set and rematch instead of keeping the worse round.
- StateSelect: parse/preserve `stateSelect` on flattened Reals (`Declaration.state_select`); classify demotes `Never`/`Avoid` when `Prefer`/`Always` states exist (`RUSTMODLICA_STATE_SELECT=0` disables).
- StateSelect: alias demote and BLT dummy-derivative prefer Prefer/Always ranks over name heuristics.
- StateSelect: Prefer ranks also drive Multibody "has joint DOF" presence; Body absolute name demote remains as fallback until Avoid attrs fully survive flatten.
- Flatten: expand Body/Joint/Frame/Orientation on every instance; other types still once-only (FullRobot size).
- Flatten/parser: preserve `type X = Real[n]` array dims; expand Orientation to Real scalars with `each stateSelect`.
- Classify: demote `*_frame_a_r_0_*` when paired body `*_r_0_*` carries Avoid/Never; also simple `a=b` partners.
- Classify: when Prefer/Always joint DOFs exist, demote Default-ranked Multibody body absolutes (covers aggregates missing Avoid); `RUSTMODLICA_MB_STATE_DEMOTE=0` no longer disables that Prefer+Default rule (only drive-motor legacy extras).
- Classify: kinematic-loop Fourbar (Always + >=3 joint instances, or UniversalSpherical / JointSSP / planar cut): keep Always DOFs, demote Prefer loop joints and rod internals (`rRod_0`, `prismatic_distance`). Does not collapse independent parallel joints (SpringMass).
- BLT: unmatched residuals that still mention algebraic unknowns are torn kinematic loops (index-1), not `g(x)=0` index-2; dummy-derivative will not demote `StateSelect.always` states.
- BLT: simplify `x - x` residuals to 0 so flattened output aliases (`j1_phi = j1.phi`) are not reported as index-2 constraints.
- Classify: when a `Sources.Position`/`Accelerate`/`Move` state shares a flange alias set with a joint or mass, keep the prescribed-motion source and demote the rest (Surfaces `position_*` vs `prismatic_*`).

### Fix 1D mechanical index-2 from gear/force/rolling aliases (2026-07-25)

- Classify: demote LossyGear `phi_a` / Force `s` / flange sensor potentials when they alias absolute mass/inertia states (without treating `*_rel` as keepers).
- Classify: demote IdealRollingWheel cross-domain pairs `(phi-support)*radius = (s-support)` so rotational and translational absolutes are not both integrators.
- Classify: split state-demote helpers into `classify_demote_{alias,relative}.rs` (`classify.rs` was over 1000 lines).
- BLT: dependency edges are producer->consumer so `a = tau/J` is scheduled before `der_w = a` (was reversed; First/RollingWheel stayed at 0 with applied torque).

### Fix RateTransitionDemo all-zero curves (2026-07-24)

- Flatten: do not instance-prefix imported `pi`/`eps` (was `sine1_pi` -> 0 -> `sin(0)`).
- JIT: do not namespace-passthrough `Modelica.Math.sin` (and other scalar math); call native sin.
- Flatten clock_infer: `sampleTrigger = sample(...)` is continuous Boolean, not clocked state.
- JIT: evaluate continuous algebraic equations before when-algorithms so sampleTrigger is visible to when edges.
- Classify: only when-assigned vars are discrete from `pre`/`previous` (UnitDelay `pre(u)` no longer freezes continuous `u`).

### Fix Studio algebraic / electrical example zeros (2026-07-24)

- Simulation: algebraic-only models (no states / when / crossings) now call `calc_derivs` each step instead of copying start values (Blocks.Sine, SineVoltage aliases).
- Solvers: empty state vector still evaluates algebraics (Euler / BE / RK4 / RK45).
- Classify: undeclared equation refs become algebraic outputs, not frozen parameters at 0 (connector pins).
- Flatten connect: synthesize connector potential/flow member decls when instances lack expanded members (electrical Pin `v`/`i`, rotational Flange/Support `phi`/`tau`, translational Flange `s`/`f`) so potential/flow equations are emitted.
- BLT alias elim: do not collapse states or `v = der_*` kinematic links (was removing `w = der(phi)` so `der_phi` stayed 0).
- BLT matching: prefer assigning `der_*` unknowns for `v = der_s` equations.
- Classify: demote connector-potential states equated to a real component state (SpeedSensor `flange.phi` vs `inertia.phi`).
- Classify: demote relative `phi_rel`/`w_rel`/`s_rel`/`v_rel` when flanges resolve to absolute states/constants, including affine `flange_s = mass_s +/- L/2` (Damper/SpringDamper index-2).
- Classify: when connect equates flange roots without `_s`/`_phi`, also union potential members so translational sensors/SpringDamper relatives demote (avoids index-2 rebound).
- `expand_der_linear`: support `der(x/c)` for constant `c` (Mass `s +/- L/2` after alias rewrite).
- Flatten connect: expand potential/flow from either connector side (was a-path-only root fallback); synthesize `s`/`f` vs `phi`/`tau` from Translational/Rotational owner types so Damper/SpringDamper no longer get bare `flange_a = flange_b` roots.
- Flatten connect: only infer flange potential/flow on `*_flange*` / Support paths (was synthesizing `torque1_tau_phi` from Rotational owner and zeroing mechanical sims).
- Flatten: split `connections.rs` into `connections/{mod,expand,stream}.rs` (file was over 1000 lines).
- Classify: demote AccSensor-local `v`/`w` when they duplicate an absolute mass/inertia velocity (`Translational.Examples.Sensors` index-2).
- Classify: attach PartialRigid/Rod `flange = s +/- L/2` affines so ElastoGap/Damper relatives resolve across rod chains (`PreLoad`).
- Studio examples: `trans_sensor_control`, `rot_spring_damper`, `trans_spring_damper`.
- BLT alias subst: rewrite `der_x` through transitive potential aliases (`der_flange` -> `der_inertia_phi`); expand `der(0-x)`.
- Example: `rc_circuit` is a real series StepVoltage-R-C-Ground (was lone unexcited Capacitor).
- Example: `rot_sensor_control` rotational flange connects + der(phi) assignment (inertia/sensor/scope no longer stuck at 0).

### Modai Studio F90 Export Suite (2026-07-24)

- ExportWizard: Coder / FMU CS / FMU ME; model identifier + GUID; expected artifact preview; last options in `settings.lastExport`.
- Backend `studio_emit_artifacts`: `fmiKind` + `FmiExportOptions`; package kinds include `fmu-me` / `coder+fmu-me`.
- Results dock **Export** history (`EmitRecord`) with open folder / reveal FMU via opener.
- Headless gate: `--smoke-emit-example=<id>` (Coder + FMU CS + FMU ME). Board: `docs/modai-studio/tasks_f90.json` (P563–P574).

### Modai Studio Rate Transition / multi-rate (2026-07-24)

- Catalog: `blocks.discrete.rateTransition` (ZOH) + `rateTransitionUd` (UnitDelay); Configurable discrete family; RT Mask + `RT Ts=` badge.
- Advisor `sampleRateMismatch` when discrete samplePeriod differs without a Rate Transition bridge.
- Example: `examples/rate_transition`. Task board: `docs/modai-studio/tasks_rate.json` (P556–P562).

### Modai Studio F89 Profiler (2026-07-24)

- `SimProfile` on `studio_simulate`: emit / compile / apply_params / integrate ms; Fast Restart marks compile skipped.
- Progress phases `emit` / `compile`; Results dock **Profiler** tab (table + bar chart); profile stored on RunRecord.
- Task board: `docs/modai-studio/tasks_f89.json` (P547–P555).

### Modai Studio Q4-C routing + icons (2026-07-24)

- Auto orthogonal routing with obstacle avoidance + feedback wrap-around (`edgePath.ts`); toolbar Reroute (selected edges or all).
- Edge label show/hide + vertical offset in Settings; Mux/Bus face dimension badge (`n→m`).
- Mask coverage: secondOrder/min/max/noise/stateSpace; wrap/MSL/domain Icon fallback (no empty placeholder).

### Modai Studio Q4-B sensors + mix examples (2026-07-24)

- F71 sensor catalog: Potential/Power, rotational Speed/Acc/Rel*, translational Speed/Acc/Rel* (`catalog/sensors.rs`).
- Bridge convention: sensor RealOutput -> signal (no converter block); connect infers unit/dimension; Advisor domain/unit hints.
- Examples: `rot_sensor_control`, `elec_sensor_control`.

### Modai Studio Q4-A trust gates + observation (2026-07-24)

- Catalog FQN inventory + gate CLI (`--dump-catalog-gate=` / `--run-catalog-gate=`) and `run_catalog_gate.ps1`; signal/physical/routing core Analyze green.
- Emit golden checks (`--check-emit-golden=`): subsystem prefix, inactive Variant absent, Goto/From connect.
- Validate Messages tag `[fallback]` for placeholder/fallback noise.
- Observation: Scope sink, edge quick-plot (F51), Display end value (F52), Results cursor readout (F53).

### Modai Studio Q4 task board (2026-07-24)

- Next phase board: Tier A trust gates + observation (P519–P530), Tier B sensors/mix examples (P531–P538), Tier C routing/icons (P539–P546).
- Canonical JSON: `docs/modai-studio/tasks_q4.json`; runner: `run_next.ps1 -Board q4`.
- Deferred on this board: F89 Profiler, Rate Transition, FMU export polish.

### Modai Studio P499–P518 signal attrs + domain edge styles (2026-07-24)

- Edge `unit` / `dimension` annotations; label shows `name [unit]`; Properties panel when a wire is selected.
- Domain wire styles (F72): signal arrow/thin, electrical thick solid, mechanical/multibody dashed; inferred from source port/domain.
- Canvas domain legend; selected edge highlight.

### Modai Studio P479–P498 Translational + Multibody joints (2026-07-24)

- Translational 1D (F70): Mass, Spring, Damper, SpringDamper, Fixed, Force, Speed, Position, sensors, IdealGearR2T.
- Multibody (F74): Prismatic, Spherical, Universal, FreeMotion, FixedRotation, BodyCylinder, PointMass, actuated Revolute/Prismatic.
- Configurable variant groups for translational passives and multibody joints; Mask icons and library labels.

### Modai Studio P459–P478 Variant Subsystem (2026-07-24)

- F87 Variant Subsystem: `ui.variants` + `activeVariantId`; `subsystemRef` always mirrors the active child.
- Add Variant clones the active child diagram; Properties switch / rename / remove; toolbar menu action.
- Emit expands only the active choice; Advisor warns on mismatched variant port sets; dashed V face badge.

### Modai Studio P439–P458 physical library + Logical (2026-07-23)

- Analog expand (F70): SineVoltage, PulseVoltage, ConstantCurrent, IdealDiode, IdealClosing/OpeningSwitch.
- Rotational expand: Fixed, SpringDamper, IdealGear, Speed, AngleSensor, TorqueSensor; mechanical domain coloring.
- Logical And/Or/Not/Switch + BooleanToReal / RealToBoolean; Configurable logical variant group.
- Sample-time legend on canvas (F73); Mask icons for new blocks.

### Modai Studio P419–P438 Configurable + Discrete + sample-time colors (2026-07-23)

- Multi-select Configurable (F63): same-family type switch + bulk param Enter-to-apply.
- Catalog Discrete: Sampler / ZOH / FOH / UnitDelay / Discrete TF; Lookup2D (`CombiTable2Ds`).
- Sample-time coloring (F73): dashed discrete face, orange stripe, `Ts=` badge; Properties sample hint.
- Variant groups `discrete` / `tables` / `booleanSources`; Mask icons for discrete and Lookup2D.

### Modai Studio P399–P418 Model Reference + user catalog pack (2026-07-23)

- Wrap .mo branded as **ModelRef** catalog; copies into `wrapped/` and adds library path.
- Emit prepends referenced non-MSL class sources from `wrapped/` / `userlib/` for self-contained `.mo`.
- Validate/simulate auto-inject project `wrapped` + `userlib/*` library paths.
- User catalog packs: `catalog.json` + `.mo`; Import Pack / Export Pack menu; UserLib category + Ref icon.

### Modai Studio P379–P398 advisor + structure + PNG (2026-07-23)

- Model Advisor (F80): unconnected ports, orphans, missing Ground, Goto/From tags, empty subsystems, signal cycles; merged into Update Messages.
- Structure diagnostics (F81): tag compiler algebraic/singular messages with `[structure]`; clickable warnings focus blocks.
- Export PNG (F82) via SVG rasterize; menu toggle in settings.

### Modai Studio P358–P378 param dictionary + From File (2026-07-23)

- Project `paramDictionary` (F85): block params use `$Name`; resolved at emit / Fast Restart overrides; Data activity panel; Properties Promote; Dashboard dict knobs; loggedVariables editor.
- From File (F86): `CombiTimeTable` catalog entry; Load CSV embeds `table=[...]`; Clock + Boolean sources; Mask icons.
- Menu Export SVG for diagram snapshot; validate/simulate/emit pass `paramDictionary`.

### Modai Studio P338–P357 signal library + edit UX (2026-07-23)

- Catalog: Math (Product/Division/Abs/Sqrt/Sign/trig), Nonlinear (Limiter/DeadZone), Sources (Ramp/Pulse/ExpSine), Tables Lookup1D, Continuous (TF/Derivative/PI/PID), Sinks Display.
- Emit formats array/enum/string modifiers safely (`format_param_value`).
- Library favorites (localStorage); expanded Type variant groups; Mask icons for new blocks.
- Rotate selection 90 deg (toolbar / Ctrl+R); canvas Find (Ctrl+F); port hover type tips.

### Modai Studio subsystem boundary In/Out (2026-07-23)

- Create Subsystem promotes cut edges to child Inport/Outport terminals and parent face ports `In1`/`Out*`.
- Navigating up, Save, and Validate sync parent subsystem ports from child Terminals.
- Subsystem block: double-frame face, colored In/Out handles, port labels; Properties lists boundary bindings.

### Modai Studio Goto/From + labelBelow + edge vertices (2026-07-23)

- Library **Routing** Goto / From: `RealPassThrough` with matching `tag`; emit adds `connect(goto.y, from.u)` (tag is diagram-only, not a Modelica modifier).
- Properties **Label below** stores `ui.labelBelow` and draws the instance name under the block.
- Selected edges show editable orthogonal vertices (Alt+click / double-click wire to add; drag; double-click vertex to remove); persisted on `edge.vertices`.

### Modai Studio Mux/Bus thick bars (2026-07-23)

- Library **Routing**: Mux 2–4, Demux 2–4, Bus 2–3 as thick vertical bars.
- Emit maps to `Modelica.Blocks.Routing.Multiplex*` / `DeMultiplex*` (scalar `n*=1`).
- Properties Type switch resizes the bar and ports; Bus is Multiplex with thicker styling.

### Modai Studio In/Out terminal tags (2026-07-23)

- Library category **Terminals**: Inport (blue) / Outport (orange) color-tag blocks.
- Maps to `Modelica.Blocks.Interfaces.RealInput` / `RealOutput`; emit uses connector `self` port (`connect(in1, gain.u)`).
- Properties `portNumber` shown on the tag face.

### Modai Studio Full JIT validate toggle (2026-07-23)

- Messages show `Analyze only` / `Full JIT` from validate stage; Analyze success adds a hint line.
- Toolbar **Full JIT** (Cpu) toggle; when on, Update / open-example validate with Full compile + codegen.

### Modai Studio validate Analyze stop (2026-07-23)

- `studio_validate` / `validate_model` stop at `CompileStopPhase::Analyze` (no JIT codegen).
- Run / simulate still uses Full compile; opening examples and Update are much faster.

### Modai Studio validate off main thread (2026-07-23)

- `studio_validate` is async + `spawn_blocking` so opening examples / Update no longer freezes the UI during JIT compile.
- Example open yields two animation frames so the diagram paints before validate; status shows validating.

### Modai Studio edge labels + junctions (2026-07-23)

- Custom `studioStep` orthogonal edges with short signal-name labels (full name in tooltip).
- Fan-out from the same source port draws a filled junction dot; only one label per branch group.

### Modai Studio wrap Icon face (2026-07-23)

- Wrap `.mo` parses `annotation(Icon(...))` into catalog/node `icon`.
- `IconFace` renders Modelica graphics on the block; falls back to Mask glyphs when absent.

### Modai Studio wrap .mo auto ports (2026-07-23)

- `studio_wrap_mo`: parse Modelica model/block, auto-extract multi in/out (and connector) ports + parameters into catalog **Wrapped** entry.
- Menu **Wrap .mo**; place on canvas; Properties edits params; Run options stay project-level.

### Modai Studio block Mask icons (2026-07-23)

- Catalog SVG glyphs on blocks (Gain triangle, Add/Feedback, Integrator, Step/Sine, R/C/L, …).
- Visual mapping doc: `docs/modai-studio/09-visual-graphics.md` (screenshot → 06 capability map).

### Modai Studio Draw NodeResizer (2026-07-23)

- Draw nodes: select then drag corner/edge handles (`NodeResizer`) to resize; size written to `ui.width/height`.
- Snap-to-grid applies on resize end when snap is enabled; Properties width/height still work.

### Modai Studio Draw graphics library (2026-07-22)

- Library category **Draw**: Rectangle, Ellipse, Line, Polygon, Text, Note (`Studio.Draw.*`).
- Canvas-only `nodeKind: "draw"` (no ports); skipped by `emit_mo`; properties for size/stroke/fill/text.

### Modai Studio Simulink-like block diagram (2026-07-22)

- Canvas blocks: sharp rectangular frames, domain stripe, side port rows with labels (signal triangle / connector square).
- Orthogonal step edges with closed arrow markers; dual line grid (20/100); snap 10x10.
- Styles in `styles/simulinkBlocks.css`; replaces flowchart-card aesthetics.

### Modai Studio example auto-validate (2026-07-22)

- Opening an example runs Validate automatically.
- On success: switch to Results and pre-select `plotHints` (`instance.hint`); Run prefers those vars.

### Modai Studio classic theme manager (2026-07-22)

- Classic themes: Dark / Light / Classic Blue / High Contrast (`settings/themes.ts`).
- Settings General theme swatches; shell/canvas/charts adapt via CSS tokens (`data-theme`).

### Modai Studio example open / visualize / test (2026-07-22)

- Bundled examples: `signal_feedback`, `rc_circuit`, `multibody_preview` (diagram + `.mo`).
- GUI: File **Examples** menu + welcome overlay to open examples for canvas + Run.
- Headless: `--smoke-example=<id>` / `--smoke-all` open+validate+simulate; `test_studio_autoload.ps1`.

### Modai Studio auto-load smoke (2026-07-22)

- **GUI**: `MODAI_STUDIO_OPEN` or `--project=<dir>` auto-opens a Studio project on startup.
- **Headless**: `--smoke-open=<dir>` opens project and prints JSON `{ok,rootDir,modelName,nodes,edges}`.
- **Script**: `pwsh -File ./test_studio_autoload.ps1` (unit + smoke; launches GUI unless `-NoGui`).

### Modai Studio frameless VS Code shell (2026-07-22)

- **Frameless**: Tauri `decorations: false` + custom title bar (drag, min/max/close).
- **Workbench**: Activity bar | primary sidebar | editor | secondary sidebar | bottom panel | status bar.
- **Views**: Explorer (Library/Outline) vs Properties/Dashboard; Ctrl+B / Ctrl+J / Ctrl+Alt+B toggles.
- **Persist**: `layout.activity`, `showSidebar`, `showPanel`, `showSecondary` in settings.
- **Split**: `useStudioSession` (composer) + `useStudioProject` / `useStudioEdit` / `useStudioSim` + `Workbench`; `App` is thin.
- **Tauri versions**: pin `@tauri-apps/api`/`cli` to 2.11.x; bump `tauri` crate to 2.11.x to match.

### Modai Studio shell UI settings (2026-07-22)

- **Iconized menus/tools**: lucide icons for menu bar + toolbar; optional labels; grouped File/Edit/Export/Sim.
- **System settings**: locale (zh/en), font family/size, density, tips/shortcuts, menu/tool visibility; persisted in `localStorage`.
- **Shortcuts**: Ctrl+N/O/S, Ctrl+Enter (Run), plus existing Undo/Redo/Copy/Paste; tips via `title` when enabled.
- **i18n**: shell + panel/tab titles via `src/i18n`; save syncs `manifest.ui.locale`.
- **Layout**: drag-resize left/right panels, Library/Outline split, Results dock height; sizes in settings (`layout`) and CSS vars.
- **Status i18n**: run/validate/emit/edit status and error strings via `t`/`tf` (zh/en).
- **Shortcuts help**: Settings dialog Shortcuts tab; press `?` to open; Esc closes.
- **Progress i18n**: map known sim progress phases/messages (`started`/`running`/`done`/`fmu_cosim`) via `localizeProgress`.
- **Sim hotkeys**: Space pause/resume, `.` step, Ctrl+`.` stop; listed on Shortcuts page.

### Modai Studio P0 MVP (2026-07-21)

- **App**: new workspace member `modai-studio` (Tauri 2 + React + XYFlow) — Library | Canvas | Properties | Results shell.
- **Backend**: project open/save (`studio-project.json` + `*.mo` + `*.diagram.json`), builtin catalog (14 blocks), `emit_mo`, `studio_validate` / `studio_simulate` via `rustmodlica` with default MSL root under `jit-compiler`.
- **Examples**: `modai-studio/examples/signal_feedback`, `rc_circuit`.
- **Tasks**: `docs/modai-studio/08-implementation-tasks.md`, `build/modai_studio/tasks.json` (S01–S09 done).

### Modai Studio P1 Core (2026-07-21)

- **Sim control**: async `studio_simulate` with `SimControl` cancel (`studio_cancel_simulation`) + `studio-sim-progress` events (`simTime`/`tEnd`/fraction); UI Stop + progress bar.
- **Edit**: Undo/Redo, Copy/Paste, Snap grid, Align L/T/H/V; Outline panel; edge signal labels; invalid edges red + Messages click-to-focus instance.
- **Export**: Results CSV/JSON download (C01–C06 done in `build/modai_studio/tasks_p1.json`).

### Modai Studio P2 Pro (2026-07-21)

- **Subsystems**: Create Subsystem from selection; double-click drill + breadcrumb; `subsystems/*.diagram.json` I/O; emit flattens via instance-name prefix + `portBindings`.
- **Multi-run**: Runs tab stores last 20 successful runs; overlay dashed curves vs latest solid Results.
- **Catalog**: Inductor, VoltageSensor, CurrentSensor, Damper, Torque (Sensors/Analog/Rotational).

### Modai Studio P3 Future (partial) (2026-07-21)

- **Emit**: `studio_emit_artifacts` — C (`model.c`/`model.h`) and FMI 2.0 CS + `.fmu` package via `rustmodlica::fmi`.
- **XY**: Results dock XY phase scatter; Runs show `wallMs` profiler.
- **Dashboard**: pre-run numeric sliders (not mid-sim online tune).
- **Variant**: Properties type switch among Sources/Math/Continuous compatible groups.
- **Out/Blocked**: MathWorks Embedded Coder / certification parity（Studio Coder-grade 包已另条落地）

### Modai Studio coder-grade C package (2026-07-22)

- **Coder package**: beside `model.c`/`model.h`, emit `rml_step.{c,h}` (fixed-step RK4 API), `rml_main.c`, `CMakeLists.txt`, `interface.json`, `INTEGRATION.txt`.
- **Studio**: toolbar **Emit Coder** / **Emit FMU** (coder package default on); IPC `coderPackage` + `packageKind`.
- **CLI**: `--emit-coder=<dir>` sets emit-c + package.
- **Boundary**: not MathWorks Embedded Coder / AUTOSAR / MISRA certification.

### Modai Studio Multibody 3D viewport (2026-07-22)

- **3D dock tab**: three.js scene with OrbitControls, Play/scrub; diagram kinematic preview (World + FixedTranslation chain) and simulation playback from `*.r_0[1..3]` / `*_r_0_*` series.
- **Catalog**: MultiBody World / Fixed / FixedTranslation / Body / BodyBox / Revolute; World emit as `inner`.
- **Example**: `modai-studio/examples/multibody_preview` pendulum-style diagram for preview.

### Modai Studio Fast Restart (2026-07-22)

- **Fast Restart**: session-cached `CompiledModel` on a dedicated worker thread; param-only / sim-option changes reuse JIT Artifacts via `with_params` (no recompile).
- **UI**: toolbar "Fast Restart" toggle (default on); Run status / Runs label show `fast restart` vs `full compile`; IPC `restartMode`.
- **Structure key**: diagram topology + types + param *names* (not values); library path or topology change forces full compile. FMU cosim path unchanged.

### Modai Studio tight DAE/FMU cosim (2026-07-22)

- **Tight co-integration**: single communication loop — plant `calc_derivs` RK4 step + FMI2 CS `doStep` with bridge params `sfmu_*.k`; Pause/Step honored.
- **Fallback**: weak Jacobi two-pass path if tight compile/param binding fails.

### Media.Examples loader + Noise/ComplexBlocks (2026-07-21)

- **Parser**: `redeclare_class_definition` / `redeclare_short_class_definition`; `redeclare_*_extends` accepts `replaceable`/`partial` and `record|package|block|connector` (unblocks giant `Modelica/Media/package.mo`).
- **Strict gate**: Media.Examples **20/20** (incl. Utilities.*); Clocked/Blocks Noise **7/7**; ComplexBlocks+ComplexMath **4/4**. Short-sim samples `fallback_total=0`.
- **Fluid Examples**: bare Medium helpers (`specificEnthalpy_pTX`, `setState_*`, …) mapped to `medium_props`; Fluid Examples leaf models **19/19** strict green.
- **MultiBody Examples**: **63/63** strict green. Electrical `MeasureRiseTime` absent from OM MSL 4.1.0 tree (present in bundled Modelica) — version skew, not a loader defect.
- **Machines/Power**: Machines+PowerConverters+Polyphase Examples **125/125**; QuasiStatic+Batteries **19/19**; short-sim samples `fallback_total=0`.
- **Thermal**: HeatTransfer+FluidHeatFlow Examples **20/20** strict; short-sim `fallback_total=0`.
- **MSL full-green board**: `build/msl_full_green/` — Clocked 87/87, Blocks/Noise 31/31, Complex/Utilities 3/3; ModelicaTest **95/95** after `positiveMax`/`Fluid.Dissipation` builtins + cardinality missing-degree strict0.

### Mechanics + Media strict domain (2026-07-21)

- **Strict gate**: Rotational 30/30, Translational 25/25, Media (resolvable FQNs) 15/15; short-sim samples `fallback_total=0`.
- **Builtins**: `is_equal_fn`, `move_position`, `linearTemperatureDependency` (via `thermal_convert`), `stribeck_table_f0`; `convertAlpha` aligned to MSL `(alpha1, T2, T1=293.15)`.

### Discrete/Tables domain + table fallback tighten (2026-07-21)

- **Strict gate**: Blocks.Discrete / Tables / Noise sample — 98/98 resolvable models pass under `STRICT_PLACEHOLDERS` + `POLICY_STRICT`; short-sim `fallback_total=0`.
- **Table builtins**: `ext_combitable_lookup` replaces warn0 for ExternalCombi*; real `rustmodlica_table_get_value` path no longer increments `jit_builtin` fallback; pre() table placeholders fail-closed under strict.
- **Clock()**: empty-arg legitimate ctor no longer emits false `[fallback:jit-builtin] empty-args` (JSON handler before empty-args warn).

### SimControl cancel + progress (2026-07-21)

- **Engine**: `simulation/control.rs` — `SimControl` cooperative cancel (`AtomicBool`) + throttled `time/t_end` progress; wired through `run_simulation` / `run_simulation_collect` / `simulate_from_source_with_control`.
- **IDE**: `cancel_simulation_run` + `modai-jit-progress.simTime/tEnd`; SimulationPanel Stop; `useSimulation.cancelSimulation`.
- **Product constraints (confirmed)**: full MSL not green; fallbacks remain; no true pause/step; emit-c/FMU ≠ Embedded Coder — Coder stays Out; Pause/Step stays P3.
- **Docs**: `07-compiler-feasibility.md` cancel/progress moved off Blocked.

### Modai Studio specs (2026-07-21)

- **Docs**: `docs/modai-studio/` requirements, design, data structures, features, implementation rationale for a Simulink-like Modelica desktop app (independent of `modai-ide`, XYFlow, dual-file `*.mo` + `*.diagram.json`, path-dep `rustmodlica`).
- **Simulink map**: `docs/modai-studio/06-simulink-capability-map.md`; requirements/features expanded to P0 MVP / P1 Core / P2 Pro / P3 Future; data model reserves subsystem/run/annotation fields.
- **Compiler feasibility**: `docs/modai-studio/07-compiler-feasibility.md` — P0 viable vs jit-compiler; full Simulink parity not feasible (product family / semantics / runtime / compiler maturity); P1 cancel/progress via `SimControl` (true pause still P3).
- **Pointer**: `docs/superpowers/specs/2026-07-21-modai-studio-design.md`.

### Ideal-switch Newton warm-start (2026-07-20)

- **Event-step algebraic guess**: restore pre-trial `outputs` and clamp IdealDiode/transistor complementarity `s` to 0 before the refined step (`prepare_event_step_algebraic_guess`).
- **JIT Newton init**: tearing and dense solvable blocks start ideal-switch `s` at 0; failed Newton no longer write_backs blown-up iterates into `outputs`.
- **Solver recovery**: on status 2 with switch unknowns — homotopy retry, **Backward-Euler micro-steps first**, dt bisection, sticky `dt_active`, short `prefer_implicit_near_switch` window, bounded discontinuity nudge, event-iteration force-`s=0` / fail-open, assert-counter reset.
- **Gate**: DCPM_Drive / SinglePhaseTwoLevel / HBridge / Chopper ExampleTemplates / `ChopperBuckBoost_DutyCycle` short-sim green on **strict** path (`--solver=implicit`, no fail-open).
- **Chopper PWM follow-up**: sync scratch algebraics into `outputs` before trial-eval; reset discontinuity-nudge budget when time advances; floor zero-crossing `dt_event` to avoid PWM edge livelock; prefer BE near ideal switches.
- **Fail-open tightened**: default OFF; continue with `|r|>tol` only when `RUSTMODLICA_SWITCH_FAIL_OPEN=1`, always emitting `[degraded:switch-fail-open]` (+ fallback counter). Accept tol via `RUSTMODLICA_SWITCH_ACCEPT_TOL` (default 1e-5). `STRICT_NEWTON` disables fail-open.
- **BLT tear hygiene**: never Newton on `time` / `*_start` / PWM clock start params; residual-only frozen eqs skipped (fixes DutyCycle `|r|~1` poison path).
- **Simulation split**: `run_simulation` → `simulation/driver/` (`run`, `crossing`, `sched`, `sundials_dispatch`); switch recovery → `switch_drive.rs`; Newton recovery → `newton_recovery/` (`algebraic`, `t0`, `ideal_switch`, …).
- **`compare_omc.ps1`**: `-LibPath`, `-OutDir`, `-CrossSolverSelfGate` (rk4 vs rk45 last-row when no OMC); `-OmcOut` / `*.omc.csv` when reference CSV exists.
- **Docs**: `docs/Ideal_Switch_Simulation.md` (ideal-switch / PWM / `--solver=implicit` / recovery / fail-open policy).

### JIT strict builtin dispatch (2026-07-19)

- **`strict function-builtin` ordering**: hard-fail only on the unknown-import passthrough path so hand-coded handlers (`fill`, `assert`, …) and `try_compile_builtin_call` run first under `RUSTMODLICA_JIT_POLICY_STRICT=function_builtin`.
- **`not(e)`**: compile Modelica boolean `not` Call to Cranelift select (0→1, nonzero→0).
- **Fix `default_function_builtin_rules.json`**: missing comma after `MultiBody.Frames` rule emptied the entire rule table at load time (`unwrap_or_default`).
- **`match_function_builtin_rule`**: still matches under `POLICY_STRICT=function_builtin` (strict only rejects unmatched unknown-import); namespace/capitalized helper passthrough disabled in that mode.
- **Wired builtins**: `zeros`/`ones`/`cat`, `Clock`/`sample`/`interval`/`shiftSample`/`superSample`/`subSample`/`backSample`, `Frames.*` (`frames_orient`), `cross`, `from_deg`/`to_deg`, `gravityAcceleration`, `normalize`/`normalizeWithAssert`, `getNextTimeEvent` (inf-scale, matched before `.Internal.`).
- Strict smoke (OM MSL): Pendulum / DoublePendulum / ForceAndTorque / TanksWithOverflow / Sample1 / AbsoluteClocks / ShiftSample / SuperSample → success.
- Fluid follow-ups: `Medium.*` props, `regRoot2`/`regSquare*`, `*Characteristic`, Dot(`ArrayLiteral`, member).
- **HeatingSystem Cranelift SSA**: `compile_if_stmt` / `compile_when_stmt` snapshot+restore `var_map` per branch so then-arm SSA Values cannot be `stack_store`d in else (verifier non-dominating).
- Follow-on builtins: `Vectors.length`, `anyTrue`/`allTrue`, `maxWithoutEvent`, `WallFriction.*`, table `get*Value*`, `Strings.findLast`, `BaseClasses` passthrough, `from_degC`/`to_degC`.
- Strict subset gate: clocked 87/87, fluid 13/13, multibody 63/63 under `STRICT_PLACEHOLDERS` + `POLICY_STRICT`.
- **Electrical/magnetic strict expand**: JSON builtin rules win over external-import short-circuit; unmatched externals with a rule skip "not linked". Wired Complex/ComplexMath/thermal/space-phasor/`Functions.state*`, conj/transpose/wrapAngle/brushVoltageDrop/quasiRMS/`from_rpm`. Gate: electrical **212/213** (1× model_not_found), magnetic **79/79**.

### P5 simulation numerical kernel (2026-07-14)

- **Newton sparse `auto` production calibration**: `NEWTON_SPARSE_AUTO_MAX_DENSITY=0.35`, `NEWTON_SPARSE_AUTO_MIN_N=8` (overridable via `RUSTMODLICA_SPARSE_DENSITY_THRESHOLD` / `RUSTMODLICA_SPARSE_MIN_SIZE`); unified `RUSTMODLICA_NEWTON_PATH` + `RUSTMODLICA_NEWTON_SPARSE_POLICY` preference.
- **Perf JSON**: `newton_sparse_blocks`, `newton_dense_blocks`, `newton_sparse_nnz_total`, `newton_sparse_policy`, `tierup_step_threshold`.
- **Codegen cache**: variant string includes Newton policy tag (`nwa`/`nwd`/`nws`) so path policy changes miss cache correctly.
- **Tier-up**: `TieringPolicy::for_equation_count` (50/100/200 by size); overrides `RUSTMODLICA_TIERUP_STEP_THRESHOLD`, `RUSTMODLICA_BACKGROUND_TIERUP`.
- **SIMD step**: `RUSTMODLICA_SIMD_STEP` defaults on for RK4 state update (`n>=4`).
- **`--perf-json` sim timing**: record `sim_ms` / `sim_us` whenever `--perf-json` is set (not only with `RUSTMODLICA_PERF_TRACE=1`).
- **Salsa EngineV6 state collapse fix**: query-db `decl_expand_preinherited` now uses `DeclAndSubEq` and passes nested equations/connections into `eq_expanded` (previously root-only eq expand left MultiBody at state=0). Cache stage epochs bumped for DeclExpand/EqExpand/FlatModelQ/FlatFull.

### JIT validate worker (2026-07-13)

- **`--validate-stdio`**: long-lived validate worker; read JSON requests from stdin (`model`, optional `perf_json` / cache artifact paths), emit one validate JSON per request; reuse `ModelLoader` and Salsa session across models in-process.
- **`regress-harness --worker-per-scenario`**: spawn one `rustmodlica --validate-stdio` per scenario and feed the model matrix over stdin (replaces one-process-per-case). A/B on MultiBody: EngineV6 `devloop_multi_model` run2 ~720ms → ~16ms vs legacy subprocess path.

### 审计缺口关闭 (2026-05-09 ~ 2026-05-10)

- **DAE指标约简**: Pantelides默认开启 (`--index-reduction-method=pantelides`)，线性求解器扩展 + 诊断
- **Stream语义**: 修正误导警告，确认JIT端已实现MSL 3.1完整公式
- **Expandable connector**: AST + parser + flatten动态成员注入 + 测试
- **Partial模型校验**: AST + parser + flatten拒绝实例化（修复pest prefix检测bug）
- **SIMD向量化**: 聚类阈值4→2，for循环预展开(≤1024)，Cranelift自动向量化默认开启，跳转表启用
- **FMI导出**: ZIP打包 + C编译自动检测 + 单步导出
- **macOS Mach-O**: 代码缓存重定位支持(x86_64 + ARM64)
- **检查点/重启**: simulation/checkpoint.rs JSON序列化 + 调度器
- **运行时PGO**: simulation/pgo.rs 热方程检测 + 档案持久化
- **解释器**: 限制放宽至20方程/10状态
- **Jacobian着色**: 距离-1图着色，cv_jac+ida_jac集成
- **KINSOL**: 代数初始化 Phase 0 + 事件代数细化
- **IDA**: 代数分量支持(全部按微分处理)
- **断言风暴**: RUSTMODLICA_ASSERT_STORM_LIMIT可配置
- **枚举**: parser→flatten→JIT完整校验管线
- **encapsulated/pure/impure**: AST + parser关键字捕获
- **可解块独立编译**: RUSTMODLICA_BLOCK_COMPILE=1启用，声明+记录+批量编译+call替换+tearing，热替换StandaloneBlock基础设施

### 新求解器 (2026-05-10)

- **Radau IIA(3)**: 2级L-稳定隐式RK(3阶)，简化Newton+LU，自适应步长，`--solver=radau`
- **QSS1**: 一阶量化状态系统，事件驱动多速率，`--solver=qss`

### 测试 (2026-05-09 ~ 2026-05-10)

- Parser模糊测试: 31个property/fuzz测试覆盖边界/畸形输入
- 性能回归基线: perf_baseline.ps1 + 历史对比
- MSL冒烟列表: msl_smoke_models.txt (7个代表性模型)
- QSS + Radau + 块编译端到端验证通过
- 审计报告更新: 19缺口全部关闭/~92%规范覆盖

### Validation (full regression + DIR)

- **2026-04-25/26**: `run_regression.ps1` full (DIR embedded, `target_regression`, quarantine **not** retried) then `run_modelica_dir_regression.ps1` full with **`-RetryQuarantined`**. Main regression log: `build_regression_logs/run_regression_20260425_235632.ndjson`; latest DIR aggregate: `build_modelica_dir_regress/summary.txt` and `build_modelica_dir_regress/dir_metrics.json`.

### Compiler

- **Loader fallback-scan memoization (`loader.rs`)**: `scanned_file_items` caches `parse_all` per file path so the multi-top-level scan and package-directory scan read + parse each source file at most once per process (previously every failed name probe re-read and re-parsed all loaded files; EngineV6 cold validate 141s -> ~5s, ~86% was `qualify_short_type_names` probes hitting these scans).
- **Loader negative name cache (`loader.rs`)**: `absent_name_epoch` short-circuits repeated `NotFound` lookups; keyed on a registration epoch (library roots + loaded models/paths) so any new registration retries, and `forget_model` clears both caches for IDE edits. Non-silent early returns still record `probed_absent` negative deps.
- **Package-root parent registration (`loader.rs` `add_path`)**: when a `--lib-path` points at a package directory (contains `package.mo`), its parent directory is registered as a fallback root, so `Modelica.*` resolution no longer depends on the process working directory (running from repo root previously failed with `Model not found`). `add_path` also de-duplicates roots.
- **Warm-cache JIT keepalive (`compile_model/compile/entry.rs`)**: move the `Jit` into `Artifacts.jit_module_keepalive` instead of dropping it at end of compile; a jit-codegen disk-cache HIT stores its relocated `ExecCodeBuffer` in the `Jit`, and dropping it freed executable memory still referenced by `calc_derivs` (intermittent warm-cache `STATUS_ACCESS_VIOLATION` on large MultiBody models, e.g. EngineV6).
- **Loader / inheritance perf probes**: `RUSTMODLICA_PERF_TRACE=1` prints `[perf] loader.probe ...` (cache-miss calls, fallback scan counts/us) and `[perf] inherit.segments_us ...` (base load, apply-mod, merge, `qualify_short_types`, probe count).
- **Query-db flatten breakdown (`--perf-json` / `RUSTMODLICA_PERF_TRACE=1`)**: `expand_declarations_with_mode` records **`decl_expand_param_pass_us`**, **`decl_expand_array_dim_us`**, **`decl_expand_decl_loop_us`** (summed across recursive `Process` tasks); **`eq_expand_prep_us`** plus **`eq_expand_*_us`** segments inside **`eq_expand_root_preinherited`** (`flatten/mod.rs`). **`CompilePerfReport`** carries the same keys for regression diffs.
- **Decl-expand inheritance template cache (`flatten/mod.rs`, `decl_expand/flattener_impl.rs`)**: **`Flattener.inheritance_flat_template_cache`** stores **`flatten_inheritance`** results keyed by resolved class FQN; repeated instances (e.g. many identical connectors) reuse an **`Arc`** clone before per-instance **`apply_modification_to_model`**. Counters **`inherit_flat_template_cache_hit`** / **`inherit_flat_template_cache_miss`** in **`query_db::perf`** (cold vs repeated loads).
- **Perf JSON inheritance-template counters** (`compiler/mod.rs`, `compile_model/compile/entry.rs`): `CompilePerfReport` now exports **`inherit_flat_template_cache_hit`**, **`inherit_flat_template_cache_miss`**, and **`inherit_flat_template_cache_hit_ratio`**; `RUSTMODLICA_PERF_TRACE=1` prints `[perf] query.inherit_flat_template_cache ...`.
- **Decl-loop segment timing (Stage C)** (`decl_expand/flattener_impl.rs`, `compile_model/compile/entry.rs`): added microsecond split inside `decl_expand_decl_loop` for **`try_load_sub_model`**, **`flatten_inheritance`**, **`apply_modification_to_model`**, and **parameter `substitute+fold`**; exported in perf JSON and trace as `query.decl_expand_decl_loop_segments_us`.
- **Param-pass allocator churn (`decl_expand/param_pass.rs`, `substitute/cache.rs`)**: reuse one **`SubstituteCache`** across fast passes with **`clear()`** between iterations.
- **Nested instance parameter bindings** (`flatten/decl_expand/flattener_impl.rs`): after `apply_modification_to_model` on a loaded sub-model, substitute each parameter `start_value` in the **enclosing** declaration `context` so `final m=m` resolves before the child `Process` task. Improves const array-dimension evaluation for MSL polyphase helpers (e.g. `PlugToPins_*` / connector `pin[m]`) and reduces JIT `Dot(ArrayAccess(...))` flatten gaps.
- **Child parameter const-fold** (`flattener_impl.rs`): when the substituted `start_value` is a compile-time scalar (`eval_const_expr_with_param_exprs`), store `Expression::Number` so modifiers like `final m=mSystems` become numeric before nested array expansion.
- **MSL `numberOfSymmetricBaseSystems`** (`flatten/param_expr_eval.rs`): constant-fold the Polyphase helper (same recursive algorithm as MSL) so models like `QuasiStatic.Polyphase.Basic.MultiStar` can size `PlugToPins_n(final m=mSystems)` without leaving unevaluated array dimensions.
- **Decl-expand cache epoch** (`cache/ir_epoch.rs`): bump `DeclExpand` stage epoch to **3** so existing flatten disk/SHM rows are invalidated after declaration-expansion / import-resolve tweaks.
- **Sub-model load candidates** (`flatten/import_resolve/mod.rs`): for types already rooted at `Modelica.*`, skip lexical parent qualification in `build_load_candidates` (avoids invalid `...Utilities.Modelica.Electrical...` probes and catastrophic flatten time on large MSL examples such as `ControlledDCDrives.*`).
- **Fluid vs StateGraph connector aliases** (`flatten/import_resolve/domains_misc.rs`, `loader_compat.rs`): map `CompositeStepStatePort_in` and `Modelica.Fluid.Interfaces.CompositeStepStatePort_in` to `Modelica.StateGraph.Interfaces.CompositeStepStatePort_in` (fixes `NormalOperation` / `TankController` `FLATTEN_UNKNOWN_TYPE`).
- **Array dimension fast-pass complexity** (`flatten/decl_expand/array_dim.rs`): score `Expression::Dot` as `1 + complexity(inner)` instead of the generic high cap so simple member accesses (e.g. `plug_p.m`) are not marked uncalculable without trying `eval_const_expr_with_param_exprs`.
- **StateGraph / Complex connector heuristics** (`flatten/utils.rs`): treat `Step_out_forAlternative` / `Step_out_forParallel` with `Transition_in` as compatible; allow `Complex` with `ComplexInput` / `ComplexOutput` for `ComplexMath.Bode`-style connects.
- **Import-scoped type qualification guard** (`flatten/utils.rs` `is_import_scoped_or_unit_type_name`): `qualify_short_type_names` no longer prepends enclosing package scopes to `SI.*`, `NonSI.*`, `Machines.*`, `Interfaces.*`, and other import-alias prefixes. Fixes `FLATTEN_PARTIAL` false positives where `SI.Reluctance` was mis-resolved to the partial icon `Icons/Reluctance.mo` (FluxTubes ~15 DIR failures) and `FLATTEN_INCOMPATIBLE_CONNECTOR` from duplicated prefixes such as `...Interfaces.Machines.Interfaces...` (e.g. `DCPM_Temperature`).
- **NonSI global alias** (`flatten/import_resolve/domains_misc.rs`): map `NonSI` / `NonSI.*` to `Modelica.Units.NonSI.*` so Fluid pump models (`NonSI.AngularVelocity_rpm` in `PartialPump`) validate (12+ `ModelicaTest.Fluid.TestComponents.Machines.*` + `Modelica.Fluid.Examples.*` DIR failures).
- **Newton tearing stack slot fallback** (`jit/translator/equation/solvable_tearing.rs`): allocate a local stack slot when the tearing variable was not pre-registered in `stack_slots` (fixes JIT panic `Tearing var must have stack slot` on `Modelica.Fluid.Examples.PumpingSystem` simulation).
- **Magnetic FW `Machines.Losses` alias** (`flatten/import_resolve/domains_magnetic.rs`, `domains_misc.rs`): route `Machines.Losses.*` / `Machines.Interfaces.*` / `Machines.Thermal.*` inherited from `Modelica.Electrical.Machines` to `Modelica.Electrical.Machines.*` instead of `Modelica.Magnetic.*.BasicMachines.*` (fixes 10 SMPM FundamentalWave/QuasiStatic DIR regressions after the import-scoped qualification guard).
- **Function root entry eval (F3-1)** (`compiler_impl.rs` `run_function_once`): tries **each** output expression in declaration order for scalar `eval_expr` (so a later `output Real s := ...` still yields a value when an earlier output is array/comprehension-only). If **all** outputs fail only with **array/dot/range** or **unknown variable** (cross-output references at entry), returns **0.0** instead of aborting full compile—aligns non-`validate_only` behavior with TestLib multi-output functions.

### `regress-harness` + ModelicaTest mirror

- **`runner.rs`**: `kind: model` runs now pass **`case.env`** into the child (`rustmodlica` or `cargo run`), fixing ignored per-case `env` (used for JIT-cache/AOT workarounds on Windows). **`discover_rustmodlica_exe`**: pick the **first existing** candidate path (JIT `target_regression`, then `jit-compiler/target`, then workspace `target`) instead of **newest mtime**, so a fresh workspace build does not override the regression binary. **`run_mos`**: add **`--release`** for `cargo run` script mode; forward **`case.env`**.
- **`regress_smoke_v2.json`**: **`s_evt_discrete`** sets **`RUSTMODLICA_JIT_CODEGEN_CACHE=0`** and **`RUSTMODLICA_AOT_NATIVE_LOAD=0`** to avoid intermittent **`ACCESS_VIOLATION`** when a cached JIT object is loaded for `TestLib/DiscreteTest` (when + reinit).
- **`regress_full.json`**: **`f_lang_algorithm_else_when_dup`** same env as above; **`f_msl_blocks_pid_controller`** / **`f_msl_blocks_filter`**: **`timeout_ms`** **1200000** and **`extra_rust_args`** **`--t-end=5.0`**, **`--dt=0.02`** (shorter / coarser than harness defaults) so these cases are less likely to hit the default **600s** wall on first heavy MSL compile+sim; CLI parses args in order, so per-case t_end/dt override the defaults.
- **`jit-compiler/Cargo.toml`**: set **`default-run = "rustmodlica"`** so `cargo run` from `jit-compiler/` (MOS / script children) no longer errors when both **`rustmodlica`** and **`msl_pack_bake`** exist.
- **`regress_full.json`**: removed **17** `f_mtest_*` cases that depend on optional paths (`RedeclareSmoke`, `Tables/*`, `Test*`) in a slim `ModelicaTest` checkout; they live in **`regress_modelicatest_mirror.json`**, run with **`--tier mtest_mirror`**. New **`scope modelicatest_mirror`** points at that file.

### Bug fixes: Clocked / Discrete / Noise model runtime crashes

- **JIT `when_count`/`crossings_count` buffer truncation** (`compile.rs`): moved counter snapshot after all `alg_equations`/`diff_equations` compilation so the returned counts reflect the full codegen. Previously, `when`/`elsewhen` and zero-crossing slots generated inside equation-to-algorithm conversion were not counted, leading to undersized `when_states`/`crossings` buffers and `ACCESS_VIOLATION` (exit `-1073741819`) during simulation for `TriggeredSampler`, many `Modelica.Clocked.Examples.*`, and `CascadeControlledDrive.*` models.
- **Variable classification now scans `algorithms` and `initial_equations`** (`classify_body.rs`): `extract_unknowns` previously only scanned `flat_model.equations`, missing variables referenced exclusively inside `when`/`sample` algorithm sections (e.g. input `u` in `ZeroOrderHold`, `Sampler`, `FirstOrderHold`). Added `collect_vars_alg` for `AlgorithmStatement` trees and extended the referenced-variable collection to include `flat_model.algorithms`, `initial_equations`, and `initial_algorithms`.
- **Model inheritance now merges `algorithms`, `initial_equations`, and `initial_algorithms`** (`flatten/utils.rs`): `merge_models` previously only merged `equations` and `declarations` from base classes, silently dropping algorithm sections. This caused Noise blocks (which inherit `when sample(...)` algorithms from `PartialNoise`) to produce empty flattened models with 0 equations and missing internal variables like `r_raw`.
- **JIT `distribution()` handler** (`call.rs`): added explicit passthrough handler for MSL `distribution(r_raw)` calls instead of falling through to the generic `unknown-import` path, preventing chain failures in all `Modelica.Blocks.Noise.*` models.
- **`constrainedby` BFS uses unflattened AST** (`query_db/constrainedby.rs`): the query-DB path previously used `inheritance_flattened` models (which have `extends` merged and emptied) for the extends-chain BFS, making it impossible to verify that a redeclare type extends the constraint base. Now uses `model_ast` to preserve the `extends` chain, fixing `[FLATTEN_CONSTRAINEDBY]` false failures like `MathBoolean.And` vs `PartialBooleanMISO`.
- **Debug assertions** (`simulation/events.rs`): added `debug_assert!` checks for `when_states` and `pre_discrete` buffer sizes before `calc_derivs` invocation.

### Refined directory regression (MSL+Modelica)

- **`run_modelica_dir_regression.ps1`**: default **`-PerModelTimeoutSec`** raised from **300** to **720** so cold **Full** flatten + JIT on large MSL examples (e.g. **SpeedControlledDCPM**, ~9–10min wall) stays under the watchdog; override lower for fast subsets.
- **`run_regression.ps1`**: **`DirPerModelTimeoutSec`** default **720** (was **300**) to match DIR watchdog alignment above.
- **`jit-compiler/scripts/compare_salsa_flatten_speed_dcpm.ps1`**: cold **`flat_full`** ( **`RUSTMODLICA_CACHE_SQLITE=0`** + **`RUSTMODLICA_CACHE_SHM=0`** ) compares **`RUSTMODLICA_SALSA=1`** vs **`0`** on **`--validate-tier=flatten`** for **SpeedControlledDCPM**; writes **`build_modelica_dir_regress/salsa_flatten_compare/summary.json`**. Sample run (same machine): **`flatten_inline_ms`** ~**607k** (SALSA=1, query `decl_expand`/`eq_expand` visible in **`compile_perf`**) vs ~**858k** (SALSA=0, legacy; query counters often zero—use **`flatten_inline_ms`**).
- **`run_modelica_dir_regression.ps1`**: when **not** using **`-UsePrivateCache`** and **`RUSTMODLICA_FLATTEN_CACHE_DIR` is unset**, default it to **`<OutDir>/shared_flatten_cache`** so **repeat runs of the same model** (and any other compile sharing the same **flat_full** key) can **`FLAT_HIT`** after the first cold write; different qualified names still use **separate** keys (e.g. **CurrentControlledDCPM** does not warm **SpeedControlledDCPM**).
- **`run_modelica_dir_regression.ps1`**: when **`RUSTMODLICA_SALSA` is unset** on the process and the run is **not** **`AnalyzeOnly`**, default **`RUSTMODLICA_SALSA=1`** for simulation children so **query-based flatten** matches `--validate` defaults and reduces cold **Full** flatten wall time on large MSL examples (e.g. **ControlledDCDrives**). Set **`RUSTMODLICA_SALSA=0`** before invoking the script to force **legacy** flatten for every case.
- **`run_modelica_dir_regression.ps1`**: optional **N-shard parallel analyze gate** via **`AnalyzeParallelWorkers`** and **`AnalyzeShardNoProgressTimeoutSec`**, reusing the shard watchdog. **`Invoke-RustmodlicaWithTimeout`** assigns child **`rustmodlica.exe`** to a **Win32 Job Object** when possible (**`PerProcessMemoryLimitMb`**, kill-on-close, optional OOM from RSS vs limit). Local **`dir_quarantine.json`** (default under **`build_modelica_dir_regress/local/`**, gitignored) skips repeat-offender models unless **`-RetryQuarantined`**. **Analyze** checkpoint: **`ResumeAnalyzeCheckpoint`**, per-run **`models_hash`**, and **`AnalyzeCheckpointEvery`**; **`dir_metrics.json`**, enhanced **`summary.txt`**, and **ETA / Top-N** slow-model heartbeats. TwoStage analyze gate: **`AnalyzeValidationMode`** (default **`quick`**, maps to **`--validation-mode=quick`**) to align with **`--validate-tier=analyze`**; override with **`full`** for stricter flatten cost. Default **`AnalyzeFirstTimeoutSec`** is **180** for heavy MSL batches.
- **`run_regression.ps1`**: **preflight** extends smoke (Filter analyze tier, subprocess **`--version`**, free-RAM check vs workers x memory cap); forwards new DIR parameters (**`DirAnalyzeValidationMode`**, **`DirAnalyzeFirstTimeoutSec`** default **180s** for heavy TwoStage analyze); **`-RecordBaseline`** **regression_summary.json** merges **phase wall seconds** and **dir** metrics when **`dir_metrics.json`** is present. After a pull, re-run a **release** build and, when using T3, **`run_regression.ps1 -RecordBaseline baseline/full_regression_v1`** to refresh the rollup.

### Tooling

- **`scripts/refresh_code_review_graph.ps1`**: wrapper for **`python -m code_review_graph`** `update` (default) or `build` against the repo root, optional **`--skip-flows`**, **`--skip-postprocess`**, then prints **`status`**. The MCP config **`.cursor/mcp.json`** still points **`code_review_graph serve`** at this repo; refresh the on-disk graph under **`.code-review-graph/`** (gitignored) before heavy code review so MCP tools see current symbols and edges.

### MSL versioned pre-baked cache (IDE + `rustmodlica`)

- **`MslPack` session + hydrate**: `jit-compiler/src/cache/msl_pack/` bakes parse/model-ast + flatten disk rows under a stable **MSL tree digest** + version label; `ModelLoader::add_path` hydrates matching packs from **`RUSTMODLICA_MSL_PACK_DIRS`**; **`msl_pack_bake`** binary and **`scripts/build_msl_cache.ps1`** produce a pack directory for **`modai-ide/src-tauri/resources/msl-cache/`** (ignored except **`README.txt`**).
- **Hotness**: optional **`RUSTMODLICA_MSL_HOTNESS_JSON`** + **`on_flatten_success`** after flatten records **`Modelica.*`** names with LRU cap (**`RUSTMODLICA_MSL_HOTNESS_CAP`**, default 200); modai-ide bootstrap sets the path under app-data **`msl-cache/msl-hotness.json`**.
- **modai-ide**: startup copies bundled packs into app-data; Tauri commands **`msl_cache_*`** (status, clear, check/download update from **`extensions.msl_pack_manifest_url`**, local rebuild); Settings UI section for MSL pack.

### Regression baselines and harness

- **`regress-harness`**: `jit record-correctness` / `jit compare-correctness` write/read **`baseline/correctness_testlib_v1/`**-style artifacts; **`jit validate-perf`** gains **`--default-correctness-models`** and **`--include-jitstress-probe`**; **`jit compare-baseline` / `jit update-baseline`** accept **`--preset large-scale-v2`** (default file **`baseline/large_scale_jit_validate_perf_v2/jit_perf_baseline.json`**, thresholds from `baseline::thresholds_large_scale_v2`).
- **`baseline/large_scale_jit_validate_perf_v2/`**: large-matrix perf baseline (benchmark rows aligned with **`testlib_validate_perf_v1`**, relaxed thresholds for analyze-tier work).
- **`baseline/full_regression_v1/`**: template for **`run_regression.ps1 -RecordBaseline`** NDJSON rollup (`regression_summary.json`).
- **`run_regression.ps1`** / **`run_modelica_dir_regression.ps1`** / **`jit-compiler/scripts/dir_smoke_analyze.ps1`** (new): **environment preflight** runs `cargo build --target-dir <cargoTargetDirPrimary> -p rustmodlica --release` then a 3s `--validate --validate-tier=analyze Modelica.Blocks.Sources.Sine` smoke **before** any DIR / EVENT-SCAN work; on failure the regression aborts with **exit=2** and a clear `[PREFLIGHT]` diagnostic, so sundials-DLL / target-dir mismatches no longer cost ~4h of DIR wall-clock to surface as "0 model(s) passed analyze gate". Bypass with **`-SkipPreflight`** only when triaging the preflight itself. DIR analyze-gate Sundials DLL probe corrected: it now uses **`<target-dir>/<profile>/build/sundials-sys-*/out/lib`** (was incorrectly stripping the profile dir, hitting the non-existent `target_regression\build`); resolution order is `(exe.parent)/build` -> `target_regression/release/build` -> `target/release/build` -> `repoRoot/target/release/build`. `Invoke-RustmodlicaWithTimeout` + `ConvertTo-ProcessArgumentString` replace the prior `Start-Process -ArgumentList` path so analyze-gate child processes receive correct argv quoting and a hard timeout.
- **`run_event_scan_matrix.ps1`** / **`run_regression.ps1`**: EVENT-SCAN-MATRIX now passes **`--target-dir`** to `cargo run` via **`-CargoTargetDir`** (defaults from regression: **`jit-compiler/<cargoTargetDirPrimary>`**, same as **`target_regression`**), so `rustmodlica` matches the binary used for TestLib/DIR instead of a separate default **`jit-compiler/target/release`** build (fixes **`ModelicaTest.JitStress.SyncOmCompare`** matrix cells reporting **`process_failed`** when only **`target_regression`** was populated).
- **`run_regression.ps1`**: optional **`-RecordBaseline`**, **`-CompareBaseline`**, **`-CompareBaselineCurrent`**; **`[bool]$CleanCaches=$true`** now purges the **full** tier set (AOT archives; `jit-codegen`; flatten SQLite under `jit-compiler/build/cache/{project,std,user}`; `path_hash_index.sqlite{,-shm,-wal}`; `stage_epochs.txt` / `ir_schema_epoch.txt`; `cache_miss_agg_v1.json` / `model_hotness_v1.json`; warmup locks; both `%LOCALAPPDATA%\rustmodlica\{jit-codegen,std-cache}` and `%APPDATA%\rustmodlica\user-cache`) unless **`-CleanCaches:$false`** or **`-KeepCaches`** (switch, preferred for **`Start-Process -ArgumentList`** / subprocess argv to avoid **`[bool]`** misparsing **`CleanCaches:False`** as a string). The earlier purge list missed the three flatten SQLite scopes plus the helper indices, so "warm-cache reuse" surfaced stale rows as `-1073741819` access violations during simulation even when the current JIT was a fresh compile. DIR stage defaults **`DirUsePrivateCache=$true`**, sets tier cache roots, optional **`DirTwoStage`**, forwards timeouts to **`run_modelica_dir_regression.ps1`**, uses **heartbeat** while waiting on the DIR child process; after green **`JIT_VALIDATE_PERF`**, runs informational **`jit compare-baseline --preset large-scale-v2`** when that baseline file exists.
- **`run_regression.ps1` self-heal on AV**: `Invoke-RustmodlicaCargoRun` now detects Windows `exit=-1073741819` (**`STATUS_ACCESS_VIOLATION` / `0xC0000005`**) and retries once with **`RUSTMODLICA_JIT_CODEGEN_CACHE=0`**, **`RUSTMODLICA_AOT_NATIVE_LOAD=0`**, **`RUSTMODLICA_FLATTEN_FULL_CACHE=0`**, **`RUSTMODLICA_CACHE_SQLITE=0`** so the retry is fully disk-cache-free. Used primarily to keep **`-KeepCaches`** / warm-cache runs green even when a stale cache row slips through. The case detail trail records **`self_healed=true`** in such cases; pairs with the purge path above.
- **PowerShell script hygiene (PSScriptAnalyzer)**: **`Escape-Csv`** renamed **`ConvertTo-CsvField`** in **`run_regression.ps1`**, **`run_modelica_dir_regression.ps1`**, **`run_modelica_dir_validate.ps1`** (CSV log quoting only; not meant for external dot-source). **`run_modelica_dir_regression.ps1`**: **`Apply-DirPrivateCacheEnv`** -> **`Set-DirPrivateCacheEnv`** (doc **`docs/regression/DIR_Private_Incremental_Cache_Design.md`** updated); **`Normalize-ModelName`** -> **`ConvertTo-NormalizedModelName`**. **`run_modelica_dir_validate.ps1`**: **`Collect-MoFiles`** -> **`Get-MoFileList`**; parallel/serial validate paths use **`$cmdArgs`** instead of overwriting automatic **`$args`**; **`$null`** comparisons follow **`$null -ne $x`** style.
- **`run_modelica_dir_regression.ps1`**: **`-TwoStage`** (analyze gate before sim), **`-PerModelTimeoutSec`**, **`-AnalyzeFirstTimeoutSec`**, parallel shard wait **heartbeat**, **`|stage=...`** in private-cache policy string. Parallel **shard watchdog** (`-ShardNoProgressTimeoutSec`) now follows **`taskkill /T`** on the shard `powershell` PID after `Stop-Process` so `rustmodlica.exe` and other descendants cannot outlive a killed shard (avoids stuck **`shard_wait`** + stray simulator processes on Windows). Fixed console **`Summary: ... failed`** under **`-TwoStage`**: the script now re-counts **after** prepending `analyze_gate_failed` lines, matching **`summary.txt`**.

### JIT perf baseline

- **`regress-harness jit validate-perf`**: repeatable **`--set-env KEY=VAL`** for child `rustmodlica` processes; manifest records `child_env`. Optional **`--std-cache-root`** / **`--user-cache-root`** set tier roots for every child.
- **Default `jit compare-baseline` baseline path** (when **`--baseline`** is omitted): **`baseline/20260418_three_tier_devloop/jit_perf_baseline.json`** (six models, **`stdlib_bake`** + **`devloop_multi_model`** speedup pair, same numeric `duration_ms_p50` seeds as the prior baseline). Optional threshold fields (`std_hit_rate_min_pct`, `user_hit_rate_min_pct`, `aot_hit_rate_min_pct`, `project_rebuild_regression_pct`) are reserved for extended compares. Legacy baseline files live under **`baseline/_legacy/20260417_jit_cranelift_none/`**.
- **`.gitignore`**: under **`baseline/**`**, ignore JIT validate-perf bulk (`cache_*`, **`*.sqlite*`**, perf/log/stats/manifest artifacts, **`aot_archive*.bin`**); version control **`report.json`** + **`jit_perf_baseline.json`** + optional **`bake_manifest-*.json`** for the default JIT perf baseline folder.
- **Scripts**: **`scripts/jit_opt_matrix_regress.ps1`** runs **`jit validate-perf`** under several **`--set-env`** profiles, **`jit compare-baseline`** each **`report.json`** against the default baseline, and writes **`build/jit_opt_matrix/matrix_summary.json`** plus per-profile **`compare_baseline.json`**.

### JIT cache / warmup (Leyden-style)

- **Native tier auto-invalidation via `binary_build_id`**: `codegen_host_tag()` (`jit/codegen_cache/cache_key.rs`) now appends `cache::build_id::binary_build_id()`, which flows into **`CodegenCacheKey::stable_hash`** and therefore the **`jit-codegen/<hash>.bin`** filename. Rebuilding `rustmodlica.exe` produces a new `binary_build_id`, so old machine-code blobs become unreachable by filename lookup (not served, not imported) without any manual `CODEGEN_CACHE_VERSION` / `CODEGEN_JIT_ABI_REVISION` bump. Dev builds that previously shared a `CARGO_PKG_VERSION` between runs now self-invalidate their native codegen tier. Orphaned old `.bin` files stay on disk until `enforce_global_budget` / `--cache-gc` reclaims them; they are invisible to the current binary in the meantime.
- **AOT archive compiler fingerprint**: `jit/aot_archive.rs` introduces **`aot_archive_compiler_fingerprint() = "<CARGO_PKG_VERSION>|<binary_build_id>"`** and stamps it into **`AotArchiveHeader::compiler_version`** on write. `AotArchive::from_bytes` rejects any archive whose header disagrees with the current fingerprint (`compiler version mismatch: archive=... host=... (regenerate aot_archive.bin)`), so **all three tier archives** (`aot_archive-std.bin`, `aot_archive-user.bin`, `aot_archive-project.bin`) and the legacy `aot_archive.bin` are invalidated as a whole the first time a new binary reads them — upstream of the per-TOC `codegen_key_hash` check, and without requiring a manual `AOT_ARCHIVE_VERSION` bump. A unit test (`archive_rejects_foreign_binary_fingerprint`) pins the invariant. Combined with the `codegen_host_tag` change above, both native-code tiers auto-invalidate across binary rebuilds, matching the SQLite IR-cache behavior.
- **Windows COFF loader: tracing + identical section filters + builtin merge for native reloc**: `RUSTMODLICA_COFF_RELOC_TRACE=1` enables stderr lines prefixed **`[coff-reloc]`** from `jit/codegen_cache/{reloc_trace,coff_reloc,exec_buffer}.rs` (anonymous `VirtualAlloc` / `VirtualProtect`, per-loader summary: section count, image length, relocation count, `import_slots` count, `calc_derivs` offset, first 16 `__imp_` slot addresses). **`=2`** adds per-section relocation counts. Trace context **`native-reloc`** is any **`load_aot_code_blob`** path (AOT archive **or** sim-bundle reuse), not only tiered AOT. **`load_coff_object_exec_windows`** and **`load_aot_blob_exec_windows`** share **`include_section_in_image`**: skip **`.debug*`**, **`.pdata*`**, and **`.xdata*`** so disk vs archive serves the same layout. **`load_aot_code_blob`** now merges **`builtin_jit_symbol_ptrs()`** into the caller map before relocation (caller entries override on key clash); sim-bundle / artifact fast paths used to pass **`all_symbols`** (externals-only), often **empty** for TestLib — traces showed **`runtime_syms=0`** while resolution fell through to **`GetProcAddress`**, which can diverge from JIT-registered addresses. Helper script: **`scripts/repro_coff_reloc_trace.ps1`**.
- **SQLite `parent_hash` chain (flatten / IR caches)**: `jit-compiler/src/flatten/cache_sqlite.rs` adds a `parent_hash TEXT NOT NULL DEFAULT ''` column to `cache_entries` (migrated in-place) and stamps every row with **`compute_parent_hash(key, kind) = xxh3_128(PARENT_HASH_CHAIN_VERSION || binary_build_id || kind || stage_epoch || key)`**. Reads fetch the stored row and compare `parent_hash` against the current `compute_parent_hash(...)`; mismatch is treated as miss (row returned `None`; `cache_kind_stats` bumps a synthetic `__chain_miss__` counter for observability). The chain root is **`cache::build_id::binary_build_id()`** (`cache/build_id.rs`): `xxh3_128` of the running **`rustmodlica.exe`** bytes, memoized via `OnceLock`; falls back to `v:<CARGO_PKG_VERSION>` when `current_exe()` or the prefix read is unavailable (first 8 MiB windowed for very large debug builds). Consequence: rebuilding `rustmodlica` automatically invalidates **every** pre-existing SQLite row even when nobody bumped `stage_epochs.txt` / `CODEGEN_CACHE_VERSION` / `AOT_ARCHIVE_VERSION`, because the new binary's `parent_hash` no longer matches the stored one. Same-binary warm runs still hit every row (same `binary_build_id`), so the cache keeps its acceleration value; only across-binary reuse silently fails closed. `xxhash-rust` now carries the **`xxh3`** feature in addition to **`xxh64`**.
- **Codegen cache key**: `Jit::compile` (`jit/compile.rs`) and `calc_derivs_codegen_cache_key` (`jit/config.rs`) now fold the **real model identity** into `flat_model_hash` / `CodegenCacheKey` via new `Jit::set_active_model_name`. Previously all three call sites hard-coded `"calc_derivs"`, so two TestLib models with identical `state_vars` / `discrete_vars` / `param_vars` / `output_vars` / `array_info` shapes produced the **same** `stable_hash`, shared one `.bin` under `jit-codegen/`, and loaded the wrong machine code on the second run — consistently crashing Windows regression with `exit -1073741819` (access violation) on models like `TestLib/NoEventInAlg`, `TestLib/FuncInline`, `TestLib/AlgTest`, `TestLib/WhenTest`, `TestLib/JacobianTest`, etc. Callers in `compiler/compile_model/compile/entry.rs` now call `jit.set_active_model_name(model_name)` before `jit.compile(...)`; `calc_derivs_codegen_cache_key` already receives the real name and forwards it.
- **`AOT_ARCHIVE_VERSION`**: bumped to **3** in sync with `CODEGEN_CACHE_VERSION=5` / tiered lookup fix. Existing `aot_archive-*.bin` produced by older binaries are rejected with `version mismatch`, forcing a clean rebuild on first run so the fixed model-name hash repopulates the archive.
- **`run_regression.ps1`**: `-CleanCaches` behavior and **`-CleanCaches:$false`** escape hatch are documented in **`baseline/README.md`** and the script header (same paths as the **Regression baselines** bullet above).
- **`CODEGEN_CACHE_VERSION`**: bumped to **5** so existing `jit-codegen` disk entries (JSON + `.bin`) are rejected on load (`version mismatch` path), avoiding stale native objects after recent AOT / tier read fixes and reducing Windows access violations under full optimization.

- **Three-tier roots**: `RUSTMODLICA_STD_CACHE_ROOT`, `RUSTMODLICA_USER_CACHE_ROOT` (defaults under `%LOCALAPPDATA%` / `%APPDATA%`), project tier remains `RUSTMODLICA_FLATTEN_CACHE_DIR`. SQLite reads use a tier fallback chain; writes target the model scope root. JIT codegen objects live under `<tier-root>/jit-codegen` with the same read fallback order (unless `RUSTMODLICA_JIT_CODEGEN_CACHE_DIR` overrides with a single global dir).
- **AOT**: tier archives `aot_archive-std.bin` / `aot_archive-user.bin` / `aot_archive-project.bin` plus legacy `aot_archive.bin` fallback; `AotArchiveSet::load_tiered` merges lookups. Merge-on-compile uses `write_merged_for_scope` from the model codegen tier. **Native load**: `AotArchiveSet::lookup_entry_with_blob` reads the code slice from the same tier file as the matched TOC entry (avoiding a wrong object when the same stable hash exists under another tier root); artifact and sim-bundle paths reject a TOC hit when `when_count` / `crossings_count` disagree with the loaded bundle, and full compile rejects when TOC `when_count` disagrees with the flattened when-equation count.
- **CLI**: `rustmodlica cache warmup|bake --scope std|user|project --input <models.json> [--root <dir>] [--lib-path ...]`, `cache import|export` with `--from` / `--to`, writes `manifest.json` on bake/export.
- **Warmup**: cross-process **`.warmup-<tier>.lock`** under the locked cache root, `RUSTMODLICA_WARMUP_TIER=auto`, optional `RUSTMODLICA_WARMUP_DEP_GRAPH`, shallow scans of `RUSTMODLICA_STDLIB_ROOTS` / `RUSTMODLICA_USERLIB_ROOTS`, ranked candidates (`model_hotness_v1.json` + `.mo` size), compile-epoch cancel for foreground compiles (`CompilerOptions.warm_background`), periodic global budget checks during warmup loops.
- **Precompile**: `precompile_models_parallel` uses **rayon** when more than one model.
- **Codegen disk**: `codegen_path_index.sqlite` under JIT cache root for faster `global_budget` scans; updated on cache write.
- **Path hash index**: `path_hash_index.sqlite` under flatten cache root; `closure_hash::unified_file_hash` consults it after the in-process LRU.
- **CLI**: `--cache-stats` is read-only; **`--cache-gc`** runs `enforce_global_budget`; **`--cache-invalidate`** (`soft|hard|model`); **`--cache-stats --miss-breakdown`** prints aggregated `cache_miss_agg_v1.json`.
- **Perf / observability**: `CompilePerfReport` adds `warmup_populated_count`, `warmup_attributable_hits`, `warmup_time_ms`; end-of-compile hook persists `cache_miss_agg` + `model_hotness_record`.
- **`--perf-json` / `CompilePerfReport` query counters**: capture `query_db::perf_snapshot()` immediately after `flatten_and_inline` so nested `FlattenCondenser` compiles (which call `perf_reset()` at compile entry) no longer wipe `flatten_wall_us`, `parse_us`, `cache_scope_stage_*`, etc. `query_db::perf::snapshot` now clones the map on poisoned `RwLock` reads instead of returning an empty map.
- **`regress-harness jit validate-perf`**: default child `--lib-path` list now mirrors `run_regression.ps1` (`jit-compiler` when `Modelica/package.mo` exists, plus `jit-compiler/TestLib`, plus `StandardLib` when present) so full TestLib matrices go green; validate success prefers parsing stdout JSON `success` rather than loose substring matching.
- **TestLib JIT perf baseline**: `baseline/testlib_validate_perf_v1/jit_perf_baseline.json` (860 `scenario/model` rows) for `jit compare-baseline` after optimizations; thresholds omit tier floors and set `speedup_min_ratio` to 0 so analyze-tier matrices compare cleanly on duration/codegen caps (`baseline::TESTLIB_VALIDATE_PERF_V1_BASELINE_REL`).
- **Invalidation**: `CompileFlagsChanged` maps to **soft** invalidation; soft deletes stage-keyed SQLite rows (flat_full + array_sizes) without full project DB wipe.
- **Artifact cache**: `RUSTMODLICA_ARTIFACT_PRUNE_INTERVAL_MS` (default 60s) throttles SQLite prune on get/put.
- **AOT**: `try_load_default_archive` logs skip reason on load failure.

### Adaptive CONST_FOLD / EQ_DCE and compile perf extensions

- Optional **`RUSTMODLICA_ADAPTIVE_FOLD_POLICY=1`**: SQLite-backed per-flatten-hash record (`fold_benefit_record`) skips const-fold/DCE when the last run had zero benefit, and applies a short cooldown when external-resolve time regresses; **`persist_and_tierup_flags`** also arms tier-up recompiles via `tierup_skip_const_fold` (see `simulation.rs`).
- **`--perf-json`**: new fields include `external_resolve_*_us` sub-spans, `const_fold_skipped_by_policy`, `const_fold_cooldown_active`, `jit_bypassed_tier0`, `warmup_auto_enqueued`, and flatten inline sub-phase proxies (`salsa_flat_full_get_us`, etc.).
- **External resolve cache key** now uses **pre-fold** call-site lists so CONST_FOLD on/off shares the same SQLite key where sites are unchanged.
- Scripts: **`scripts/phase_split_negative_three.ps1`** (10-round medians + `summary.json`), **`scripts/msl_complex_opt_compare.ps1`** (`-AdaptiveFoldPolicyOnOpt`), **`scripts/cache_warm_kpi.ps1`** (cold vs hot KPI sample).

### Leyden-style AOT archive (v3) and JIT codegen disk cache

- **AOT archive format** is at **version 3** (`jit-compiler/src/jit/aot_archive.rs`, magic `RMJITAOT`). v3 bumps in sync with `CODEGEN_CACHE_VERSION=5` / tiered lookup fix. Table-of-contents entries store `when_count` and `crossings_count` in addition to `codegen_key_hash` and optional `import_symbols`, so the **AOT native fast path** can skip in-process `jit.compile` while still wiring the simulation loop correctly.
- **Version 1** `aot_archive.bin` files are rejected with a version mismatch error; delete the file or re-run a full compile to regenerate the archive.
- **Non-empty code blobs** in the archive are populated from the **JIT codegen disk cache** object file (`.bin` next to `.json` under the codegen cache root, keyed by `CodegenCacheKey::stable_hash()`). For this to work in practice:
  - Keep **`RUSTMODLICA_JIT_CODEGEN_CACHE` enabled** (default when unset: on; set to `0` / `false` / `no` to disable). Disabling the cache prevents writing/reading the relocatable object bytes used when merging into `aot_archive.bin`.
  - After at least one successful compile that **writes** a relocatable object for the model, the AOT merge step can embed those bytes; otherwise the TOC entry may carry an **empty** blob and the AOT native loader will fall back to normal JIT.
- **Optional cache directory**: `RUSTMODLICA_JIT_CODEGEN_CACHE_DIR`; default follows `dirs::cache_dir()` / `rustmodlica/jit-codegen` (see `codegen_cache_key.rs`).
- **Relocation loaders**:
  - **Windows**: COFF relocation in `jit-compiler/src/jit/codegen_cache/coff_reloc.rs`.
  - **Linux**: ELF64 relocation in `jit-compiler/src/jit/codegen_cache/elf_reloc.rs` (undefined symbols resolved via `dlsym`; the JIT crate links `libdl` on Linux).
  - **Other Unix** (e.g. macOS): AOT native load remains **unsupported** in this release (object artifacts are ELF on Linux; macOS typically uses Mach-O from the toolchain).

### `compile_perf` / `--perf-json` (dual compile + AOT native)

Structured compile performance JSON (via `--perf-json=...`) includes Leyden-related fields such as `dual_compile_requested`, `dual_compile_ok`, `dual_compile_speculation_count`, `dual_compile_status`, and `aot_native_load_status` for regression tracking.
