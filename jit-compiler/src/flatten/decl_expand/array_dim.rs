use crate::ast::{Expression, Model};
use crate::flatten::eval_const_expr_with_param_exprs;
use crate::flatten::substitute::SubstituteCache;
use crate::flatten::Flattener;
use crate::flatten::ValidationMode;
use std::collections::{HashMap, HashSet};

use super::env::perf_trace_enabled;

#[derive(Debug, Default)]
pub(super) struct ArrayDimensionOptimizer {
    pub(super) computed_dims: HashMap<String, usize>,
    pub(super) uncalculable: HashSet<String>,
}

impl ArrayDimensionOptimizer {
    fn compute_expr_complexity(expr: &Expression) -> u32 {
        use crate::ast::Expression as E;
        match expr {
            E::Number(_) => 0,
            E::Variable(_) => 1,
            E::StringLiteral(_) => 0,
            E::BinaryOp(l, _, r) => 1 + Self::compute_expr_complexity(l) + Self::compute_expr_complexity(r),
            E::Call(_, args) => 3 + args.iter().map(Self::compute_expr_complexity).sum::<u32>(),
            E::If(c, t, f) => {
                2 + Self::compute_expr_complexity(c)
                    + Self::compute_expr_complexity(t)
                    + Self::compute_expr_complexity(f)
            }
            E::Der(inner) => 1 + Self::compute_expr_complexity(inner),
            E::ArrayAccess(a, i) => 2 + Self::compute_expr_complexity(a) + Self::compute_expr_complexity(i),
            E::ArrayLiteral(items) => 2 + items.iter().map(Self::compute_expr_complexity).sum::<u32>(),
            _ => 10,
        }
    }

    pub(super) fn optimize_array_dims(
        &mut self,
        flattener: &mut Flattener,
        model: &Model,
        context: &HashMap<String, Expression>,
        local_array_sizes: &mut HashMap<String, usize>,
    ) -> usize {
        const COMPLEXITY_THRESHOLD: u32 = 5;
        let max_fast_passes = match flattener.validation_mode {
            ValidationMode::SuperFast => return 0,
            ValidationMode::QuickStructure => 3usize,
            ValidationMode::Full => 16usize,
        };
        let perf = perf_trace_enabled();
        self.computed_dims.clear();
        self.uncalculable.clear();

        let mut pass = 0usize;
        while pass < max_fast_passes {
            let mut sub_cache = SubstituteCache::new(4096);
            let mut dim_changed = false;
            for decl in &model.declarations {
                if local_array_sizes.contains_key(&decl.name) {
                    continue;
                }
                if self.uncalculable.contains(&decl.name) {
                    continue;
                }
                let Some(size_expr) = decl.array_size.as_ref() else {
                    continue;
                };
                if let Some(&n) = self.computed_dims.get(&decl.name) {
                    local_array_sizes.insert(decl.name.clone(), n);
                    continue;
                }
                if let Expression::Number(n) = size_expr {
                    let sz = *n as usize;
                    if sz > 0 {
                        local_array_sizes.insert(decl.name.clone(), sz);
                        self.computed_dims.insert(decl.name.clone(), sz);
                        dim_changed = true;
                    }
                    continue;
                }
                let complexity = Self::compute_expr_complexity(size_expr);
                if complexity > COMPLEXITY_THRESHOLD {
                    self.uncalculable.insert(decl.name.clone());
                    if perf {
                        eprintln!(
                            "[perf] array_dim_skip_complex name={} complexity={}",
                            decl.name, complexity
                        );
                    }
                    continue;
                }
                if let Some(ref cond_expr) = decl.condition {
                    let cond_sub = flattener.substitute_cached_cow(cond_expr, context, &mut sub_cache);
                    if let Some(v) =
                        eval_const_expr_with_param_exprs(cond_sub.as_ref(), context, local_array_sizes)
                    {
                        if v == 0.0 {
                            continue;
                        }
                    }
                }
                let sub_expr = flattener.substitute_cached_cow(size_expr, context, &mut sub_cache);
                if let Some(val) =
                    eval_const_expr_with_param_exprs(sub_expr.as_ref(), context, local_array_sizes)
                {
                    let n = val as usize;
                    if n > 0 {
                        local_array_sizes.insert(decl.name.clone(), n);
                        self.computed_dims.insert(decl.name.clone(), n);
                        dim_changed = true;
                    }
                }
            }
            if !dim_changed {
                break;
            }
            pass += 1;
        }
        pass
    }
}
