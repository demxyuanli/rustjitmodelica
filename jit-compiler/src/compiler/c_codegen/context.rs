use super::parse_array_index;
use std::collections::{HashMap, HashSet};

pub fn array_layout_macro_name(name: &str) -> String {
    name.replace('.', "_").to_uppercase()
}

/// FUNC-7: True if the variable is an array base (appears in layout with size > 1).
pub(super) fn array_base_in_ctx(ctx: &CCodegenContext, name: &str) -> bool {
    let check = |layout: Option<&[(String, usize, usize)]>| {
        layout.map_or(false, |l| l.iter().any(|(n, _, sz)| n == name && *sz > 1))
    };
    check(ctx.state_array_layout) || check(ctx.output_array_layout) || check(ctx.param_array_layout)
}

/// FUNC-7: Return (base_c_name, start_index, size) for array base variable for C call ABI.
pub(super) fn get_array_layout_info(
    ctx: &CCodegenContext,
    name: &str,
) -> Option<(&'static str, usize, usize)> {
    let find = |layout: Option<&[(String, usize, usize)]>, base: &'static str| {
        layout.and_then(|l| {
            l.iter()
                .find(|(n, _, sz)| n == name && *sz > 1)
                .map(|(_, s, sz)| (base, *s, *sz))
        })
    };
    find(ctx.state_array_layout, "x")
        .or_else(|| find(ctx.output_array_layout, "y"))
        .or_else(|| find(ctx.param_array_layout, "p"))
}

/// Context for mapping variable names to C array access (x[], xdot[], p[], y[]).
/// Optional var_overrides: use a C expression for a variable (e.g. "local_tear" inside Newton block).
/// CG1-4: Optional array layouts enable symbolic indices (x[FOO_START + i]) in generated C.
/// When loop_context is Some(name), Variable(base_1) for any array base emits base[BASE_START + name] (for loop fusion).
#[derive(Clone)]
pub struct CCodegenContext<'a> {
    pub state_index: HashMap<String, usize>,
    pub param_index: HashMap<String, usize>,
    pub output_index: HashMap<String, usize>,
    pub var_overrides: HashMap<String, String>,
    pub state_array_layout: Option<&'a [(String, usize, usize)]>,
    pub output_array_layout: Option<&'a [(String, usize, usize)]>,
    pub param_array_layout: Option<&'a [(String, usize, usize)]>,
    pub loop_context: Option<String>,
    /// FUNC-6: When set, Call(name, args) for name in this set is emitted as name(args); (extern declared in C).
    pub external_fns: Option<HashSet<String>>,
    /// EXT-5: When set, use c_name for extern and for Call; key = modelica name, value = C name.
    pub external_c_names: Option<HashMap<String, String>>,
}

