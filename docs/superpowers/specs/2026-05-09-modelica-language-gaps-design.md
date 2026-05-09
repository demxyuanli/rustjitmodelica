# Modelica Language Feature Gaps — Design Spec

> Date: 2026-05-09
> 6 sub-items completing Modelica spec coverage from ~85% to ~92%

---

## 1. Enumeration Type Safety

**Current**: `type E = enumeration(a, b, c);` maps to `type E = Integer;`. Literals like `E.a` are discarded.

**Changes**:
- `ast.rs`: new `TypeName::Enumeration(Vec<String>)` variant; `type_aliases` extended
- `parser/decl_parse.rs`: parse `enumeration_type` into `TypeName::Enumeration`
- `flatten/inheritance.rs`: enum literal access `E.a` → validate membership, map to integer index
- `flatten/connections.rs`: type compatibility check for enum connectors

## 2. pure/impure Semantics

**Current**: Parsed in `class_prefixes` but discarded.

**Changes**:
- `ast.rs Model`: add `is_pure: bool`, `is_impure: bool`
- `parser/model_parse.rs`: capture "pure"/"impure" from class_prefixes
- `compiler/inline/rewrite.rs`: pure functions allowed in parameter inlining; impure calls preserved

## 3. encapsulated Semantics

**Current**: Parsed in `class_prefixes` but discarded.

**Changes**:
- `ast.rs Model`: add `is_encapsulated: bool`
- `parser/model_parse.rs`, `entry.rs`, `decl_parse.rs`: capture "encapsulated"
- `flatten/import_resolve/`: encapsulated model imports don't leak to outer scope

## 4. within Clause

**Current**: Parsed but skipped. Namespace via loader import system.

**Changes**:
- `ast.rs`: add `StoredDefinition { within_clause, items }`
- `modelica.pest`: update `stored_definition` rule
- `parser/entry.rs`: return within clause with parsed items
- `loader.rs`: auto-add package prefix from within clause to search path

## 5. Annotation Preservation

**Current**: Most annotations discarded during flatten. Only vendor/library kept.

**Changes**:
- `flatten/decl_expand/`: preserve component annotations through instantiation
- `compiler/mod.rs`: expose annotations in CompilerArtifacts

## 6. Array Dimension Symbolic Evaluation

**Current**: Non-const dimensions require `--array-sizes-json` external override.

**Changes**:
- `flatten/decl_expand/flattener_impl.rs`: use parameter context to eval_const_expr on array dimensions

## Implementation Order

1. encapsulated (smallest, same pattern as partial/expandable)
2. pure/impure (same pattern)
3. within (grammar change, low risk)
4. annotation (preservation, no semantic change)
5. 枚举 (largest, new type variant)
6. 数组维度 (heuristic evaluation, may need edge case handling)

Each independently testable. Total ~4 days.
