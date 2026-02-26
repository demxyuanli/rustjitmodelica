use std::collections::HashMap;
use cranelift::prelude::*;
use cranelift_jit::JITModule;
use cranelift::codegen::ir::StackSlot;
use super::types::ArrayInfo;

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
    pub output_var_index: &'a HashMap<String, usize>,
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
        output_var_index: &'a HashMap<String, usize>,
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
            output_var_index,
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
}
