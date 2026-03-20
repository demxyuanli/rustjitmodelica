use super::types::{ArrayInfo, ArrayType};
use cranelift::codegen::ir::StackSlot;
use cranelift::prelude::*;
use cranelift_jit::JITModule;
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use std::collections::HashMap;

pub struct TranslationContext<'a> {
    pub module: &'a mut JITModule,
    pub var_map: &'a mut HashMap<String, Value>,
    pub stack_slots: &'a HashMap<String, StackSlot>,
    pub array_info: &'a HashMap<String, ArrayInfo>,

    // Pointers to data arrays
    pub states_ptr: Value,
    pub discrete_ptr: Value,
    pub params_ptr: Value,
    pub outputs_ptr: Value,
    pub derivs_ptr: Value,
    pub pre_states_ptr: Value,
    pub pre_discrete_ptr: Value,
    pub when_states_ptr: Value,
    pub crossings_ptr: Value,

    // Counters (mutable references)
    pub when_idx: &'a mut usize,
    pub crossings_idx: &'a mut usize,

    #[allow(dead_code)]
    pub state_vars: &'a [String],
    #[allow(dead_code)]
    pub discrete_vars: &'a [String],
    #[allow(dead_code)]
    pub output_vars: &'a [String],
    pub state_var_index: &'a HashMap<String, usize>,
    pub discrete_var_index: &'a HashMap<String, usize>,
    pub param_var_index: &'a HashMap<String, usize>,
    pub output_var_index: &'a HashMap<String, usize>,

    /// When set, JIT writes residual and tearing value on Newton failure (status 2) for diagnostics.
    pub diag_residual_ptr: Option<Value>,
    pub diag_x_ptr: Option<Value>,

    /// FUNC-7 / EXT-3: Cache import func_id by name then ABI tag (f64 vs const char*, etc.).
    pub declared_imports: Option<&'a mut HashMap<String, HashMap<String, FuncId>>>,

    /// FUNC-7: String literal -> DataId for JIT external calls (const char*).
    pub string_literal_cache: Option<&'a mut HashMap<String, DataId>>,
    /// Reusable DataDescription and counter for creating string data.
    pub string_literal_data_ctx: Option<&'a mut DataDescription>,
    pub string_data_counter: Option<&'a mut usize>,
}

impl<'a> TranslationContext<'a> {
    /// FUNC-7: Get or create DataId for string literal (null-terminated); returns None if string args not enabled.
    pub fn get_or_create_string_data(&mut self, s: &str) -> Result<Option<DataId>, String> {
        let (cache, data_ctx, ctr) = match (
            self.string_literal_cache.as_deref_mut(),
            self.string_literal_data_ctx.as_deref_mut(),
            self.string_data_counter.as_deref_mut(),
        ) {
            (Some(c), Some(d), Some(n)) => (c, d, n),
            _ => return Ok(None),
        };
        if let Some(&id) = cache.get(s) {
            return Ok(Some(id));
        }
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0u8);
        data_ctx.define(bytes.into_boxed_slice());
        *ctr += 1;
        let name = format!("jit_str_{}", *ctr);
        let id = self
            .module
            .declare_data(&name, Linkage::Local, false, false)
            .map_err(|e| e.to_string())?;
        self.module
            .define_data(id, data_ctx)
            .map_err(|e| e.to_string())?;
        data_ctx.clear();
        cache.insert(s.to_string(), id);
        Ok(Some(id))
    }
}

