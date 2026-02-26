use std::collections::HashMap;
use crate::ast::{Declaration, Equation, AlgorithmStatement};

pub struct FlattenedModel {
    pub declarations: Vec<Declaration>,
    pub equations: Vec<Equation>,
    pub algorithms: Vec<AlgorithmStatement>,
    pub connections: Vec<(String, String)>,
    pub instances: HashMap<String, String>, // full_path -> type_name
    pub array_sizes: HashMap<String, usize>, // full_path -> size
}
