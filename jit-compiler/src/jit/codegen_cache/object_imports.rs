//! Extract undefined symbol names from a relocatable object (COFF / ELF) for AOT TOC
//! and runtime relocation.

use object::{Object, ObjectSymbol, SymbolKind, SymbolSection};

/// Returns sorted, unique names of undefined symbols the object needs resolved at load time.
/// Empty when `bytes` is not a parseable object (e.g. raw JIT blob).
pub fn undefined_import_names_from_object(bytes: &[u8]) -> Vec<String> {
    let Ok(obj) = object::File::parse(bytes) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for sym in obj.symbols() {
        if sym.kind() == SymbolKind::File {
            continue;
        }
        if sym.section() != SymbolSection::Undefined {
            continue;
        }
        let Ok(name) = sym.name() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        if name == "calc_derivs" {
            continue;
        }
        names.push(name.to_string());
    }
    names.sort();
    names.dedup();
    names
}
