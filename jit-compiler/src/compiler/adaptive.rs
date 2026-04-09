use super::CompilerOptions;

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct ModelStats {
    pub state_count: usize,
    pub discrete_count: usize,
    pub param_count: usize,
    pub alg_eq_count: usize,
    pub diff_eq_count: usize,
    pub differential_index: u32,
    pub algebraic_loop_count: usize,
    pub when_count: usize,
    pub crossings_count: usize,
    pub clock_partition_count: usize,
    pub total_equations: usize,
    pub total_declarations: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdaptiveProfile {
    Small,
    Medium,
    Large,
    HighIndex,
    EventHeavy,
}

impl AdaptiveProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::HighIndex => "high_index",
            Self::EventHeavy => "event_heavy",
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ParamOverride {
    pub name: String,
    pub value: String,
    pub source: &'static str,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct AdaptiveResolution {
    pub enabled: bool,
    pub profile: AdaptiveProfile,
    pub overrides: Vec<ParamOverride>,
    pub warnings: Vec<String>,
}

impl AdaptiveResolution {
    pub fn profile_name(&self) -> String {
        if self.enabled {
            self.profile.as_str().to_string()
        } else {
            "disabled".to_string()
        }
    }

    pub fn apply_env_overrides(&self) {
        if !self.enabled {
            return;
        }
        for item in &self.overrides {
            if std::env::var_os(&item.name).is_none() {
                // SAFETY: compile pipeline performs env writes on a single thread.
                unsafe { std::env::set_var(&item.name, &item.value) };
            }
        }
    }

}

pub struct AdaptiveParameterEngine;

#[derive(Clone, Copy)]
struct Thresholds {
    small_max_eq: usize,
    small_max_states: usize,
    medium_max_eq: usize,
    medium_max_states: usize,
    event_heavy_min: usize,
}

impl Thresholds {
    fn read() -> Self {
        Self {
            small_max_eq: read_usize("RUSTMODLICA_ADAPTIVE_SMALL_MAX_EQ", 50),
            small_max_states: read_usize("RUSTMODLICA_ADAPTIVE_SMALL_MAX_STATES", 20),
            medium_max_eq: read_usize("RUSTMODLICA_ADAPTIVE_MEDIUM_MAX_EQ", 500),
            medium_max_states: read_usize("RUSTMODLICA_ADAPTIVE_MEDIUM_MAX_STATES", 100),
            event_heavy_min: read_usize("RUSTMODLICA_ADAPTIVE_EVENT_HEAVY_MIN", 10),
        }
    }
}

impl AdaptiveParameterEngine {
    pub fn resolve(stats: &ModelStats, options: &CompilerOptions) -> AdaptiveResolution {
        let enabled = read_bool("RUSTMODLICA_ADAPTIVE_ENABLED", true);
        let mut out = AdaptiveResolution {
            enabled,
            profile: AdaptiveProfile::Medium,
            overrides: Vec::new(),
            warnings: Vec::new(),
        };
        if !enabled {
            return out;
        }

        let thresholds = Thresholds::read();
        out.profile = classify(stats, thresholds);

        if matches!(out.profile, AdaptiveProfile::Large) {
            push_env(
                &mut out,
                "RUSTMODLICA_JIT_STUB_PARALLEL",
                "1",
                "large model enables stub parallel compilation",
            );
            push_env(
                &mut out,
                "RUSTMODLICA_FLATTEN_EQ_PARALLEL",
                "1",
                "large model enables flatten equation parallel path",
            );
            push_env(
                &mut out,
                "RUSTMODLICA_SPARSE_MIN_SIZE",
                "4",
                "large model prefers sparse linear algebra earlier",
            );
            push_env(
                &mut out,
                "RUSTMODLICA_NEWTON_SPARSE_POLICY",
                "auto",
                "large model keeps Newton sparse policy on auto",
            );
        }

        if matches!(out.profile, AdaptiveProfile::HighIndex) {
            if should_set_index_reduction(options) {
                push_option(
                    &mut out,
                    "RUSTMODLICA_INDEX_REDUCTION_METHOD",
                    "pantelides",
                    "high-index model forces pantelides reduction",
                );
            }
            push_env(
                &mut out,
                "RUSTMODLICA_OVERDET_CHECK",
                "1",
                "high-index model enables overdetermined checks",
            );
            out.warnings.push(
                "adaptive selected high_index profile; convergence risk may increase".to_string(),
            );
        }

        if matches!(out.profile, AdaptiveProfile::EventHeavy) {
            push_env(
                &mut out,
                "RUSTMODLICA_EVENT_COUNT_DEADBAND",
                "8e-4",
                "event-heavy model widens event count deadband",
            );
            push_env(
                &mut out,
                "RUSTMODLICA_TAIL_VELOCITY_DEADBAND",
                "5e-2",
                "event-heavy model widens tail velocity deadband",
            );
            push_env(
                &mut out,
                "RUSTMODLICA_EVENT_MAX_SAME_HITS",
                "16",
                "event-heavy model raises event same-hit cap",
            );
        }

        if matches!(out.profile, AdaptiveProfile::Small) {
            push_env(
                &mut out,
                "RUSTMODLICA_JIT_STUB_PARALLEL",
                "0",
                "small model avoids stub parallel overhead",
            );
            push_env(
                &mut out,
                "RUSTMODLICA_FLATTEN_EQ_PARALLEL",
                "0",
                "small model avoids flatten parallel overhead",
            );
            push_env(
                &mut out,
                "RUSTMODLICA_SUNDIALS_LINSOL",
                "dense",
                "small model prefers dense linear solver",
            );
        }

        out
    }
}

fn classify(stats: &ModelStats, th: Thresholds) -> AdaptiveProfile {
    let total_events = stats.when_count + stats.crossings_count;
    if stats.differential_index > 1 {
        return AdaptiveProfile::HighIndex;
    }
    if total_events > th.event_heavy_min {
        return AdaptiveProfile::EventHeavy;
    }
    if stats.total_equations <= th.small_max_eq && stats.state_count <= th.small_max_states {
        return AdaptiveProfile::Small;
    }
    if stats.total_equations > th.medium_max_eq || stats.state_count > th.medium_max_states {
        return AdaptiveProfile::Large;
    }
    AdaptiveProfile::Medium
}

fn should_set_index_reduction(options: &CompilerOptions) -> bool {
    if std::env::var_os("RUSTMODLICA_INDEX_REDUCTION_METHOD").is_some() {
        return false;
    }
    options.index_reduction_method == CompilerOptions::default().index_reduction_method
}

fn push_env(out: &mut AdaptiveResolution, name: &str, value: &str, reason: &str) {
    if std::env::var_os(name).is_some() {
        return;
    }
    out.overrides.push(ParamOverride {
        name: name.to_string(),
        value: value.to_string(),
        source: "adaptive",
        reason: reason.to_string(),
    });
}

fn push_option(out: &mut AdaptiveResolution, name: &str, value: &str, reason: &str) {
    out.overrides.push(ParamOverride {
        name: name.to_string(),
        value: value.to_string(),
        source: "adaptive",
        reason: reason.to_string(),
    });
}

fn read_bool(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => default,
    }
}

fn read_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(default)
}
