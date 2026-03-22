use crate::flatten::FlattenedModel;
use crate::loader::ModelLoader;

mod builtin;
mod function_body;
mod record_access;
mod rewrite;
mod subst;
mod traverse;

pub(crate) use builtin::is_builtin_function;
pub(crate) use function_body::get_function_body;

const MAX_INLINE_RECURSION_DEPTH: u32 = 64;

pub fn inline_function_calls(flat: &mut FlattenedModel, loader: &mut ModelLoader) {
    traverse::inline_function_calls_in_model(flat, loader, MAX_INLINE_RECURSION_DEPTH);
}
