// Multi-language support for compiler and simulation messages.
// Language: env RUSTMODLICA_LANG (en|zh) or CLI --lang=en|zh. Default: en.

use std::fmt::Display;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Zh,
}

impl Lang {
    pub fn from_env() -> Self {
        let s = match std::env::var("RUSTMODLICA_LANG") {
            Ok(v) => v.to_lowercase(),
            Err(_) => return Lang::En,
        };
        match s.as_str() {
            "zh" | "zh-cn" | "zh_cn" => Lang::Zh,
            _ => Lang::En,
        }
    }
}

fn template(lang: Lang, key: &str) -> &'static str {
    let (en, zh) = match key {
        "loading_model" => ("Loading model '{}'...", "\u{6b63}\u{5728}\u{52a0}\u{8f7d}\u{6a21}\u{578b} '{}'..."),
        "evaluating_function_args" => ("Evaluating function with --function-args...", "\u{6b63}\u{5728}\u{4f7f}\u{7528} --function-args \u{6c42}\u{503c}\u{51fd}\u{6570}..."),
        "evaluating_function_default" => ("Evaluating function with default inputs (0.0)...", "\u{6b63}\u{5728}\u{4f7f}\u{7528}\u{9ed8}\u{8ba4}\u{8f93}\u{5165} (0.0) \u{6c42}\u{503c}\u{51fd}\u{6570}..."),
        "flattening_model" => ("Flattening model...", "\u{6b63}\u{5728}\u{6241}\u{5e73}\u{5316}\u{6a21}\u{578b}..."),
        "flattened_equations" => ("  Flattened equations: {}", "  \u{6241}\u{5e73}\u{5316}\u{65b9}\u{7a0b}\u{6570}: {}"),
        "flattened_declarations" => ("  Flattened declarations: {}", "  \u{6241}\u{5e73}\u{5316}\u{58f0}\u{660e}\u{6570}: {}"),
        "analyzing_variables" => ("Analyzing variables...", "\u{6b63}\u{5728}\u{5206}\u{6790}\u{53d8}\u{91cf}..."),
        "normalizing_derivatives" => ("Normalizing derivatives...", "\u{6b63}\u{5728}\u{89c4}\u{8303}\u{5316}\u{5bfc}\u{6570}..."),
        "performing_structure_analysis" => ("Performing Structure Analysis...", "\u{6b63}\u{5728}\u{8fdb}\u{884c}\u{7ed3}\u{6784}\u{5206}\u{6790}..."),
        "jit_compiling" => ("JIT Compiling...", "\u{6b63}\u{5728} JIT \u{7f16}\u{8bd1}..."),
        "equations_after_sorting" => ("  Equations after aliasing/sorting: {}", "  \u{6392}\u{5e8f}\u{540e}\u{65b9}\u{7a0b}\u{6570}: {}"),
        "state_variables" => ("  State variables: {}", "  \u{72b6}\u{6001}\u{53d8}\u{91cf}\u{6570}: {}"),
        "discrete_variables" => ("  Discrete variables: {}", "  \u{79bb}\u{6563}\u{53d8}\u{91cf}\u{6570}: {}"),
        "parameters_count" => ("  Parameters: {}", "  \u{53c2}\u{6570}\u{6570}: {}"),
        "aliases_eliminated" => ("  Aliases eliminated: {}", "  \u{5df2}\u{6d88}\u{9664}\u{522b}\u{540d}\u{6570}: {}"),
        "remaining_equations" => ("  Remaining equations after alias elim: {}", "  \u{6d88}\u{9664}\u{522b}\u{540d}\u{540e}\u{5269}\u{4f59}\u{65b9}\u{7a0b}\u{6570}: {}"),
        "compilation_successful" => ("Compilation successful!", "\u{7f16}\u{8bd1}\u{6210}\u{529f}\u{ff01}"),
        "states" => ("  States: {}", "  \u{72b6}\u{6001}\u{6570}: {}"),
        "discrete_vars" => ("  Discrete Vars: {}", "  \u{79bb}\u{6563}\u{53d8}\u{91cf}\u{6570}: {}"),
        "parameters" => ("  Parameters: {}", "  \u{53c2}\u{6570}\u{6570}: {}"),
        "outputs" => ("  Outputs: {}", "  \u{8f93}\u{51fa}\u{6570}: {}"),
        "when_statements" => ("  When Statements: {}", "  When \u{8bed}\u{53e5}\u{6570}: {}"),
        "zero_crossings" => ("  Zero-Crossings: {}", "  \u{8fc7}\u{96f6}\u{68c0}\u{6d4b}\u{6570}: {}"),
        "starting_simulation" => ("Starting simulation...", "\u{5f00}\u{59cb}\u{4eff}\u{771f}..."),
        "simulation_completed" => ("Simulation completed.", "\u{4eff}\u{771f}\u{5b8c}\u{6210}\u{3002}"),
        "result" => ("Result: {}", "\u{7ed3}\u{679c}: {}"),
        "compiling" => ("Compiling {}...", "\u{6b63}\u{5728}\u{7f16}\u{8bd1} {}..."),
        "warnings_generated" => ("{} warning(s) generated", "\u{751f}\u{6210}\u{4e86} {} \u{6761}\u{8b66}\u{544a}"),
        "time" => ("Time", "\u{65f6}\u{95f4}"),
        "adaptive_rk45_steps" => ("Adaptive RK45 total steps: {}", "\u{81ea}\u{9002}\u{5e94} RK45 \u{603b}\u{6b65}\u{6570}: {}"),
        "loading_dependency" => ("Loading dependency: {}", "\u{6b63}\u{5728}\u{52a0}\u{8f7d}\u{4f9d}\u{8d56}: {}"),
        "could_not_find_model" => ("Could not find model: {}", "\u{672a}\u{627e}\u{5230}\u{6a21}\u{578b}: {}"),
        "simulation_failed_at" => ("Error: Simulation failed at time {} with status code {}", "\u{9519}\u{8bef}\u{ff1a}\u{4eff}\u{771f}\u{5728}\u{65f6}\u{523b} {} \u{5931}\u{8d25}\u{ff0c}\u{72b6}\u{6001}\u{7801} {}"),
        "simulation_terminated" => ("Simulation terminated by terminate() at t={}", "Simulation terminated by terminate() at t={}"),
        "newton_failure" => ("  Newton-Raphson failure: possible cause max iterations exceeded or Jacobian too small (|J| < 1e-12).", "  Newton-Raphson \u{5931}\u{8d25}\u{ff1a}\u{53ef}\u{80fd}\u{539f}\u{56e0}\u{8d85}\u{8fc7}\u{6700}\u{5927}\u{8fed}\u{4ee3}\u{6b21}\u{6570}\u{6216}\u{96ac}\u{77e9}\u{9635}\u{8fc7}\u{5c0f} (|J| < 1e-12)\u{3002}"),
        "tearing_vars_residual" => ("  Tearing variable(s): {}, residual = {}, value = {}", "  \u{6495}\u{88c2}\u{53d8}\u{91cf}\u{ff1a}{}\u{ff0c}\u{6b8e}\u{5dee} = {}\u{ff0c}\u{503c} = {}"),
        "event_loop_no_converge" => ("Warning: Event loop did not converge at time {}", "\u{8b66}\u{544a}\u{ff1a}\u{4e8b}\u{4ef6}\u{5faa}\u{73af}\u{5728}\u{65f6}\u{523b} {} \u{672a}\u{6536}\u{655b}"),
        "simulation_failed_trial" => ("Error: Simulation failed at time {} (trial step) with status code {}", "\u{9519}\u{8bef}\u{ff1a}\u{4eff}\u{771f}\u{5728}\u{65f6}\u{523b} {} (\u{8bd5}\u{63a2}\u{6b65})\u{5931}\u{8d25}\u{ff0c}\u{72b6}\u{6001}\u{7801} {}"),
        "notification_frontend" => ("Notification: Model statistics after passing the front-end and creating the data structures used by the back-end:", "\u{901a}\u{77e5}\u{ff1a}\u{7a0b}\u{5e8f}\u{524d}\u{7aef}\u{901a}\u{8fc7}\u{5e76}\u{521b}\u{5efa}\u{540e}\u{7aef}\u{6570}\u{636e}\u{7ed3}\u{6784}\u{540e}\u{7684}\u{6a21}\u{578b}\u{7edf}\u{8ba1}\u{ff1a}"),
        "number_of_equations" => (" * Number of equations: {}", " * \u{65b9}\u{7a0b}\u{6570}: {}"),
        "number_of_variables" => (" * Number of variables: {}", " * \u{53d8}\u{91cf}\u{6570}: {}"),
        "notification_dae_form" => ("Notification: Explicit DAE form (0 = F(x, x', z, u, t)):", "\u{901a}\u{77e5}\u{ff1a}\u{663e}\u{5f0f} DAE \u{5f62}\u{5f0f} (0 = F(x, x', z, u, t))\u{ff1a}"),
        "states_x" => (" * States (x): {}", " * \u{72b6}\u{6001} (x): {}"),
        "derivatives" => (" * Derivatives (x'): {}", " * \u{5bfc}\u{6570} (x'): {}"),
        "algebraic_z" => (" * Algebraic (z): {}", " * \u{4ee3}\u{6570} (z): {}"),
        "inputs_u" => (" * Inputs (u): {}", " * \u{8f93}\u{5165} (u): {}"),
        "discrete" => (" * Discrete: {}", " * \u{79bb}\u{6563}: {}"),
        "simulation_equations" => (" * Simulation equations (diff + alg): {}", " * \u{4eff}\u{771f}\u{65b9}\u{7a0b} (\u{5fae}\u{5206}+\u{4ee3}\u{6570}): {}"),
        "initial_equations" => (" * Initial equations: {}", " * \u{521d}\u{59cb}\u{65b9}\u{7a0b}: {}"),
        "constraint_equations" => (" * Constraint equations (before index reduction): {}", " * \u{7ea6}\u{675f}\u{65b9}\u{7a0b} (\u{6307}\u{6807}\u{7ea6}\u{5316}\u{524d}): {}"),
        "notification_backend" => ("Notification: Model statistics after passing the back-end for simulation:", "\u{901a}\u{77e5}\u{ff1a}\u{7a0b}\u{5e8f}\u{540e}\u{7aef}\u{901a}\u{8fc7}\u{540e}\u{7684}\u{6a21}\u{578b}\u{7edf}\u{8ba1}\u{ff1a}"),
        "independent_subsystems" => (" * Number of independent subsystems: 1", " * \u{72ec}\u{7acb}\u{5b50}\u{7cfb}\u{7edf}\u{6570}: 1"),
        "number_of_states" => (" * Number of states: {}{}", " * \u{72b6}\u{6001}\u{6570}: {}{}"),
        "number_of_discrete" => (" * Number of discrete variables: {}{}", " * \u{79bb}\u{6563}\u{53d8}\u{91cf}\u{6570}: {}{}"),
        "clocked_states" => (" * Number of clocked states: 0 ()", " * \u{65f6}\u{949f}\u{72b6}\u{6001}\u{6570}: 0 ()"),
        "top_level_inputs" => (" * Top-level inputs: 0", " * \u{9876}\u{5c42}\u{8f93}\u{5165}: 0"),
        "notification_strong" => ("Notification: Strong component statistics for simulation ({}):", "\u{901a}\u{77e5}\u{ff1a}\u{5f3a}\u{8fde}\u{901a}\u{7ec4}\u{4ef6}\u{7edf}\u{8ba1} ({})\u{ff1a}"),
        "single_equations" => (" * Single equations (assignments): {}", " * \u{5355}\u{65b9}\u{7a0b} (\u{8d4b}\u{503c}): {}"),
        "array_equations" => (" * Array equations: 0", " * \u{6570}\u{7ec4}\u{65b9}\u{7a0b}: 0"),
        "algorithm_blocks" => (" * Algorithm blocks: {}", " * \u{7b97}\u{6cd5}\u{5757}\u{6570}: {}"),
        "record_equations" => (" * Record equations: 0", " * \u{8bb0}\u{5f55}\u{65b9}\u{7a0b}: 0"),
        "when_equations" => (" * When equations: {}", " * When \u{65b9}\u{7a0b}: {}"),
        "if_equations" => (" * If-equations: 0", " * If-\u{65b9}\u{7a0b}: 0"),
        "equation_systems" => (" * Equation systems (not torn): 0", " * \u{65b9}\u{7a0b}\u{7ec4} (\u{672a}\u{6495}\u{88c2}): 0"),
        "torn_equation_systems" => (" * Torn equation systems: {}", " * \u{6495}\u{88c2}\u{65b9}\u{7a0b}\u{7ec4}: {}"),
        "mixed_systems" => (" * Mixed (continuous/discrete) equation systems: 0", " * \u{6df7}\u{5408}\u{65b9}\u{7a0b}\u{7ec4}: 0"),
        "blocks_partitioning" => (" * Blocks (partitioning): {} single, {} torn, {} mixed", " * \u{5757}\u{5206}\u{5272}: {} \u{5355}, {} \u{6495}\u{88c2}, {} \u{6df7}\u{5408}"),
        "notification_backend_details" => ("Notification: Backend details (rustmodlica):", "\u{901a}\u{77e5}\u{ff1a}\u{540e}\u{7aef}\u{8be6}\u{60c5} (rustmodlica)\u{ff1a}"),
        "index_reduction_method" => (" * Index reduction method: {}", " * \u{6307}\u{6807}\u{7ea6}\u{5316}\u{65b9}\u{6cd5}: {}"),
        "differential_index" => (" * Differential index: {}", " * \u{5fae}\u{5206}\u{6307}\u{6807}: {}"),
        "tearing_method" => (" * Tearing method: {}", " * \u{6495}\u{88c2}\u{65b9}\u{6cd5}: {}"),
        "tearing_variables_selected" => (" * Tearing variables (selected): {}", " * \u{6495}\u{88c2}\u{53d8}\u{91cf} (\u{5df2}\u{9009}): {}"),
        "torn_unknowns_total" => (" * Torn unknowns (total in blocks): {}", " * \u{6495}\u{88c2}\u{672a}\u{77e5}\u{6570} (\u{603b}\u{8ba1}): {}"),
        "block_unknowns_min_max" => (" * SolvableBlock unknowns per block: min {}, max {}", " * SolvableBlock \u{6bcf}\u{5757}\u{672a}\u{77e5}\u{6570}: min {} max {}"),
        "block_residuals_min_max" => (" * SolvableBlock residuals per block: min {}, max {}", " * SolvableBlock \u{6bcf}\u{5757}\u{6b8e}\u{5dee}\u{6570}: min {} max {}"),
        "vars_with_equations" => (" * Variables with equations: {}", " * \u{6709}\u{65b9}\u{7a0b}\u{7684}\u{53d8}\u{91cf}\u{6570}: {}"),
        "equations_per_var" => (" * Equations per variable: min {}, max {}, avg {}", " * \u{6bcf}\u{53d8}\u{91cf}\u{65b9}\u{7a0b}\u{6570}: min {} max {} \u{5e73}\u{5747} {}"),
        "generate_dynamic_jacobian" => (" * generateDynamicJacobian: {}", " * generateDynamicJacobian: {}"),
        "strong_component_jacobians" => (" * Strong component Jacobians: {}", " * \u{5f3a}\u{8fde}\u{901a}\u{96ac}\u{77e9}\u{9635}: {}"),
        "symbolic_ode_jacobian" => (" * Symbolic ODE Jacobian: {}", " * \u{7b26}\u{53f7} ODE \u{96ac}\u{77e9}: {}"),
        "numeric_ode_jacobian" => (" * Numeric ODE Jacobian: {}", " * \u{6570}\u{503c} ODE \u{96ac}\u{77e9}: {}"),
        "symbolic_jacobian_size" => (" * Symbolic ODE Jacobian matrix size: {} x {}", " * \u{7b26}\u{53f7} ODE \u{96ac}\u{77e9}\u{5c3a}\u{5bf8}: {} x {}"),
        "ode_jacobian_sparsity" => (" * ODE Jacobian sparsity (IR4-4): {} nnz / {} total, density {}", " * ODE Jacobian sparsity: {} nnz / {} total, {}"),
        "c_codegen_emitted" => ("C code emitted: {}", "C code emitted: {}"),
        "warning_array_size" => ("Warning: Could not evaluate array size for '{}'", "\u{8b66}\u{544a}\u{ff1a}\u{65e0}\u{6cd5}\u{8bc4}\u{4f30}\u{6570}\u{7ec4}\u{5c3a}\u{5bf8} '{}'"),
        "warning_connect_path" => ("Warning: Could not resolve connection path: {:?} - {:?}", "\u{8b66}\u{544a}\u{ff1a}\u{65e0}\u{6cd5}\u{89e3}\u{6790}\u{8fde}\u{63a5}\u{8def}\u{5f84}"),
        "yes" => ("yes", "\u{662f}"),
        "no" => ("no", "\u{5426}"),
        _ => ("(unknown)", "\u{672a}\u{77e5}"),
    };
    match lang {
        Lang::En => en,
        Lang::Zh => zh,
    }
}

fn replace_placeholders(mut s: String, args: &[&dyn Display]) -> String {
    for a in args {
        if let Some(pos) = s.find("{}") {
            s = format!("{}{}{}", &s[..pos], a, &s[pos + 2..]);
        }
    }
    s
}

/// Format a message for the current language. Placeholders in template are "{}" in order.
pub fn msg(key: &str, args: &[&dyn Display]) -> String {
    let lang = Lang::from_env();
    let t = template(lang, key);
    replace_placeholders(t.to_string(), args)
}

/// Message with no format arguments.
pub fn msg0(key: &str) -> &'static str {
    let lang = Lang::from_env();
    template(lang, key)
}

/// Current language (for code that needs to branch on lang).
#[allow(dead_code)]
pub fn current_lang() -> Lang {
    Lang::from_env()
}
