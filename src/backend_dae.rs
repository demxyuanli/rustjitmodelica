// Explicit DAE representation for backend (IR1-1, IR1-2).
// Equations represented as 0 = F(x, x', u, t) with clear state/derivative/algebraic/input sets.
// Initial and simulation equation systems are separated; same backend pipeline can process both.

use std::collections::HashSet;

use crate::ast::Equation;

/// Block type for partitioning (IR1-3): explicit single eq, torn nonlinear system, or mixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BlockType {
    /// Single equation (explicit or linear)
    Single,
    /// Torn algebraic loop (nonlinear system)
    Torn,
    /// Mixed continuous/discrete (future)
    Mixed,
}

/// One strongly connected component / block after BLT (IR1-3).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BlockInfo {
    pub block_type: BlockType,
    pub equation_count: usize,
    pub unknown_count: usize,
    /// For torn blocks: number of residual equations in Newton loop
    pub residual_count: Option<usize>,
}

/// Variable classification for explicit DAE: 0 = F(x, x', z, u, t)
#[derive(Debug, Clone, Default)]
pub struct DaeVariableSets {
    /// State variables x
    pub states: Vec<String>,
    /// Derivative variables der(x) (symbolic names like der_x)
    pub derivatives: Vec<String>,
    /// Algebraic variables z
    pub algebraic: Vec<String>,
    /// Input variables u (from connector inputs / top-level inputs)
    pub inputs: Vec<String>,
    /// Discrete variables (kept separate for when/pre)
    pub discrete: Vec<String>,
    /// Parameters (constant)
    pub parameters: Vec<String>,
}

impl DaeVariableSets {
    pub fn state_count(&self) -> usize {
        self.states.len()
    }
    pub fn derivative_count(&self) -> usize {
        self.derivatives.len()
    }
    pub fn algebraic_count(&self) -> usize {
        self.algebraic.len()
    }
    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }
    pub fn discrete_count(&self) -> usize {
        self.discrete.len()
    }
    pub fn parameter_count(&self) -> usize {
        self.parameters.len()
    }

    #[allow(dead_code)]
    pub fn all_variable_count(&self) -> usize {
        self.states.len()
            + self.derivatives.len()
            + self.algebraic.len()
            + self.inputs.len()
            + self.discrete.len()
            + self.parameters.len()
    }

    #[allow(dead_code)]
    pub fn state_set(&self) -> HashSet<&str> {
        self.states.iter().map(String::as_str).collect()
    }
    #[allow(dead_code)]
    pub fn derivative_set(&self) -> HashSet<&str> {
        self.derivatives.iter().map(String::as_str).collect()
    }
    #[allow(dead_code)]
    pub fn algebraic_set(&self) -> HashSet<&str> {
        self.algebraic.iter().map(String::as_str).collect()
    }
    #[allow(dead_code)]
    pub fn input_set(&self) -> HashSet<&str> {
        self.inputs.iter().map(String::as_str).collect()
    }
}

/// SYNC-2: One clock partition for solver/event handling (vars updated on same clock).
#[derive(Debug, Clone, Default)]
pub struct ClockPartition {
    pub id: String,
    pub var_names: HashSet<String>,
}

/// Explicit DAE system: variable sets + equation counts + blocks (IR1-1, IR1-3).
/// Residual form: 0 = F(x, x', z, u, t).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DaeSystem {
    pub variables: DaeVariableSets,
    pub differential_equation_count: usize,
    pub algebraic_equation_count: usize,
    pub total_equation_count: usize,
    pub when_equation_count: usize,
    pub single_equation_count: usize,
    pub torn_block_count: usize,
    pub torn_unknowns_total: usize,
    pub differential_index: u32,
    pub constraint_equation_count: usize,
    /// Partitioning: strongly connected components with block type (IR1-3).
    pub blocks: Vec<BlockInfo>,
    /// SYNC-2: Clocked variable partitions for solver/event handling.
    pub clock_partitions: Vec<ClockPartition>,
}

/// Initial equation system: same structure as DaeSystem but for initial equations only.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct InitialDae {
    pub equation_count: usize,
    pub variable_count: usize,
}

/// Simulation DAE (continuous time): full explicit form.
#[derive(Debug, Clone)]
pub struct SimulationDae {
    pub dae: DaeSystem,
    /// Optional: initial system for consistent initialization
    pub initial: InitialDae,
}

