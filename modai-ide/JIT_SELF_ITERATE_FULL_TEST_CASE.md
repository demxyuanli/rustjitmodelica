# JIT Self-Iterate Full Test Case (Compiler Feature + Regression Case)

This test case is used **inside the IDE** to complete one full JIT self-iterate cycle: implement a small compiler extension and its corresponding mo regression case, and thereby validate the **complete JIT self-iterate functionality** (sandbox, build, test, mo cases). Use it after the [minimal test case](JIT_SELF_ITERATE_TEST_CASE.md) (comment-only change).

## Purpose

- **In-IDE JIT self-iterate test**: Run the full self-iterate pipeline (optionally with a diff that adds the feature and case) and assert that the pipeline succeeds and the new regression case passes.
- **Add compiler functionality**: Implement a new built-in `smooth(Real) -> Real` (identity) in the JIT.
- **Add regression case**: New model `TestLib/SmoothTest.mo` and register it in `jit_traceability.json`.

The sandbox runs in the repo root (excluding `target`, `.git`, `modai-ide`), so the diff only touches `src/`, `TestLib/`, `jit_traceability.json`, and `REGRESSION_CASES.txt`.

## Verification (in-IDE)

- **Automated**: In `modai-ide/src-tauri` run `cargo test self_iterate_full_pipeline`. This runs the same `self_iterate_impl` pipeline used by the IDE (sandbox copy, release build, test, mo cases) and asserts success and that **TestLib/SmoothTest** is in the mo results with **pass**. No diff is applied; the test confirms that the current codebase (with smooth + SmoothTest already present) passes the full pipeline.
- **Manual**: In the IDE, open Self-Iterate → Manual diff → paste the full test diff below → **Run in sandbox** → **Run full build**. Confirm that the result shows **TestLib/SmoothTest** with **pass**.

## Prerequisites

Same as the minimal case: Rust toolchain, IDE with JIT Iterate workspace. No system `patch` required (IDE uses built-in apply).

## Detailed UI workflow (界面操作详细流程)

The Self-Iterate panel has four steps. Follow in order.

### Step 1: Select context (选择上下文)

- In the right-hand panel, ensure the **Iterate** tab is open (Self-Iterate / 自迭代).
- **Target** (目标): Optionally enter a short description of the change (e.g. "Add smooth() built-in and SmoothTest"). For the full test case you can leave it empty or fill for history.
- **Context files** (可选): You can skip adding context files when using Manual diff; they are used when generating a patch with AI.
- No button to click here; go to Step 2 when ready.

### Step 2: Generate / Edit (生成 / 编辑 Diff)

- In the card titled **2. Diff 与沙箱** (or "Generate / Edit"), choose one of:
  - **Generate patch** (生成补丁): Uses AI to produce a diff from the target; requires API key.
  - **Manual diff** (手动输入 diff): Paste a pre-made unified diff. Use this for the full test case.
- Click **Manual diff** (手动输入 diff). A diff editor appears.
- Paste the **full test diff** from the "Full test diff" section below into the editor (replace any existing content). Ensure the whole diff is one block (from `--- a/...` to the last line of the last hunk).
- Do not click anything else in this step; the diff is already "entered". Proceed to Step 3.

### Step 3: Test & Validate (测试与验证)

- In the card **3. Mo 结果** (Test & Validate), you will see the **Run in sandbox** button (沙箱运行).
- Click **Run in sandbox** (沙箱运行). The IDE runs a quick check (`cargo check`) in a sandbox with your diff applied. Wait until the button is enabled again.
- Check the result message:
  - If it says **"Check OK. (Xms) Run full build to..."** and is green, the quick check passed. You will see a new button **Run full build** (完整构建).
  - If it is red, fix the diff or environment and run again.
- Click **Run full build** (完整构建). The IDE runs full `cargo build --release`, `cargo test --release`, and all mo cases in the sandbox. This may take several minutes.
- When it finishes, check:
  - The result box should show **"Build and test OK; mo cases: N passed. (Xms)"** (or similar) in green.
  - Below that, the **mo cases table** should list **TestLib/SmoothTest** with status **OK** (pass). If you see it and it passed, the full JIT self-iterate test has succeeded.
- Optional: Click **Save to history** (保存到历史) to record this run in the iteration history.

### Step 4: Adopt / Commit (采纳 / 提交)

- In the card **4. 采纳/提交** (Adopt / Commit), after a successful run you can:
  - **Adopt to workspace** (采纳到工作区): Applies the diff from the sandbox to your real workspace (no commit). Click it once; the diff editor may clear and the banner shows "Patch adopted to workspace."
  - **Commit patch** (提交补丁): Only appears after adopt (when there is no diff left to adopt). Enter a commit message in the input, then click **Commit patch** to run `git add -A` and `git commit` in the repo root.

End of workflow: you have either only validated in the sandbox (steps 1–3) or also adopted and committed (step 4).

## Steps (summary)

1. Switch to **JIT Iterate** workspace and open the **Iterate** tab (Self-Iterate).
2. In step 2 ("Generate / Edit"), click **Manual diff**.
3. Paste the **full test diff** below into the editor (see "Full test diff" section). If the diff is large, you can apply in two steps: first the compiler + TestLib + REGRESSION_CASES diff, then the jit_traceability diff (or add the JSON edits manually).
4. Click **Run in sandbox** (quick: `cargo check` only).
5. After "Check OK", click **Run full build** to run `cargo build --release`, `cargo test --release`, and all mo cases.
6. Expected: **Build and test OK**; mo cases include **TestLib/SmoothTest** with **pass**.
7. (Optional) **Adopt to workspace** and **Commit patch**.

## Full test diff

