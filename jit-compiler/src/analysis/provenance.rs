//! Equation provenance tracking for incremental recompilation.
//!
//! This module provides fine-grained dependency tracking at the equation/variable level,
//! enabling minimal recompilation scope when parameters or components change.
//!
//! ## Design Notes (Audit 2026-04)
//!
//! **轨道 A vs 轨道 B**：
//! - 轨道 A（前端/展平）：源码或依赖闭包变化 → 阶段缓存失效 → 度量 `flatten_wall_us`
//! - 轨道 B（Codegen）：编译期常量固化导致 IR 变化 → 增量代码生成（谨慎）→ 度量 `codegen_wall_us`
//!
//! **关键约束**：运行时参数 `p` 变化不应触发 codegen，只需更新内存中的参数向量。

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Provenance information for a single equation in a flattened model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquationProvenance {
    /// Index in the flattened model's equation list.
    pub flat_eq_index: usize,
    /// Short label for debugging (e.g., "x = y + 1" or "eq[5]").
    pub label: String,
    /// Source file that contributed this equation.
    pub source_file: Option<String>,
    /// Component instance path (e.g., "resistor1.R" or "circuit.resistor").
    pub instance_path: Option<String>,
    /// Variables this equation depends on (reads from).
    pub depends_on_vars: Vec<String>,
    /// Variables this equation solves for (writes to).
    pub solves_vars: Vec<String>,
    /// Whether this is an initial equation.
    pub is_initial: bool,
    /// Whether this is a when-equation.
    pub is_when: bool,
}

/// Variable dependency information for incremental analysis.
#[derive(Debug, Clone, Default)]
pub struct VarDependencyInfo {
    /// Variables that directly depend on this variable (reverse of depends_on).
    /// e.g., if equation `y = x + 1`, then `x.dependents` contains `y`.
    pub dependents: HashSet<String>,
    /// Equations that read this variable.
    pub read_by_equations: Vec<usize>,
    /// Equations that write to this variable.
    pub written_by_equations: Vec<usize>,
}

/// Complete provenance index for a flattened model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProvenanceIndex {
    /// Per-equation provenance.
    pub equations: Vec<EquationProvenance>,
    /// Variable → dependency info (reverse index).
    pub var_dependencies: HashMap<String, VarDependencyInfoSerialized>,
    /// Parameter → dependent variables (transitive closure, computed at build time).
    pub param_to_dependent_vars: HashMap<String, Vec<String>>,
    /// Component instance → type name.
    pub instance_types: HashMap<String, String>,
}

/// Serialized form of VarDependencyInfo (HashSet → Vec).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarDependencyInfoSerialized {
    pub dependents: Vec<String>,
    pub read_by_equations: Vec<usize>,
    pub written_by_equations: Vec<usize>,
}

impl From<VarDependencyInfo> for VarDependencyInfoSerialized {
    fn from(info: VarDependencyInfo) -> Self {
        let mut dependents: Vec<String> = info.dependents.into_iter().collect();
        dependents.sort();
        Self {
            dependents,
            read_by_equations: info.read_by_equations,
            written_by_equations: info.written_by_equations,
        }
    }
}

/// Result of computing the impact of a change.
#[derive(Debug, Clone)]
pub struct ChangeImpact {
    /// Variables affected by the change.
    pub affected_vars: HashSet<String>,
    /// Equation indices that need recompilation.
    pub affected_equations: HashSet<usize>,
    /// Whether the change requires full model re-flatten.
    pub requires_full_reflatten: bool,
    /// Reason for full re-flatten, if required.
    pub reflatten_reason: Option<String>,
}

