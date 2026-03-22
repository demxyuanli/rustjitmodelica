use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, OnceLock};

/// Interned variable identifier. Wraps u32 for cheap Copy/Eq/Hash.
/// Resolve to `&str` via `StringInterner::resolve()` or the global `resolve_id()`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarId(pub u32);

impl VarId {
    pub const INVALID: VarId = VarId(u32::MAX);

    pub fn as_str(&self) -> String {
        resolve_id(*self)
    }
}

impl fmt::Debug for VarId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VarId({})", self.0)
    }
}

impl fmt::Display for VarId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v#{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct StringInterner {
    map: HashMap<String, u32>,
    strings: Vec<String>,
}

impl StringInterner {
    pub fn new() -> Self {
        StringInterner {
            map: HashMap::new(),
            strings: Vec::new(),
        }
    }

    pub fn intern(&mut self, s: &str) -> VarId {
        if let Some(&id) = self.map.get(s) {
            return VarId(id);
        }
        let id = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.map.insert(s.to_string(), id);
        VarId(id)
    }

    pub fn get_or_intern(&mut self, s: String) -> VarId {
        if let Some(&id) = self.map.get(&s) {
            return VarId(id);
        }
        let id = self.strings.len() as u32;
        self.strings.push(s.clone());
        self.map.insert(s, id);
        VarId(id)
    }

    pub fn resolve(&self, id: VarId) -> &str {
        &self.strings[id.0 as usize]
    }

    pub fn contains(&self, s: &str) -> Option<VarId> {
        self.map.get(s).map(|&id| VarId(id))
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.strings.len()
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

fn global_interner() -> &'static Mutex<StringInterner> {
    static INTERNER: OnceLock<Mutex<StringInterner>> = OnceLock::new();
    INTERNER.get_or_init(|| Mutex::new(StringInterner::new()))
}

/// Intern a string into the global interner, returning a compact VarId.
pub fn intern(s: &str) -> VarId {
    global_interner().lock().unwrap().intern(s)
}

/// Resolve a VarId back to its original string (allocates).
pub fn resolve_id(id: VarId) -> String {
    global_interner().lock().unwrap().resolve(id).to_string()
}

/// Check the global interner without resolving. Returns true if `id` resolves to `s`.
pub fn var_is(id: VarId, s: &str) -> bool {
    global_interner().lock().unwrap().resolve(id) == s
}

/// Check if the resolved string starts with a prefix.
pub fn var_starts_with(id: VarId, prefix: &str) -> bool {
    global_interner().lock().unwrap().resolve(id).starts_with(prefix)
}