impl<'a> TranslationContext<'a> {
    pub fn new(
        module: &'a mut JITModule,
        var_map: &'a mut HashMap<String, Value>,
        stack_slots: &'a HashMap<String, StackSlot>,
        array_info: &'a HashMap<String, ArrayInfo>,
        states_ptr: Value,
        discrete_ptr: Value,
        params_ptr: Value,
        outputs_ptr: Value,
        derivs_ptr: Value,
        pre_states_ptr: Value,
        pre_discrete_ptr: Value,
        when_states_ptr: Value,
        crossings_ptr: Value,
        when_idx: &'a mut usize,
        crossings_idx: &'a mut usize,
        state_vars: &'a [String],
        discrete_vars: &'a [String],
        output_vars: &'a [String],
        state_var_index: &'a HashMap<String, usize>,
        discrete_var_index: &'a HashMap<String, usize>,
        param_var_index: &'a HashMap<String, usize>,
        output_var_index: &'a HashMap<String, usize>,
        diag_residual_ptr: Option<Value>,
        diag_x_ptr: Option<Value>,
        declared_imports: Option<&'a mut HashMap<String, HashMap<String, FuncId>>>,
        string_literal_cache: Option<&'a mut HashMap<String, DataId>>,
        string_literal_data_ctx: Option<&'a mut DataDescription>,
        string_data_counter: Option<&'a mut usize>,
    ) -> Self {
        Self {
            module,
            var_map,
            stack_slots,
            array_info,
            states_ptr,
            discrete_ptr,
            params_ptr,
            outputs_ptr,
            derivs_ptr,
            pre_states_ptr,
            pre_discrete_ptr,
            when_states_ptr,
            crossings_ptr,
            when_idx,
            crossings_idx,
            state_vars,
            discrete_vars,
            output_vars,
            state_var_index,
            discrete_var_index,
            param_var_index,
            output_var_index,
            diag_residual_ptr,
            diag_x_ptr,
            declared_imports,
            string_literal_cache,
            string_literal_data_ctx,
            string_data_counter,
        }
    }

    pub fn state_index(&self, name: &str) -> Option<usize> {
        self.state_var_index.get(name).copied()
    }
    pub fn discrete_index(&self, name: &str) -> Option<usize> {
        self.discrete_var_index.get(name).copied()
    }
    pub fn output_index(&self, name: &str) -> Option<usize> {
        self.output_var_index.get(name).copied()
    }
    pub fn param_index(&self, name: &str) -> Option<usize> {
        self.param_var_index.get(name).copied()
    }
    pub fn array_storage(&self, name: &str) -> Option<(ArrayType, usize)> {
        self.array_info
            .get(name)
            .map(|info| (info.array_type, info.start_index))
            .or_else(|| {
                let first = format!("{}_1", name);
                self.state_index(&first)
                    .map(|start_index| (ArrayType::State, start_index))
                    .or_else(|| self.discrete_index(&first).map(|start_index| (ArrayType::Discrete, start_index)))
                    .or_else(|| self.param_index(&first).map(|start_index| (ArrayType::Parameter, start_index)))
                    .or_else(|| self.output_index(&first).map(|start_index| (ArrayType::Output, start_index)))
                    .or_else(|| self.state_index(name).map(|start_index| (ArrayType::State, start_index)))
                    .or_else(|| self.discrete_index(name).map(|start_index| (ArrayType::Discrete, start_index)))
                    .or_else(|| self.param_index(name).map(|start_index| (ArrayType::Parameter, start_index)))
                    .or_else(|| self.output_index(name).map(|start_index| (ArrayType::Output, start_index)))
            })
    }
    pub fn array_len(&self, name: &str) -> Option<usize> {
        if let Some(info) = self.array_info.get(name) {
            return Some(info.size);
        }
        if self.state_index(name).is_some()
            || self.discrete_index(name).is_some()
            || self.param_index(name).is_some()
            || self.output_index(name).is_some()
        {
            return Some(1);
        }
        let mut len = 0usize;
        loop {
            let elem_name = format!("{}_{}", name, len + 1);
            let exists = self.state_index(&elem_name).is_some()
                || self.discrete_index(&elem_name).is_some()
                || self.param_index(&elem_name).is_some()
                || self.output_index(&elem_name).is_some();
            if !exists {
                break;
            }
            len += 1;
            if len >= 100_000 {
                break;
            }
        }
        if len > 0 { Some(len) } else { None }
    }
}