impl ProvenanceIndex {
    /// Create an empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute the impact of changing specific parameters.
    ///
    /// This is for **轨道 A** analysis (impact on frontend/flatten), NOT for
    /// determining whether codegen is needed. Runtime parameter changes
    /// should NOT trigger codegen at all.
    pub fn compute_param_change_impact(&self, param_names: &[&str]) -> ChangeImpact {
        let mut affected_vars: HashSet<String> = HashSet::new();
        let mut affected_equations: HashSet<usize> = HashSet::new();

        // Step 1: Get directly affected variables from pre-computed transitive closure
        for param in param_names {
            if let Some(deps) = self.param_to_dependent_vars.get(*param) {
                for var in deps {
                    affected_vars.insert(var.clone());
                }
            }
        }

        // Step 2: Find all equations that involve affected variables
        for var in &affected_vars {
            if let Some(info) = self.var_dependencies.get(var) {
                affected_equations.extend(info.read_by_equations.iter().copied());
                affected_equations.extend(info.written_by_equations.iter().copied());
            }
        }

        ChangeImpact {
            affected_vars,
            affected_equations,
            requires_full_reflatten: false,
            reflatten_reason: None,
        }
    }

    /// Compute the impact of changing a component instance.
    pub fn compute_instance_change_impact(&self, instance_path: &str) -> ChangeImpact {
        let mut affected_equations: HashSet<usize> = HashSet::new();

        // Find all equations from this instance
        for eq in &self.equations {
            if let Some(ref path) = eq.instance_path {
                if path == instance_path || path.starts_with(&format!("{}.", instance_path)) {
                    affected_equations.insert(eq.flat_eq_index);
                }
            }
        }

        // Also affect equations that depend on variables from this instance
        let prefix = format!("{}.", instance_path);
        for (var, info) in &self.var_dependencies {
            if var == instance_path || var.starts_with(&prefix) {
                affected_equations.extend(info.read_by_equations.iter().copied());
                affected_equations.extend(info.written_by_equations.iter().copied());
            }
        }

        let affected_vars: HashSet<String> = affected_equations
            .iter()
            .flat_map(|&idx| {
                self.equations
                    .get(idx)
                    .map(|eq| eq.solves_vars.iter().cloned())
                    .into_iter()
                    .flatten()
            })
            .collect();

        ChangeImpact {
            affected_vars,
            affected_equations,
            requires_full_reflatten: false,
            reflatten_reason: None,
        }
    }

    /// Check if incremental recompilation is worthwhile.
    ///
    /// Returns false if >30% of equations are affected (heuristic threshold).
    pub fn is_incremental_worthwhile(&self, impact: &ChangeImpact) -> bool {
        let total = self.equations.len();
        if total == 0 {
            return false;
        }

        let affected_ratio = impact.affected_equations.len() as f64 / total as f64;
        affected_ratio < 0.3
    }

