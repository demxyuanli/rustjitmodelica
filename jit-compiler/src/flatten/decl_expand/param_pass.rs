use crate::ast::{Expression, Model};
use crate::flatten::eval_const_expr_with_param_exprs;
use crate::flatten::substitute::SubstituteCache;
use crate::flatten::Flattener;
use crate::flatten::ValidationMode;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub(super) struct ParamPassOptimizer {
    pub(super) param_deps: HashMap<String, HashSet<String>>,
    pub(super) stable_params: HashSet<String>,
    last_change_pass: HashMap<String, usize>,
    dependents_index: HashMap<String, Vec<String>>,
}

impl ParamPassOptimizer {
    fn rebuild_dependency_graph(&mut self, model: &Model) {
        self.param_deps.clear();
        self.dependents_index.clear();
        for decl in &model.declarations {
            if !decl.is_parameter {
                continue;
            }
            let mut deps: HashSet<String> = HashSet::new();
            if let Some(val) = decl.start_value.as_ref() {
                Self::collect_var_refs(val, &mut deps);
            }
            deps.remove(&decl.name);
            self.param_deps.insert(decl.name.clone(), deps);
        }
        for (p, deps) in &self.param_deps {
            for d in deps {
                self.dependents_index
                    .entry(d.clone())
                    .or_default()
                    .push(p.clone());
            }
        }
    }

    fn collect_var_refs(expr: &Expression, out: &mut HashSet<String>) {
        use crate::ast::Expression as E;
        match expr {
            E::Variable(id) => {
                out.insert(crate::string_intern::resolve_id(*id));
            }
            E::BinaryOp(l, _, r) => {
                Self::collect_var_refs(l, out);
                Self::collect_var_refs(r, out);
            }
            E::Call(_, args) => {
                for a in args {
                    Self::collect_var_refs(a, out);
                }
            }
            E::Der(inner) => Self::collect_var_refs(inner, out),
            E::If(c, t, f) => {
                Self::collect_var_refs(c, out);
                Self::collect_var_refs(t, out);
                Self::collect_var_refs(f, out);
            }
            E::ArrayAccess(arr, idx) => {
                Self::collect_var_refs(arr, out);
                Self::collect_var_refs(idx, out);
            }
            E::ArrayLiteral(items) => {
                for it in items {
                    Self::collect_var_refs(it, out);
                }
            }
            _ => {}
        }
    }

    pub(super) fn optimize_param_passes(
        &mut self,
        flattener: &mut Flattener,
        model: &Model,
        context: &mut HashMap<String, Expression>,
        local_array_sizes: &HashMap<String, usize>,
    ) -> usize {
        let (max_fast_passes, stability_passes) = match flattener.validation_mode {
            ValidationMode::SuperFast => return 0,
            ValidationMode::QuickStructure => (5usize, 1usize),
            ValidationMode::Full => (32usize, 2usize),
        };

        self.stable_params.clear();
        self.last_change_pass.clear();
        self.rebuild_dependency_graph(model);

        let mut stalled = 0usize;
        let mut pass = 0usize;
        let mut sub_cache = SubstituteCache::new(4096);
        while pass < max_fast_passes {
            sub_cache.clear();
            let mut changed_params: Vec<String> = Vec::new();
            for decl in &model.declarations {
                if !decl.is_parameter {
                    continue;
                }
                if self.stable_params.contains(&decl.name) {
                    continue;
                }
                let Some(val) = decl.start_value.as_ref() else {
                    self.stable_params.insert(decl.name.clone());
                    continue;
                };
                if self
                    .param_deps
                    .get(&decl.name)
                    .map(|s| s.is_empty())
                    .unwrap_or(false)
                {
                    self.stable_params.insert(decl.name.clone());
                    continue;
                }
                let sub = flattener.substitute_cached_cow(val, context, &mut sub_cache);
                if let Some(n) = eval_const_expr_with_param_exprs(sub.as_ref(), context, local_array_sizes) {
                    let update = match context.get(&decl.name) {
                        None => true,
                        Some(Expression::Number(p)) => (n - p).abs() > 1e-12,
                        Some(_) => true,
                    };
                    if update {
                        context.insert(decl.name.clone(), Expression::Number(n));
                        changed_params.push(decl.name.clone());
                        self.last_change_pass.insert(decl.name.clone(), pass);
                    }
                }
            }

            if changed_params.is_empty() {
                stalled += 1;
                if stalled >= stability_passes {
                    break;
                }
            } else {
                stalled = 0;
                for changed in &changed_params {
                    if let Some(deps) = self.dependents_index.get(changed) {
                        for dep in deps {
                            self.stable_params.remove(dep);
                        }
                    }
                }
            }

            for p in self.param_deps.keys() {
                if self.stable_params.contains(p) {
                    continue;
                }
                let last = *self.last_change_pass.get(p).unwrap_or(&0);
                if pass.saturating_sub(last) > 5 {
                    self.stable_params.insert(p.clone());
                }
            }
            pass += 1;
        }
        pass
    }
}
