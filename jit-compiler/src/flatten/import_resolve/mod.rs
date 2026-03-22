mod context;
mod domains_electrical;
mod domains_magnetic;
mod domains_misc;

use std::sync::Arc;

use crate::ast::Model;
use crate::loader::LoadError;

use super::Flattener;

use self::context::ResolveContext;

impl Flattener {
    pub(crate) fn resolve_import_prefix(
        model: &Model,
        name: &str,
        current_qualified: &str,
    ) -> String {
        let name = name.trim().trim_start_matches('.');
        let cq_buf = if current_qualified.contains('/') {
            current_qualified.replace('/', ".")
        } else {
            current_qualified.to_string()
        };
        let cq = cq_buf.as_str();
        if let Some(resolved) = domains_misc::resolve_global_shortcuts(model, name) {
            return resolved;
        }
        if let Some(resolved) = domains_misc::resolve_context_prechecks(name, cq) {
            return resolved;
        }
        if !name.contains('.') && model.inner_class_index.contains_key(name) {
            return name.to_string();
        }

        let ctx = ResolveContext::from_current_qualified(cq);

        if let Some(resolved) = domains_magnetic::resolve_magnetic_domain(name, cq, &ctx) {
            return resolved;
        }
        if let Some(resolved) = domains_electrical::resolve_electrical_domain(name, cq, &ctx) {
            return resolved;
        }
        if let Some(resolved) =
            domains_misc::resolve_mechanics_clocked_thermal_domain(name, cq, &ctx)
        {
            return resolved;
        }
        if let Some(resolved) = domains_misc::resolve_fluid_domain(name, cq, &ctx) {
            return resolved;
        }
        if let Some(resolved) = domains_misc::resolve_global_namespace_aliases(name) {
            return resolved;
        }
        if let Some(resolved) = domains_misc::resolve_blocks_utilities_domain(name, &ctx) {
            return resolved;
        }
        name.to_string()
    }

    pub(crate) fn qualify_in_scope(current_qualified: &str, name: &str) -> String {
        if name.contains('.') || name.contains('/') {
            return name.to_string();
        }
        let q = if current_qualified.contains('/') {
            current_qualified.replace('/', ".")
        } else {
            current_qualified.to_string()
        };
        if let Some((parent, _)) = q.rsplit_once('.') {
            return format!("{}.{}", parent, name);
        }
        name.to_string()
    }

    pub(super) fn qualify_in_current_class(current_qualified: &str, name: &str) -> String {
        if current_qualified.is_empty() || name.contains('.') || name.contains('/') {
            return name.to_string();
        }
        format!("{}.{}", current_qualified, name)
    }

    pub(super) fn resolve_import_scoped_type(
        model: &Model,
        type_name: &str,
        current_qualified: &str,
        msl_import_context: &str,
    ) -> String {
        let type_name = type_name.trim();
        // Import and short-type resolution use the lexical scope of the class being flattened
        // (`current_qualified`). `msl_import_context` is the compile root for the rare case where
        // expansion starts with no qualified current class (see decl_expand). MSL path aliases
        // (FluidHeatFlow, FundamentalWave.Utilities, Material, etc.) are handled in `ModelLoader`.
        let import_scope = if !current_qualified.is_empty() {
            current_qualified
        } else if !msl_import_context.is_empty() {
            msl_import_context
        } else {
            current_qualified
        };
        Self::resolve_import_prefix(model, type_name, import_scope)
    }