    /// Get statistics about the provenance index.
    pub fn stats(&self) -> ProvenanceStats {
        ProvenanceStats {
            equation_count: self.equations.len(),
            variable_count: self.var_dependencies.len(),
            parameter_count: self.param_to_dependent_vars.len(),
            instance_count: self.instance_types.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProvenanceStats {
    pub equation_count: usize,
    pub variable_count: usize,
    pub parameter_count: usize,
    pub instance_count: usize,
}

/// Builder for constructing a ProvenanceIndex during flattening.
#[derive(Debug, Default)]
pub struct ProvenanceBuilder {
    equations: Vec<EquationProvenance>,
    var_dependencies: HashMap<String, VarDependencyInfo>,
    /// Direct param → dependent vars (will be transitively closed in build())
    param_direct_deps: HashMap<String, HashSet<String>>,
    /// Variable → variables it depends on (for transitive closure)
    var_depends_on: HashMap<String, HashSet<String>>,
    instance_types: HashMap<String, String>,
    known_params: HashSet<String>,
}

impl ProvenanceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a parameter (to distinguish from regular variables).
    pub fn register_parameter(&mut self, name: &str) {
        self.known_params.insert(name.to_string());
    }

    /// Register a component instance.
    pub fn register_instance(&mut self, instance_path: &str, type_name: &str) {
        self.instance_types.insert(instance_path.to_string(), type_name.to_string());
    }

    /// Record an equation with its dependencies.
    pub fn record_equation(
        &mut self,
        eq_index: usize,
        label: String,
        source_file: Option<String>,
        instance_path: Option<String>,
        depends_on_vars: Vec<String>,
        solves_vars: Vec<String>,
        is_initial: bool,
        is_when: bool,
    ) {
        // Record equation provenance
        self.equations.push(EquationProvenance {
            flat_eq_index: eq_index,
            label,
            source_file,
            instance_path,
            depends_on_vars: depends_on_vars.clone(),
            solves_vars: solves_vars.clone(),
            is_initial,
            is_when,
        });

        // Update variable dependencies (forward: var → what it depends on)
        for solved in &solves_vars {
            let deps = self.var_depends_on.entry(solved.clone()).or_default();
            for dep in &depends_on_vars {
                deps.insert(dep.clone());
            }
        }

        // Update reverse dependencies (backward: var → who depends on it)
        for dep in &depends_on_vars {
            let info = self.var_dependencies.entry(dep.clone()).or_default();
            info.read_by_equations.push(eq_index);
            for solved in &solves_vars {
                info.dependents.insert(solved.clone());
            }
        }

        for solved in &solves_vars {
            let info = self.var_dependencies.entry(solved.clone()).or_default();
            info.written_by_equations.push(eq_index);
        }

        // Track direct param → dependent vars
        for dep in &depends_on_vars {
            if self.known_params.contains(dep) {
                for solved in &solves_vars {
                    self.param_direct_deps
                        .entry(dep.clone())
                        .or_default()
                        .insert(solved.clone());
                }
            }
        }
    }

    /// Compute transitive closure of param dependencies.
    ///
    /// For each param P, computes all variables that transitively depend on P.
    /// e.g., if R → i (direct) and i → p (via equation) and p → heat, then
    /// R's transitive closure includes {i, p, heat}.
    fn compute_param_transitive_closure(&self) -> HashMap<String, Vec<String>> {
        let mut result: HashMap<String, HashSet<String>> = HashMap::new();

        // Start with direct dependencies
        for (param, direct_deps) in &self.param_direct_deps {
            let closure = result.entry(param.clone()).or_default();
            closure.extend(direct_deps.iter().cloned());
        }

        // Build reverse index: var → vars that solve equations depending on var
        // i.e., if equation `y = f(x)` then `x.reverse_deps` contains `y`
        let mut reverse_deps: HashMap<String, HashSet<String>> = HashMap::new();
        for (solved_var, deps) in &self.var_depends_on {
            for dep_var in deps {
                reverse_deps
                    .entry(dep_var.clone())
                    .or_default()
                    .insert(solved_var.clone());
            }
        }

        // Propagate forward: if var A is in closure and A → B (B depends on A),
        // then B should also be in closure
        let mut changed = true;
        while changed {
            changed = false;
            for (_, closure) in result.iter_mut() {
                let mut to_add: HashSet<String> = HashSet::new();
                for var in closure.iter() {
                    // Find all vars that depend on this var
                    if let Some(forward_deps) = reverse_deps.get(var) {
                        for dep_var in forward_deps {
                            // Only add if it's a non-parameter variable and not already in closure
                            if !self.known_params.contains(dep_var) && !closure.contains(dep_var) {
                                to_add.insert(dep_var.clone());
                            }
                        }
                    }
                }
                if !to_add.is_empty() {
                    changed = true;
                    closure.extend(to_add);
                }
            }
        }

        // Convert to sorted Vec
        result
            .into_iter()
            .map(|(k, v)| {
                let mut sorted: Vec<String> = v.into_iter().collect();
                sorted.sort();
                (k, sorted)
            })
            .collect()
    }

    /// Finalize and build the index.
    pub fn build(self) -> ProvenanceIndex {
        // Compute transitive closure for param dependencies FIRST (before moving self)
        let param_to_dependent_vars = self.compute_param_transitive_closure();

        // Convert VarDependencyInfo to serialized form
        let var_dependencies: HashMap<String, VarDependencyInfoSerialized> = self
            .var_dependencies
            .into_iter()
            .map(|(k, v)| (k, v.into()))
            .collect();

        ProvenanceIndex {
            equations: self.equations,
            var_dependencies,
            param_to_dependent_vars,
            instance_types: self.instance_types,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provenance_builder() {
        let mut builder = ProvenanceBuilder::new();
        builder.register_parameter("R");
        builder.register_instance("resistor1", "Resistor");

        builder.record_equation(
            0,
            "i = v / R".to_string(),
            Some("resistor.mo".to_string()),
            Some("resistor1".to_string()),
            vec!["v".to_string(), "R".to_string()],
            vec!["i".to_string()],
            false,
            false,
        );

        let index = builder.build();
        assert_eq!(index.equations.len(), 1);
        assert!(index.var_dependencies.contains_key("v"));
        assert!(index.var_dependencies.contains_key("i"));
        assert!(index.param_to_dependent_vars.contains_key("R"));

        // Check that v has i as dependent
        let v_info = index.var_dependencies.get("v").unwrap();
        assert!(v_info.dependents.contains(&"i".to_string()));
    }

    #[test]
    fn test_change_impact_with_transitive_deps() {
        let mut builder = ProvenanceBuilder::new();
        builder.register_parameter("R");

        // i = v / R (i depends on R)
        builder.record_equation(
            0,
            "i = v / R".to_string(),
            None,
            None,
            vec!["v".to_string(), "R".to_string()],
            vec!["i".to_string()],
            false,
            false,
        );

        // p = i * i * R (p depends on i and R)
        builder.record_equation(
            1,
            "p = i * i * R".to_string(),
            None,
            None,
            vec!["i".to_string(), "R".to_string()],
            vec!["p".to_string()],
            false,
            false,
        );

        // heat = p * t (heat depends on p)
        builder.record_equation(
            2,
            "heat = p * t".to_string(),
            None,
            None,
            vec!["p".to_string(), "t".to_string()],
            vec!["heat".to_string()],
            false,
            false,
        );

        let index = builder.build();

        // Check transitive closure: R → i, p, heat
        let r_deps = index.param_to_dependent_vars.get("R").unwrap();
        assert!(r_deps.contains(&"i".to_string()));
        assert!(r_deps.contains(&"p".to_string()));
        assert!(r_deps.contains(&"heat".to_string()), "heat should be in transitive closure of R");

        let impact = index.compute_param_change_impact(&["R"]);
        assert!(impact.affected_vars.contains("i"));
        assert!(impact.affected_vars.contains("p"));
        assert!(impact.affected_vars.contains("heat"));
        assert!(impact.affected_equations.contains(&0));
        assert!(impact.affected_equations.contains(&1));
        assert!(impact.affected_equations.contains(&2));
    }

    #[test]
    fn test_var_dependents_tracking() {
        let mut builder = ProvenanceBuilder::new();

        // y = x + 1
        builder.record_equation(
            0,
            "y = x + 1".to_string(),
            None,
            None,
            vec!["x".to_string()],
            vec!["y".to_string()],
            false,
            false,
        );

        // z = y * 2
        builder.record_equation(
            1,
            "z = y * 2".to_string(),
            None,
            None,
            vec!["y".to_string()],
            vec!["z".to_string()],
            false,
            false,
        );

        let index = builder.build();

        // x.dependents should contain y
        let x_info = index.var_dependencies.get("x").unwrap();
        assert!(x_info.dependents.contains(&"y".to_string()));

        // y.dependents should contain z
        let y_info = index.var_dependencies.get("y").unwrap();
        assert!(y_info.dependents.contains(&"z".to_string()));
    }

    #[test]
    fn test_incremental_worthwhile() {
        let mut builder = ProvenanceBuilder::new();

        // Add 10 equations
        for i in 0..10 {
            builder.record_equation(
                i,
                format!("eq_{}", i),
                None,
                None,
                vec![format!("x{}", i)],
                vec![format!("y{}", i)],
                false,
                false,
            );
        }

        let index = builder.build();

        // 2 affected equations = 20% < 30%, should be worthwhile
        let impact = ChangeImpact {
            affected_vars: Default::default(),
            affected_equations: [0, 1].into_iter().collect(),
            requires_full_reflatten: false,
            reflatten_reason: None,
        };
        assert!(index.is_incremental_worthwhile(&impact));

        // 5 affected equations = 50% > 30%, should NOT be worthwhile
        let impact = ChangeImpact {
            affected_vars: Default::default(),
            affected_equations: [0, 1, 2, 3, 4].into_iter().collect(),
            requires_full_reflatten: false,
            reflatten_reason: None,
        };
        assert!(!index.is_incremental_worthwhile(&impact));
    }
}
