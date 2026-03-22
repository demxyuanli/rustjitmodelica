use std::collections::HashMap;
use crate::ast::Expression;
use crate::jit::CalcDerivsFunc;

/// Serializable simulation time series for IDE/Plotly (time + series per variable).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimulationResult {
    pub time: Vec<f64>,
    pub series: HashMap<String, Vec<f64>>,
}

/// Row collector for run_simulation when collecting in-memory (time, states, discrete, outputs).
pub type ResultCollector = Vec<(f64, Vec<f64>, Vec<f64>, Vec<f64>)>;

pub fn run_simulation_collect(
    calc_derivs: CalcDerivsFunc,
    when_count: usize,
    crossings_count: usize,
    states: Vec<f64>,
    discrete_vals: Vec<f64>,
    params: Vec<f64>,
    state_vars: &[String],
    discrete_vars: &[String],
    output_vars: &[String],
    output_start_vals: &[f64],
    state_var_index: &HashMap<String, usize>,
    t_end: f64,
    dt: f64,
    numeric_ode_jacobian: bool,
    symbolic_ode_jacobian: Option<&Vec<Vec<Expression>>>,
    newton_tearing_var_names: &[String],
    atol: f64,
    rtol: f64,
    solver: &str,
    output_interval: f64,
) -> Result<SimulationResult, String> {
    let estimated_rows = if output_interval > 0.0 {
        (t_end / output_interval) as usize + 2
    } else {
        (t_end / dt) as usize + 2
    };
    let mut collector = ResultCollector::with_capacity(estimated_rows);
    super::run_simulation(
        calc_derivs,
        when_count,
        crossings_count,
        states,
        discrete_vals,
        params,
        state_vars,
        discrete_vars,
        output_vars,
        output_start_vals,
        state_var_index,
        t_end,
        dt,
        numeric_ode_jacobian,
        symbolic_ode_jacobian,
        newton_tearing_var_names,
        atol,
        rtol,
        solver,
        output_interval,
        None,
        Some(&mut collector),
    )?;
    let mut time = Vec::with_capacity(collector.len());
    let mut series: HashMap<String, Vec<f64>> = HashMap::new();
    series.insert("time".to_string(), Vec::with_capacity(collector.len()));
    for name in state_vars {
        series.insert(name.clone(), Vec::with_capacity(collector.len()));
    }
    for name in discrete_vars {
        series.insert(name.clone(), Vec::with_capacity(collector.len()));
    }
    for name in output_vars {
        series.insert(name.clone(), Vec::with_capacity(collector.len()));
    }
    for (t, st, disc, out) in collector {
        time.push(t);
        series.get_mut("time").unwrap().push(t);
        for (i, name) in state_vars.iter().enumerate() {
            let v = st.get(i).copied().unwrap_or(0.0);
            series.get_mut(name).unwrap().push(v);
        }
        for (i, name) in discrete_vars.iter().enumerate() {
            let v = disc.get(i).copied().unwrap_or(0.0);
            series.get_mut(name).unwrap().push(v);
        }
        for (i, name) in output_vars.iter().enumerate() {
            let v = out.get(i).copied().unwrap_or(0.0);
            series.get_mut(name).unwrap().push(v);
        }
    }
    Ok(SimulationResult { time, series })
}