Apply the following unified diff. It edits `src/jit/native.rs`, `src/compiler/inline.rs`, adds `TestLib/SmoothTest.mo`, updates `REGRESSION_CASES.txt`, and updates `jit_traceability.json` (add feature T1-5, case TestLib/SmoothTest, and mappings). If `jit_traceability.json` is missing or has different structure, apply the other hunks and add the case/feature manually (see "Manual jit_traceability edits" below).

```
--- a/src/jit/native.rs
+++ b/src/jit/native.rs
@@ -48,6 +48,11 @@ extern "C" fn modelica_integer(x: f64) -> f64 {
     x.trunc()
 }
 
+/// smooth(Real) -> Real: identity for testing; Modelica uses for continuity hint.
+extern "C" fn modelica_smooth(x: f64) -> f64 {
+    x
+}
+
 extern "C" fn modelica_boolean(x: f64) -> f64 {
     if x != 0.0 { 1.0 } else { 0.0 }
 }
@@ -163,6 +168,7 @@ pub fn register_symbols(builder: &mut JITBuilder) {
     builder.symbol("max", modelica_max as *const u8);
     builder.symbol("div", modelica_div as *const u8);
     builder.symbol("integer", modelica_integer as *const u8);
+    builder.symbol("smooth", modelica_smooth as *const u8);
 
     // Modelica.Math Aliases
     builder.symbol("Modelica.Math.sin", f64::sin as *const u8);
@@ -207,7 +213,7 @@ pub fn builtin_jit_symbol_names() -> std::collections::HashSet<&'static str> {
     set.insert("sqrt"); set.insert("exp"); set.insert("log"); set.insert("log10");
     set.insert("abs"); set.insert("ceil"); set.insert("floor");
     set.insert("mod"); set.insert("rem"); set.insert("sign"); set.insert("min"); set.insert("max");
-    set.insert("div"); set.insert("integer");
+    set.insert("div"); set.insert("integer"); set.insert("smooth");
     set.insert("Modelica.Math.sin"); set.insert("Modelica.Math.cos"); set.insert("Modelica.Math.tan");
--- a/src/compiler/inline.rs
+++ b/src/compiler/inline.rs
@@ -137,7 +137,7 @@ pub(crate) fn get_function_body(model: &Model) -> Option<(Vec<String>, Vec<(Stri
 /// FUNC-2: Exposed so compiler can detect remaining user calls that were not inlined.
 pub(crate) fn is_builtin_function(name: &str) -> bool {
     matches!(name,
-        "abs" | "sign" | "sqrt" | "min" | "max" | "mod" | "rem" | "div" | "integer"
+        "abs" | "sign" | "sqrt" | "min" | "max" | "mod" | "rem" | "div" | "integer" | "smooth"
         | "ceil" | "floor" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2"
         | "sinh" | "cosh" | "tanh" | "exp" | "log" | "log10"
         | "pre" | "edge" | "change" | "noEvent" | "initial" | "terminal"
--- /dev/null
+++ b/TestLib/SmoothTest.mo
@@ -0,0 +1,5 @@
+model SmoothTest
+  Real x(start = 1.0);
+equation
+  der(x) = -smooth(x);
+end SmoothTest;
--- a/REGRESSION_CASES.txt
+++ b/REGRESSION_CASES.txt
@@ -54,6 +54,8 @@ TestLib/ConstraintEq      pass
 
 # --- Built-in math (abs, sign, sqrt, min, max, mod, rem, div, ceil, floor, integer) ---
 TestLib/MathBuiltins       pass
+# --- T1-5: smooth() built-in (identity for testing) ---
+TestLib/SmoothTest         pass
 
 # --- F2-1: Nested der() in expression ---
 TestLib/NestedDerTest      pass    (y = der(x) + 2; der(x) in RHS)
```

**jit_traceability.json**: If your repo already has `jit_traceability.json` with a `features` array and a `cases` array, add the following. Otherwise create the file or skip; the sandbox falls back to a hardcoded smoke list if the file is missing.

- In **features**, after the object with `"id": "T1-4"`, add:
  `{ "id": "T1-5", "name": "smooth() built-in", "category": "Language", "description": "smooth(expr) as identity for testing", "status": "covered" }`
- In **cases**, add:
  `{ "name": "TestLib/SmoothTest", "expected": "pass", "notes": "smooth() built-in" }`
- In **featureToCases**, add:
  `"T1-5": ["TestLib/SmoothTest"]`
- In **caseToSourceFiles**, add:
  `"TestLib/SmoothTest": ["src/jit/native.rs", "src/compiler/inline.rs"]`

(If you prefer a single patch for JSON, generate it from your tree with the above edits and paste into the same Manual diff.)

## Manual jit_traceability edits

If the combined diff fails on `jit_traceability.json` (e.g. different line numbers or missing file):

1. Apply only the Rust and TestLib and REGRESSION_CASES hunks from the diff above.
2. Open `jit_traceability.json` in the repo root and add the feature, case, and two mapping entries as in the previous section.
3. Run in sandbox again (quick then full build).

## Expected outcome

- **Quick run**: "Check OK. (Xms) Run full build to compile, test and run mo cases."
- **Full build**: "Build and test OK; mo cases: N passed. (Xms)" with **TestLib/SmoothTest** listed and **pass**.
- **Adopt**: "Patch adopted to workspace."
- **Commit**: Changes committed.

## Difference from minimal test case

| Minimal case | Full case |
|--------------|-----------|
| One comment in `src/lib.rs` | New built-in `smooth()` + new TestLib model + jit_traceability |
| No new mo case | New regression case TestLib/SmoothTest |
| Validates patch apply + build | Validates compiler change + regression case registration |
