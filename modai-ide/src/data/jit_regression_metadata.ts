/**
 * JIT capability catalog and feature-to-case mapping for the self-iteration tool.
 * Aligned with REGRESSION_CASES.txt and OPENMODELICA_ALIGNMENT_COVERAGE.md.
 */

export type FeatureStatus = "covered" | "partial";

export interface JitFeature {
  id: string;
  name: string;
  category: string;
  description: string;
  status: FeatureStatus;
}

export interface RegressionCase {
  name: string;
  expected: "pass" | "fail";
  notes?: string;
}

export const JIT_FEATURE_CATEGORIES = [
  "Language",
  "Flatten",
  "Algebraic",
  "Solver",
  "MSL",
  "IR",
  "Tooling",
] as const;

export const features: JitFeature[] = [
  { id: "T1-1", name: "noEvent in equation/algorithm/when", category: "Language", description: "noEvent(expr) compiles in equation, algorithm, and when", status: "covered" },
  { id: "T1-2", name: "initial() / terminal()", category: "Language", description: "terminal() = 1 near t_end, else 0", status: "covered" },
  { id: "T1-3", name: "function parse & AST", category: "Language", description: "Parse function; Model with is_function", status: "partial" },
  { id: "T1-4", name: "Function inlining", category: "Language", description: "User f(x) inlined at flatten", status: "covered" },
  { id: "F1-1", name: "record semantics", category: "Language", description: "Record flatten to scalar/array", status: "covered" },
  { id: "F1-2", name: "block semantics", category: "Language", description: "Block as top-level like model", status: "covered" },
  { id: "F2-1", name: "Nested der() in expression", category: "Language", description: "der(x) in RHS expressions", status: "covered" },
  { id: "F2-2", name: "pre/edge/change", category: "Language", description: "pre(), edge(), change() in JIT", status: "covered" },
  { id: "T2-1", name: "For-expansion", category: "Flatten", description: "For with small/large/bound=1", status: "covered" },
  { id: "T2-2", name: "connect type-check", category: "Flatten", description: "Incompatible connector error with location", status: "covered" },
  { id: "T2-3", name: "time_derivative (debug)", category: "Flatten", description: "time_derivative under debugPrint flag", status: "covered" },
  { id: "IR1", name: "DAE form & partitioning", category: "IR", description: "Explicit DAE, blocks (single/torn/mixed)", status: "covered" },
  { id: "IR2-3", name: "BLT & alias removal", category: "IR", description: "Matching, BLT, alias elimination", status: "covered" },
  { id: "IR3", name: "Initial eq & index", category: "IR", description: "Initial equation order; diff index; constraint", status: "covered" },
  { id: "T3-1", name: "SolvableBlock 1-32 residuals", category: "Algebraic", description: "Newton for 1..32 residuals; error text", status: "covered" },
  { id: "T3-2", name: "Newton failure diagnostics", category: "Algebraic", description: "Tearing var name, residual, value on status=2", status: "covered" },
  { id: "T3-3", name: "Symbolic vs numeric Jacobian", category: "Algebraic", description: "Jacobian consistency check in sim", status: "covered" },
  { id: "IR4-4", name: "Sparse Jacobian (API)", category: "Algebraic", description: "Sparse repr and solve API; JIT still dense", status: "covered" },
  { id: "T4-1", name: "Adaptive RK45", category: "Solver", description: "Dormand-Prince when no when/crossings", status: "covered" },
  { id: "RT1-1", name: "Events & reinit", category: "Solver", description: "when, zero-crossing, reinit", status: "covered" },
  { id: "F4-1", name: "connect() inside when", category: "Flatten", description: "Conditional connections -> When(cond, eqs, [])", status: "covered" },
  { id: "F4-3", name: "if-equation", category: "Language", description: "Equation::If in flatten/JIT", status: "covered" },
  { id: "F4-4", name: "assert/terminate", category: "Language", description: "assert and terminate in when", status: "covered" },
  { id: "F4-6", name: "Record equation flatten", category: "Flatten", description: "p2 = p1 -> component-wise equations", status: "covered" },
  { id: "F3-3", name: "Multi-output function", category: "Language", description: "(a,b)=f(x) expanded to outputs", status: "covered" },
  { id: "MSL-2", name: "Modelica.Blocks", category: "MSL", description: "Constant, Step, Sine, Integrator, TransferFunction", status: "covered" },
  { id: "MSL-3", name: "Modelica.Math built-ins", category: "MSL", description: "sin, cos, sqrt, min, max, mod, sign, etc.", status: "covered" },
  { id: "CG1-4", name: "Array preservation", category: "IR", description: "Array layout and loop fusion in C/JIT", status: "covered" },
  { id: "DBG-1", name: "backend-dae-info", category: "Tooling", description: "DAE stats, block counts, backend output", status: "covered" },
];

