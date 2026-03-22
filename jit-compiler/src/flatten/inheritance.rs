use super::Flattener;
use crate::ast::{ExtendsClause, Model};
use crate::flatten::redeclare::apply_redeclare_extends_blocks;
use crate::flatten::utils::merge_models;
use crate::flatten::{apply_modification_to_model, FlattenError, ModifyContext};
use std::sync::Arc;

impl Flattener {
    /// Iterative flatten_inheritance to avoid stack overflow on deep extends chains.
    /// Frame: (parent_arc_or_none, current_model_arc, qualified_name, extends_clauses, next_index).
    pub(crate) fn flatten_inheritance(
        &mut self,
        arc: &mut Arc<Model>,
        current_qualified: &str,
    ) -> Result<(), FlattenError> {
        let model = Arc::make_mut(arc);
        apply_redeclare_extends_blocks(model);
        let extends = std::mem::take(&mut model.extends);
        type Frame = (Option<Arc<Model>>, Arc<Model>, String, Vec<ExtendsClause>, usize);
        let mut stack: Vec<Frame> = vec![(None, Arc::clone(arc), current_qualified.to_string(), extends, 0)];

        while let Some((parent, current, qual, ext, idx)) = stack.pop() {
            if idx >= ext.len() {
                if let Some(mut p) = parent {
                    merge_models(Arc::make_mut(&mut p), current.as_ref());
                }
                continue;
            }
            let clause = &ext[idx];
            let raw_extends = clause.model_name.trim();
            let (mut base_name, base_from_inner) = if !raw_extends.contains('.') {
                if let Some(ic) = current.as_ref().find_inner_class(raw_extends) {
                    let bn = if qual.is_empty() {
                        raw_extends.to_string()
                    } else {
                        format!("{}.{}", qual, raw_extends)
                    };
                    (bn, Some(Arc::new(ic.clone())))
                } else {
                    let mut bn =
                        Self::resolve_import_prefix(current.as_ref(), &clause.model_name, &qual);
                    bn = Self::qualify_in_scope(&qual, &bn);
                    (bn, None)
                }
            } else {
                let mut bn =
                    Self::resolve_import_prefix(current.as_ref(), &clause.model_name, &qual);
                bn = Self::qualify_in_scope(&qual, &bn);
                (bn, None)
            };
            if base_name.ends_with("ExternalObject") {
                stack.push((parent, current, qual, ext, idx + 1));
                continue;
            }
            let mut base_model = if let Some(m) = base_from_inner {
                m
            } else {
                match self.loader.load_model_silent(&base_name, true) {
                Ok(m) => m,
                Err(_first_err) => {
                    let orig = Self::resolve_import_prefix(current.as_ref(), &clause.model_name, &qual);
                    let bare = if orig.contains('.') {
                        orig.split('.').next().unwrap_or(&orig).to_string()
                    } else {
                        orig.clone()
                    };
                    let suffix = if bare != orig { &orig[bare.len()..] } else { "" };
                    let mut found = None;
                    let mut scope = qual.clone();
                    while let Some((p, _)) = scope.rsplit_once('.') {
                        let candidate = format!("{}.{}{}", p, bare, suffix);
                        if let Ok(m) = self.loader.load_model_silent(&candidate, true) {
                            base_name = candidate;
                            found = Some(m);
                            break;
                        }
                        scope = p.to_string();
                    }
                    match found {
                        Some(m) => m,
                        None => self.loader.load_model(&base_name)?,
                    }
                }
            }
            };
            let mod_ctx =
                ModifyContext::for_extends_scope(&qual, self.coarse_constrainedby_only);
            for modification in &clause.modifications {
                apply_modification_to_model(
                    Arc::make_mut(&mut base_model),
                    modification,
                    &mod_ctx,
                    Some(&mut self.loader),
                )?;
            }
            apply_redeclare_extends_blocks(Arc::make_mut(&mut base_model));
            let base_extends = std::mem::take(&mut Arc::make_mut(&mut base_model).extends);
            stack.push((parent, Arc::clone(&current), qual, ext, idx + 1));
            stack.push((Some(current), base_model, base_name, base_extends, 0));
        }
        Ok(())
    }
}