    pub(super) fn normalize_decl_type_name(
        mut resolved_type: String,
        pre_inner_alias: &str,
    ) -> String {
        resolved_type = resolved_type.trim().to_string();
        if resolved_type.eq_ignore_ascii_case("real") {
            resolved_type = "Real".to_string();
        }
        let is_medium_prefix = |s: &str| -> bool {
            s.split_once('.')
                .map(|(p, _)| {
                    p == "Medium"
                        || p.strip_prefix("Medium_")
                            .and_then(|n| n.parse::<u32>().ok())
                            .is_some()
                        || p.strip_prefix("Medium")
                            .and_then(|n| n.parse::<u32>().ok())
                            .is_some()
                })
                .unwrap_or(false)
        };
        if is_medium_prefix(&resolved_type) || is_medium_prefix(pre_inner_alias) {
            return "Real".to_string();
        }
        if matches!(
            resolved_type.as_str(),
            "RealInput"
                | "RealOutput"
                | "BooleanInput"
                | "BooleanOutput"
                | "IntegerInput"
                | "IntegerOutput"
        ) || resolved_type.ends_with(".RealInput")
            || resolved_type.ends_with(".RealOutput")
        {
            return "Real".to_string();
        }
        if resolved_type.ends_with(".BooleanInput") || resolved_type.ends_with(".BooleanOutput") {
            return "Boolean".to_string();
        }
        if resolved_type.ends_with(".DigitalInput") || resolved_type.ends_with(".DigitalOutput") {
            return "Boolean".to_string();
        }
        if resolved_type.ends_with(".IntegerInput") || resolved_type.ends_with(".IntegerOutput") {
            return "Integer".to_string();
        }
        if resolved_type.starts_with("Modelica.Fluid.Types.")
            || resolved_type.ends_with(".Types.AxisLabel")
            || resolved_type.ends_with(".Types.Axis")
        {
            return "Real".to_string();
        }
        if resolved_type.contains("Modelica.Fluid")
            && (resolved_type.contains("flowCharacteristic")
                || resolved_type.contains("efficiencyCharacteristic")
                || resolved_type.contains("pressureLoss")
                || resolved_type.contains("PressureLoss"))
        {
            return "Real".to_string();
        }
        if resolved_type == "flowCharacteristic" || resolved_type == "efficiencyCharacteristic" {
            return "Real".to_string();
        }
        if resolved_type.starts_with("pressureLoss") {
            return "Real".to_string();
        }
        if resolved_type.eq_ignore_ascii_case("distribution") {
            return "Real".to_string();
        }
        if resolved_type == "realFFT" || resolved_type == "realFFTsamplePoints" {
            return "Real".to_string();
        }
        if resolved_type.eq_ignore_ascii_case("semilinear")
            || resolved_type
                .to_ascii_lowercase()
                .starts_with("semilinear.")
        {
            return "Real".to_string();
        }
        if let Some(seg) = resolved_type.rsplit('.').next() {
            if seg == "flowCharacteristic" || seg == "efficiencyCharacteristic" {
                return "Real".to_string();
            }
            if seg.eq_ignore_ascii_case("distribution") {
                return "Real".to_string();
            }
            if seg == "realFFT" || seg == "realFFTsamplePoints" {
                return "Real".to_string();
            }
            if seg.eq_ignore_ascii_case("semilinear") {
                return "Real".to_string();
            }
        }
        resolved_type
    }

    pub(super) fn build_load_candidates(
        resolved_type: &str,
        current_qualified: &str,
    ) -> Vec<String> {
        let mut load_candidates = vec![resolved_type.to_string()];
        if resolved_type.contains('/') {
            return load_candidates;
        }
        let (first_component, rest_suffix) = if let Some(dot_pos) = resolved_type.find('.') {
            (&resolved_type[..dot_pos], &resolved_type[dot_pos..])
        } else {
            (resolved_type, "")
        };
        if rest_suffix.is_empty() {
            let same_class = Self::qualify_in_current_class(current_qualified, resolved_type);
            if same_class != resolved_type {
                load_candidates.push(same_class);
            }
        }
        let mut scope = current_qualified.to_string();
        while let Some((parent, _)) = scope.rsplit_once('.') {
            let candidate = format!("{}.{}{}", parent, first_component, rest_suffix);
            if !load_candidates.contains(&candidate) {
                load_candidates.push(candidate);
            }
            scope = parent.to_string();
        }
        load_candidates
    }

    pub(super) fn try_load_sub_model(
        &mut self,
        owner_model: &Model,
        resolved_type: &str,
        current_qualified: &str,
        load_candidates: &[String],
    ) -> (Option<(String, Arc<Model>)>, Option<LoadError>) {
        let mut loaded_type: Option<(String, Arc<Model>)> = None;
        if !resolved_type.contains('.') && !resolved_type.contains('/') {
            if let Some(inner) = owner_model.find_inner_class(resolved_type) {
                let mut inner_model = inner.clone();
                for (a, q) in &owner_model.imports {
                    if !inner_model
                        .imports
                        .iter()
                        .any(|(aa, qq)| aa == a && qq == q)
                    {
                        inner_model.imports.push((a.clone(), q.clone()));
                    }
                }
                loaded_type = Some((
                    Self::qualify_in_current_class(current_qualified, resolved_type),
                    Arc::new(inner_model),
                ));
            }
        }
        let mut last_err: Option<LoadError> = None;
        if loaded_type.is_none() {
            for candidate in load_candidates {
                match self.loader.load_model_silent(candidate, true) {
                    Ok(m) => {
                        loaded_type = Some((candidate.clone(), m));
                        break;
                    }
                    Err(e) => last_err = Some(e),
                }
            }
        }
        (loaded_type, last_err)
    }
}