export const cases: RegressionCase[] = [
  { name: "TestLib/InitDummy", expected: "pass" },
  { name: "TestLib/InitWithParam", expected: "pass" },
  { name: "TestLib/InitTwoVars", expected: "pass", notes: "IR3 initial eq order" },
  { name: "TestLib/InitAlg", expected: "pass" },
  { name: "TestLib/InitWhen", expected: "pass" },
  { name: "TestLib/JacobianTest", expected: "pass" },
  { name: "TestLib/AlgebraicLoop2Eq", expected: "pass" },
  { name: "TestLib/SolvableBlock4Res", expected: "pass" },
  { name: "TestLib/AlgebraicLoopWarn", expected: "pass", notes: "SolvableBlock single residual" },
  { name: "TestLib/SolvableBlockMultiRes", expected: "pass", notes: "IR4-1 N=4..32" },
  { name: "TestLib/NoEventTest", expected: "pass" },
  { name: "TestLib/NoEventInWhen", expected: "pass" },
  { name: "TestLib/NoEventInAlg", expected: "pass" },
  { name: "TestLib/TerminalWhen", expected: "pass" },
  { name: "TestLib/SimpleFunctionDef", expected: "pass" },
  { name: "TestLib/FuncInline", expected: "pass" },
  { name: "TestLib/AdaptiveRKTest", expected: "pass" },
  { name: "TestLib/SmallFor", expected: "pass" },
  { name: "TestLib/ForBound1", expected: "pass" },
  { name: "TestLib/BigFor", expected: "pass" },
  { name: "TestLib/BadConnect", expected: "fail", notes: "Incompatible connector" },
  { name: "TestLib/AliasRemoval", expected: "pass" },
  { name: "TestLib/BackendDaeInfo", expected: "pass" },
  { name: "TestLib/ConstraintEq", expected: "pass" },
  { name: "TestLib/MathBuiltins", expected: "pass" },
  { name: "TestLib/NestedDerTest", expected: "pass" },
  { name: "TestLib/AnnotationParse", expected: "pass" },
  { name: "TestLib/SimpleRecord", expected: "pass" },
  { name: "TestLib/SimpleBlockTest", expected: "pass" },
  { name: "TestLib/SimpleBlock", expected: "pass" },
  { name: "TestLib/RecordEqTest", expected: "pass" },
  { name: "TestLib/ConnectInWhen", expected: "pass" },
  { name: "TestLib/MultiOutputFunc", expected: "pass" },
  { name: "TestLib/PreEdgeChange", expected: "pass" },
  { name: "TestLib/SimpleTest", expected: "pass" },
  { name: "TestLib/MathTest", expected: "pass" },
  { name: "TestLib/ForTest", expected: "pass" },
  { name: "TestLib/WhenTest", expected: "pass" },
  { name: "TestLib/BouncingBall", expected: "pass" },
  { name: "TestLib/BLTTest", expected: "pass" },
  { name: "TestLib/TearingTest", expected: "pass" },
  { name: "TestLib/ArrayTest", expected: "pass" },
  { name: "TestLib/ArrayLoopTest", expected: "pass", notes: "CG1-4 array run" },
  { name: "TestLib/IfEqTest", expected: "pass" },
  { name: "TestLib/AssertTerminateTest", expected: "pass" },
  { name: "TestLib/LibraryTest", expected: "pass" },
  { name: "TestLib/MSLBlocksTest", expected: "pass" },
  { name: "TestLib/MSLTransferFunctionTest", expected: "pass" },
  { name: "TestLib/SIunitsTest", expected: "pass" },
];

