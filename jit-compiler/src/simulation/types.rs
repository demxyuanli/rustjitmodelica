use std::collections::HashMap;
use crate::ast::Expression;
use crate::compiler::ClockPartitionScheduleEntry;
use crate::jit::CalcDerivsFunc;
use serde::ser::SerializeMap;

/// Serializable simulation time series for IDE/Plotly (time + series per variable).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimulationResult {
    pub time: Vec<f64>,
    #[serde(serialize_with = "serialize_series_sorted")]
    pub series: HashMap<String, Vec<f64>>,
}

fn serialize_series_sorted<S>(
    series: &HashMap<String, Vec<f64>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut entries: Vec<(&String, &Vec<f64>)> = series.iter().collect();
    entries.sort_unstable_by(|a, b| a.0.cmp(b.0));
    let mut map = serializer.serialize_map(Some(entries.len()))?;
    for (k, v) in entries {
        map.serialize_entry(k, v)?;
    }
    map.end()
}

/// Row collector for run_simulation when collecting in-memory (time, states, discrete, outputs).
pub type ResultCollector = Vec<(f64, Vec<f64>, Vec<f64>, Vec<f64>)>;

#[derive(Debug, Clone, PartialEq)]
pub enum QueuedEventKind {
    WhenEdge(usize),
    ZeroCrossing(usize),
    ClockPartition(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueuedEvent {
    pub time: f64,
    pub kind: QueuedEventKind,
}

#[derive(Debug, Default, Clone)]
pub struct EventQueue {
    items: Vec<QueuedEvent>,
}

impl EventQueue {
    pub fn push_unique(&mut self, event: QueuedEvent) {
        if self.items.iter().any(|existing| existing == &event) {
            return;
        }
        self.items.push(event);
    }

    pub fn drain_sorted(&mut self) -> Vec<QueuedEvent> {
        self.items.sort_by(|a, b| a.time.total_cmp(&b.time));
        std::mem::take(&mut self.items)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SundialsRuntimeConfig {
    pub max_order: Option<i32>,
    pub max_nonlin_iters: Option<i32>,
    pub max_step: Option<f64>,
}

impl SundialsRuntimeConfig {
    #[allow(dead_code)]
    pub fn from_env() -> Self {
        fn env_i32(name: &str) -> Option<i32> {
            std::env::var(name)
                .ok()
                .and_then(|v| v.trim().parse::<i32>().ok())
                .filter(|v| *v > 0)
        }
        fn env_f64(name: &str) -> Option<f64> {
            std::env::var(name)
                .ok()
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
        }
        Self {
            max_order: env_i32("RUSTMODLICA_SUNDIALS_MAX_ORDER"),
            max_nonlin_iters: env_i32("RUSTMODLICA_SUNDIALS_MAX_NL_ITERS"),
            max_step: env_f64("RUSTMODLICA_SUNDIALS_MAX_STEP"),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EventDebounceConfig {
    pub base_deadband: f64,
    pub count_deadband: f64,
    pub max_same_event_hits: u32,
}

impl EventDebounceConfig {
    #[allow(dead_code)]
    pub fn adaptive_from_dt(dt: f64) -> Self {
        fn env_f64(name: &str) -> Option<f64> {
            std::env::var(name)
                .ok()
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
        }
        fn env_u32(name: &str) -> Option<u32> {
            std::env::var(name)
                .ok()
                .and_then(|v| v.trim().parse::<u32>().ok())
                .filter(|v| *v > 0)
        }
        let base_scale = env_f64("RUSTMODLICA_EVENT_DEADBAND_SCALE").unwrap_or(1.0);
        let count_scale = env_f64("RUSTMODLICA_EVENT_COUNT_DEADBAND_SCALE").unwrap_or(1.0);
        let clamped_dt = dt.abs().clamp(1e-9, 1.0);
        Self {
            base_deadband: env_f64("RUSTMODLICA_EVENT_DEADBAND")
                .unwrap_or_else(|| ((clamped_dt * 0.25) * base_scale).max(1e-7)),
            count_deadband: env_f64("RUSTMODLICA_EVENT_COUNT_DEADBAND")
                .unwrap_or_else(|| ((clamped_dt * 0.5) * count_scale).max(1e-6)),
            max_same_event_hits: env_u32("RUSTMODLICA_EVENT_MAX_SAME_HITS").unwrap_or(8),
        }
    }
}

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
    differential_index: u32,
    ida_component_id: &[f64],
    solver: &str,
    output_interval: f64,
    clock_partition_schedule: &[ClockPartitionScheduleEntry],
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
        differential_index,
        ida_component_id,
        solver,
        output_interval,
        None,
        clock_partition_schedule,
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
