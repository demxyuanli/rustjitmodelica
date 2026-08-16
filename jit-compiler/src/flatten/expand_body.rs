use crate::ast::{AlgorithmStatement, Equation, Expression, Model};
use rayon::prelude::*;
use std::collections::HashMap;

use super::{ExpandTarget, FlattenedModel, Flattener};

impl Flattener {
    pub(super) fn expand_equations(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let instances = &flat.instances;
        let parallel_enabled = self.flatten_eq_parallel_enabled()
            && prefix.is_empty()
            && model.equations.len() >= self.flatten_eq_parallel_min_items();
        if parallel_enabled {
            crate::query_db::perf_record_add("flatten_parallel_poc_enabled", 1);
            let library_paths = self.loader.library_paths.clone();
            let quiet = self.loader.quiet;
            let array_sizes = flat.array_sizes.clone();
            let instances_cloned = instances.clone();
            let base_context_stack = context_stack.clone();
            let micro_batch = self.flatten_eq_parallel_micro_batch_size();
            let micro_budget = self.flatten_eq_parallel_micro_batch_budget();
            let bucket_count = rayon::current_num_threads().max(1);
            let buckets = Self::balanced_buckets_by_weight(
                &model.equations,
                Self::equation_complexity,
                bucket_count,
            );
            let mut partials: Vec<(usize, Vec<(usize, Equation)>, Vec<(usize, AlgorithmStatement)>, Vec<(usize, (String, String))>, Vec<(usize, (Expression, (String, String)))>)> = buckets
                .into_par_iter()
                .enumerate()
                .map(|(bucket_idx, bucket)| {
                    let mut local = self.clone_for_parallel_expand();
                    local.loader.set_quiet(quiet);
                    for p in &library_paths {
                        local.loader.add_path(p.clone());
                    }
                    let mut eq_out_idx = Vec::new();
                    let mut alg_out_idx = Vec::new();
                    let mut conn_out_idx = Vec::new();
                    let mut cconn_out_idx = Vec::new();
                    let batches = Self::adaptive_batches_by_budget(
                        &bucket,
                        Self::equation_complexity,
                        micro_batch,
                        micro_budget,
                    );
                    for batch in batches {
                        let mut eq_out = Vec::new();
                        let mut alg_out = Vec::new();
                        let mut conn_out = Vec::new();
                        let mut cconn_out = Vec::new();
                        let mut target = ExpandTarget {
                            equations: &mut eq_out,
                            algorithms: &mut alg_out,
                            connections: &mut conn_out,
                            conditional_connections: &mut cconn_out,
                            array_sizes: &array_sizes,
                        };
                        let mut local_context_stack = base_context_stack.clone();
                        let single: Vec<Equation> = batch.iter().map(|(_, eq)| eq.clone()).collect();
                        local.expand_equation_list(
                            &single,
                            prefix,
                            &mut target,
                            &mut local_context_stack,
                            &instances_cloned,
                            None,
                        );
                        let mut fallback_idx = batch[0].0;
                        for (k, v) in eq_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            eq_out_idx.push((fallback_idx, v));
                        }
                        fallback_idx = batch[0].0;
                        for (k, v) in alg_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            alg_out_idx.push((fallback_idx, v));
                        }
                        fallback_idx = batch[0].0;
                        for (k, v) in conn_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            conn_out_idx.push((fallback_idx, v));
                        }
                        fallback_idx = batch[0].0;
                        for (k, v) in cconn_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            cconn_out_idx.push((fallback_idx, v));
                        }
                    }
                    (bucket_idx, eq_out_idx, alg_out_idx, conn_out_idx, cconn_out_idx)
                })
                .collect();
            partials.sort_by_key(|x| x.0);
            let mut eq_all = Vec::new();
            let mut alg_all = Vec::new();
            let mut conn_all = Vec::new();
            let mut cconn_all = Vec::new();
            for (_, eqs, algs, conns, cconns) in partials {
                eq_all.extend(eqs);
                alg_all.extend(algs);
                conn_all.extend(conns);
                cconn_all.extend(cconns);
            }
            eq_all.sort_by_key(|(idx, _)| *idx);
            alg_all.sort_by_key(|(idx, _)| *idx);
            conn_all.sort_by_key(|(idx, _)| *idx);
            cconn_all.sort_by_key(|(idx, _)| *idx);
            flat.equations.extend(eq_all.into_iter().map(|(_, v)| v));
            flat.algorithms.extend(alg_all.into_iter().map(|(_, v)| v));
            flat.connections.extend(conn_all.into_iter().map(|(_, v)| v));
            flat.conditional_connections
                .extend(cconn_all.into_iter().map(|(_, v)| v));
        } else {
            let mut target = ExpandTarget {
                equations: &mut flat.equations,
                algorithms: &mut flat.algorithms,
                connections: &mut flat.connections,
                conditional_connections: &mut flat.conditional_connections,
                array_sizes: &flat.array_sizes,
            };
            self.expand_equation_list(
                &model.equations,
                prefix,
                &mut target,
                &mut context_stack,
                instances,
                None,
            );
        }
    }

    pub(super) fn expand_initial_equations(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let instances = &flat.instances;
        let mut target = ExpandTarget {
            equations: &mut flat.initial_equations,
            algorithms: &mut flat.initial_algorithms,
            connections: &mut flat.connections,
            conditional_connections: &mut flat.conditional_connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_equation_list(
            &model.initial_equations,
            prefix,
            &mut target,
            &mut context_stack,
            instances,
            None,
        );
    }

    pub(super) fn expand_algorithms(&mut self, model: &Model, prefix: &str, flat: &mut FlattenedModel) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let parallel_enabled = self.flatten_eq_parallel_enabled()
            && prefix.is_empty()
            && model.algorithms.len() >= self.flatten_eq_parallel_min_items();
        if parallel_enabled {
            crate::query_db::perf_record_add("flatten_parallel_poc_enabled", 1);
            let library_paths = self.loader.library_paths.clone();
            let quiet = self.loader.quiet;
            let array_sizes = flat.array_sizes.clone();
            let base_context_stack = context_stack.clone();
            let micro_batch = self.flatten_eq_parallel_micro_batch_size();
            let micro_budget = self.flatten_eq_parallel_micro_batch_budget();
            let bucket_count = rayon::current_num_threads().max(1);
            let buckets = Self::balanced_buckets_by_weight(
                &model.algorithms,
                Self::algorithm_complexity,
                bucket_count,
            );
            let mut partials: Vec<(usize, Vec<(usize, Equation)>, Vec<(usize, AlgorithmStatement)>)> = buckets
                .into_par_iter()
                .enumerate()
                .map(|(bucket_idx, bucket)| {
                    let mut local = self.clone_for_parallel_expand();
                    local.loader.set_quiet(quiet);
                    for p in &library_paths {
                        local.loader.add_path(p.clone());
                    }
                    let mut eq_out_idx = Vec::new();
                    let mut alg_out_idx = Vec::new();
                    let batches = Self::adaptive_batches_by_budget(
                        &bucket,
                        Self::algorithm_complexity,
                        micro_batch,
                        micro_budget,
                    );
                    for batch in batches {
                        let mut eq_out = Vec::new();
                        let mut alg_out = Vec::new();
                        let mut conn_out = Vec::new();
                        let mut cconn_out = Vec::new();
                        let mut target = ExpandTarget {
                            equations: &mut eq_out,
                            algorithms: &mut alg_out,
                            connections: &mut conn_out,
                            conditional_connections: &mut cconn_out,
                            array_sizes: &array_sizes,
                        };
                        let mut local_context_stack = base_context_stack.clone();
                        let single: Vec<AlgorithmStatement> =
                            batch.iter().map(|(_, stmt)| stmt.clone()).collect();
                        local.expand_algorithm_list(
                            &single,
                            prefix,
                            &mut target,
                            &mut local_context_stack,
                        );
                        let mut fallback_idx = batch[0].0;
                        for (k, v) in eq_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            eq_out_idx.push((fallback_idx, v));
                        }
                        fallback_idx = batch[0].0;
                        for (k, v) in alg_out.into_iter().enumerate() {
                            if k < batch.len() {
                                fallback_idx = batch[k].0;
                            }
                            alg_out_idx.push((fallback_idx, v));
                        }
                    }
                    (bucket_idx, eq_out_idx, alg_out_idx)
                })
                .collect();
            partials.sort_by_key(|x| x.0);
            let mut eq_all = Vec::new();
            let mut alg_all = Vec::new();
            for (_, eqs, algs) in partials {
                eq_all.extend(eqs);
                alg_all.extend(algs);
            }
            eq_all.sort_by_key(|(idx, _)| *idx);
            alg_all.sort_by_key(|(idx, _)| *idx);
            flat.equations.extend(eq_all.into_iter().map(|(_, v)| v));
            flat.algorithms.extend(alg_all.into_iter().map(|(_, v)| v));
        } else {
            let mut target = ExpandTarget {
                equations: &mut flat.equations,
                algorithms: &mut flat.algorithms,
                connections: &mut flat.connections,
                conditional_connections: &mut flat.conditional_connections,
                array_sizes: &flat.array_sizes,
            };
            self.expand_algorithm_list(&model.algorithms, prefix, &mut target, &mut context_stack);
        }
    }

    pub(super) fn expand_initial_algorithms(
        &mut self,
        model: &Model,
        prefix: &str,
        flat: &mut FlattenedModel,
    ) {
        let mut context: HashMap<String, Expression> = HashMap::new();
        for decl in &model.declarations {
            if decl.is_parameter {
                if let Some(val) = &decl.start_value {
                    context.insert(decl.name.clone(), val.clone());
                }
            }
        }
        let mut context_stack = vec![context];
        let mut target = ExpandTarget {
            equations: &mut flat.initial_equations,
            algorithms: &mut flat.initial_algorithms,
            connections: &mut flat.connections,
            conditional_connections: &mut flat.conditional_connections,
            array_sizes: &flat.array_sizes,
        };
        self.expand_algorithm_list(
            &model.initial_algorithms,
            prefix,
            &mut target,
            &mut context_stack,
        );
    }

}