const featureToCasesRaw: Record<string, string[]> = {
  "T1-1": ["TestLib/NoEventTest", "TestLib/NoEventInWhen", "TestLib/NoEventInAlg"],
  "T1-2": ["TestLib/TerminalWhen"],
  "T1-3": ["TestLib/SimpleFunctionDef"],
  "T1-4": ["TestLib/FuncInline"],
  "F1-1": ["TestLib/SimpleRecord", "TestLib/RecordEqTest"],
  "F1-2": ["TestLib/SimpleBlockTest", "TestLib/SimpleBlock"],
  "F2-1": ["TestLib/NestedDerTest"],
  "F2-2": ["TestLib/PreEdgeChange"],
  "T2-1": ["TestLib/SmallFor", "TestLib/ForBound1", "TestLib/BigFor"],
  "T2-2": ["TestLib/BadConnect"],
  "T2-3": ["TestLib/ConstraintEq"],
  "IR1": ["TestLib/BackendDaeInfo", "TestLib/SimpleTest"],
  "IR2-3": ["TestLib/AliasRemoval"],
  "IR3": ["TestLib/InitDummy", "TestLib/InitTwoVars", "TestLib/InitAlg", "TestLib/InitWhen"],
  "T3-1": ["TestLib/SolvableBlock4Res", "TestLib/SolvableBlockMultiRes", "TestLib/AlgebraicLoopWarn", "TestLib/BLTTest"],
  "T3-2": ["TestLib/TearingTest"],
  "T3-3": ["TestLib/JacobianTest"],
  "IR4-4": ["TestLib/JacobianTest"],
  "T4-1": ["TestLib/AdaptiveRKTest"],
  "RT1-1": ["TestLib/WhenTest", "TestLib/BouncingBall"],
  "F4-1": ["TestLib/ConnectInWhen"],
  "F4-3": ["TestLib/IfEqTest"],
  "F4-4": ["TestLib/AssertTerminateTest"],
  "F4-6": ["TestLib/RecordEqTest"],
  "F3-3": ["TestLib/MultiOutputFunc"],
  "MSL-2": ["TestLib/MSLBlocksTest", "TestLib/LibraryTest", "TestLib/MSLTransferFunctionTest"],
  "MSL-3": ["TestLib/MathBuiltins", "TestLib/LibraryTest"],
  "CG1-4": ["TestLib/ArrayLoopTest", "TestLib/ArrayTest"],
  "DBG-1": ["TestLib/BackendDaeInfo"],
};

export const featureToCases: Record<string, string[]> = featureToCasesRaw;

const caseToFeaturesRaw: Record<string, string[]> = {};
for (const [fid, caseList] of Object.entries(featureToCasesRaw)) {
  for (const c of caseList) {
    if (!caseToFeaturesRaw[c]) caseToFeaturesRaw[c] = [];
    caseToFeaturesRaw[c].push(fid);
  }
}
export const caseToFeatures: Record<string, string[]> = caseToFeaturesRaw;

export function getFeaturesByCategory(): Map<string, JitFeature[]> {
  const map = new Map<string, JitFeature[]>();
  for (const f of features) {
    const list = map.get(f.category) ?? [];
    list.push(f);
    map.set(f.category, list);
  }
  return map;
}

export function isCaseCoveringFeature(caseName: string, featureId: string): boolean {
  return (featureToCases[featureId] ?? []).includes(caseName);
}