impl<'a> CCodegenContext<'a> {
    pub fn new(state_vars: &[String], param_vars: &[String], output_vars: &[String]) -> Self {
        let state_index: HashMap<String, usize> = state_vars
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i))
            .collect();
        let param_index: HashMap<String, usize> = param_vars
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i))
            .collect();
        let output_index: HashMap<String, usize> = output_vars
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i))
            .collect();
        Self {
            state_index,
            param_index,
            output_index,
            var_overrides: HashMap::new(),
            state_array_layout: None,
            output_array_layout: None,
            param_array_layout: None,
            loop_context: None,
            external_fns: None,
            external_c_names: None,
        }
    }

    pub fn with_loop_context(mut self, loop_var_c_name: &str) -> Self {
        self.loop_context = Some(loop_var_c_name.to_string());
        self
    }

    pub fn with_layouts(
        mut self,
        state: Option<&'a [(String, usize, usize)]>,
        output: Option<&'a [(String, usize, usize)]>,
        param: Option<&'a [(String, usize, usize)]>,
    ) -> Self {
        self.state_array_layout = state;
        self.output_array_layout = output;
        self.param_array_layout = param;
        self
    }

    pub fn with_override(mut self, name: &str, c_expr: String) -> Self {
        self.var_overrides.insert(name.to_string(), c_expr);
        self
    }

    pub fn with_overrides(mut self, overrides: &[(String, String)]) -> Self {
        for (k, v) in overrides {
            self.var_overrides.insert(k.clone(), v.clone());
        }
        self
    }

    pub fn var_to_c(&self, name: &str) -> Result<String, String> {
        if let Some(expr) = self.var_overrides.get(name) {
            return Ok(expr.clone());
        }
        if name == "time" {
            return Ok("t".to_string());
        }
        if let Some(ref loop_var) = self.loop_context {
            if name.starts_with("der_") {
                let rest = &name[4..];
                if let Some((base, idx)) = parse_array_index(rest) {
                    if idx == 1 {
                        if let Some(layout) = self.state_array_layout {
                            if layout.iter().any(|(n, _, sz)| *n == base && *sz >= 1) {
                                let mac = array_layout_macro_name(&base);
                                return Ok(format!("xdot[{}_START + {}]", mac, loop_var));
                            }
                        }
                    }
                }
            } else if let Some((base, idx)) = parse_array_index(name) {
                if idx == 1 {
                    if let Some(layout) = self.state_array_layout {
                        if layout.iter().any(|(n, _, sz)| *n == base && *sz >= 1) {
                            let mac = array_layout_macro_name(&base);
                            return Ok(format!("x[{}_START + {}]", mac, loop_var));
                        }
                    }
                    if let Some(layout) = self.output_array_layout {
                        if layout.iter().any(|(n, _, sz)| *n == base && *sz >= 1) {
                            let mac = array_layout_macro_name(&base);
                            return Ok(format!("y[Y_{}_START + {}]", mac, loop_var));
                        }
                    }
                    if let Some(layout) = self.param_array_layout {
                        if layout.iter().any(|(n, _, sz)| *n == base && *sz >= 1) {
                            let mac = array_layout_macro_name(&base);
                            return Ok(format!("p[P_{}_START + {}]", mac, loop_var));
                        }
                    }
                }
            }
        }
        if name.starts_with("der_") {
            let rest = &name[4..];
            if let Some((base, idx)) = parse_array_index(rest) {
                if idx >= 1 {
                    if let Some(layout) = self.state_array_layout {
                        if layout.iter().any(|(n, _, sz)| *n == base && idx <= *sz) {
                            let mac = array_layout_macro_name(&base);
                            return Ok(format!("xdot[{}_START + {}]", mac, idx - 1));
                        }
                    }
                }
            }
        }
        if let Some((base, idx)) = parse_array_index(name) {
            if idx >= 1 {
                let offset = idx - 1;
                if let Some(layout) = self.state_array_layout {
                    if layout.iter().any(|(n, _, sz)| *n == base && idx <= *sz) {
                        let mac = array_layout_macro_name(&base);
                        return Ok(format!("x[{}_START + {}]", mac, offset));
                    }
                }
                if let Some(layout) = self.output_array_layout {
                    if layout.iter().any(|(n, _, sz)| *n == base && idx <= *sz) {
                        let mac = array_layout_macro_name(&base);
                        return Ok(format!("y[Y_{}_START + {}]", mac, offset));
                    }
                }
                if let Some(layout) = self.param_array_layout {
                    if layout.iter().any(|(n, _, sz)| *n == base && idx <= *sz) {
                        let mac = array_layout_macro_name(&base);
                        return Ok(format!("p[P_{}_START + {}]", mac, offset));
                    }
                }
            }
        }
        if let Some(&i) = self.state_index.get(name) {
            return Ok(format!("x[{}]", i));
        }
        if name.starts_with("der_") {
            let base = &name[4..];
            if let Some(&i) = self.state_index.get(base) {
                return Ok(format!("xdot[{}]", i));
            }
        }
        if let Some(&i) = self.param_index.get(name) {
            return Ok(format!("p[{}]", i));
        }
        if let Some(&i) = self.output_index.get(name) {
            return Ok(format!("y[{}]", i));
        }
        Err(format!("C codegen: unknown variable '{}'", name))
    }
}