impl SimulationDae {
    #[allow(dead_code)]
    pub fn state_count(&self) -> usize {
        self.dae.variables.state_count()
    }
    #[allow(dead_code)]
    pub fn algebraic_count(&self) -> usize {
        self.dae.variables.algebraic_count()
    }
    #[allow(dead_code)]
    pub fn total_equations(&self) -> usize {
        self.dae.total_equation_count
    }
    #[allow(dead_code)]
    pub fn initial_equation_count(&self) -> usize {
        self.initial.equation_count
    }
}

/// Build SimulationDae from backend data (states, algebraic, sorted equations, initial equations).
/// IR1-2: initial_equation_count and initial_variable_count describe the separate initial system.
pub fn build_simulation_dae(
    state_vars: &[String],
    discrete_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    input_var_names: &[String],
    differential_equation_count: usize,
    sorted_algebraic_equations: &[Equation],
    initial_equation_count: usize,
    initial_variable_count: usize,
    when_equation_count: usize,
    differential_index: u32,
    constraint_equation_count: usize,
    clock_partitions: &[ClockPartition],
) -> SimulationDae {
    let state_set: HashSet<&str> = state_vars.iter().map(String::as_str).collect();
    let discrete_set: HashSet<&str> = discrete_vars.iter().map(String::as_str).collect();
    let param_set: HashSet<&str> = param_vars.iter().map(String::as_str).collect();
    let input_set: HashSet<&str> = input_var_names.iter().map(String::as_str).collect();

    let derivatives: Vec<String> = state_vars
        .iter()
        .map(|s| format!("der_{}", s))
        .collect();

    let mut algebraic = Vec::new();
    for v in output_vars {
        if state_set.contains(v.as_str())
            || v.starts_with("der_")
            || discrete_set.contains(v.as_str())
            || param_set.contains(v.as_str())
            || input_set.contains(v.as_str())
        {
            continue;
        }
        algebraic.push(v.clone());
    }

    let variables = DaeVariableSets {
        states: state_vars.to_vec(),
        derivatives,
        algebraic,
        inputs: input_var_names.to_vec(),
        discrete: discrete_vars.to_vec(),
        parameters: param_vars.to_vec(),
    };

    let mut single_equation_count = 0usize;
    let mut torn_block_count = 0usize;
    let mut torn_unknowns_total = 0usize;
    let mut blocks: Vec<BlockInfo> = Vec::new();
    for eq in sorted_algebraic_equations {
        match eq {
            Equation::Simple(_, _) | Equation::For(_, _, _, _) | Equation::If(_, _, _, _) | Equation::Assert(_, _) | Equation::Terminate(_) | Equation::MultiAssign(_, _) => {
                single_equation_count += 1;
                blocks.push(BlockInfo {
                    block_type: BlockType::Single,
                    equation_count: 1,
                    unknown_count: 1,
                    residual_count: None,
                });
            }
            Equation::SolvableBlock { unknowns, residuals, .. } => {
                torn_block_count += 1;
                torn_unknowns_total += unknowns.len();
                blocks.push(BlockInfo {
                    block_type: BlockType::Torn,
                    equation_count: unknowns.len(),
                    unknown_count: unknowns.len(),
                    residual_count: Some(residuals.len()),
                });
            }
            _ => {}
        }
    }

    let algebraic_equation_count = sorted_algebraic_equations.len();
    let total_equation_count = differential_equation_count + algebraic_equation_count;

    let initial = InitialDae {
        equation_count: initial_equation_count,
        variable_count: initial_variable_count,
    };

    let dae = DaeSystem {
        variables,
        differential_equation_count,
        algebraic_equation_count,
        total_equation_count,
        when_equation_count,
        single_equation_count,
        torn_block_count,
        torn_unknowns_total,
        differential_index,
        constraint_equation_count,
        blocks,
        clock_partitions: clock_partitions.to_vec(),
    };

    SimulationDae { dae, initial }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dae_variable_sets_counts() {
        let v = DaeVariableSets {
            states: vec!["x".into()],
            derivatives: vec!["der_x".into()],
            algebraic: vec!["y".into()],
            inputs: vec![],
            discrete: vec![],
            parameters: vec!["p".into()],
        };
        assert_eq!(v.state_count(), 1);
        assert_eq!(v.derivative_count(), 1);
        assert_eq!(v.algebraic_count(), 1);
        assert_eq!(v.all_variable_count(), 4);
    }
}
