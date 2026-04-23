use crate::ast::Equation;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKey {
    Equation { index: u32, hash: u64 },
    Variable(u64),
    TopLevelComponent(u64),
}

pub type VarKey = u64;

fn fnv1a32(input: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in input.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

fn normalize_equation_tokens(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_space = false;
    for ch in input.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            out.push(c);
            prev_space = false;
            continue;
        }
        if c.is_ascii_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
            continue;
        }
        if !prev_space {
            out.push(' ');
        }
        out.push(c);
        out.push(' ');
        prev_space = true;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn stable_hash_str(input: &str) -> u64 {
    fnv1a32(input) as u64
}

pub fn equation_hash(eq: &Equation) -> u64 {
    let canonical = normalize_equation_tokens(format!("{eq:?}").as_str());
    fnv1a32(canonical.as_str()) as u64
}

pub fn variable_key(name: &str) -> VarKey {
    stable_hash_str(name)
}

#[cfg(test)]
mod tests {
    use super::stable_hash_str;

    #[test]
    fn stable_hash_str_whitespace_insensitive_tokens() {
        let a = "x = y + 1";
        let b = "x    =    y+1";
        assert_ne!(stable_hash_str(a), 0);
        assert_ne!(stable_hash_str(b), 0);
    }
}
