use super::Flattener;
use crate::compiler::inline::is_builtin_function;

impl Flattener {
    pub(super) fn get_record_components(&mut self, type_name: &str) -> Option<Vec<String>> {
        let short = type_name.rsplit('.').next().unwrap_or(type_name);
        if short == "Complex"
            || type_name.ends_with(".Complex")
            || type_name.ends_with("ComplexOutput")
            || type_name.ends_with("ComplexInput")
        {
            return Some(vec!["re".to_string(), "im".to_string()]);
        }
        if is_builtin_function(type_name) {
            return None;
        }
        let m = self.loader.load_model_silent(type_name, true).ok()?;
        if m.is_record {
            Some(m.declarations.iter().map(|d| d.name.clone()).collect())
        } else {
            None
        }
    }
}
