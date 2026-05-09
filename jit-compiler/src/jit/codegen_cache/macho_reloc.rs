//! macOS Mach-O load and relocation for cached object artifacts.

use std::collections::HashMap;

use object::macho;
use object::{Object, ObjectSection, RelocationTarget};

use super::exec_buffer::ExecCodeBuffer;
use super::reloc_trace;

fn include_section_in_image(section_name: &str) -> bool {
    if section_name.starts_with(".debug") {
        return false;
    }
    // Keep __text, __data, __const, __got, etc.
    true
}

/// Apply a Mach-O relocation at `place` for `target_addr`.
fn apply_relocation_macho(
    image: *mut u8,
    image_len: usize,
    base_addr: usize,
    place_off: usize,
    target_addr: u64,
    reloc_kind: u32,
    reloc_size: u8,
    reloc_addend: i64,
) -> Option<()> {
    if place_off.saturating_add(8) > image_len {
        return None;
    }
    let place = unsafe { image.add(place_off) };
    let addend = reloc_addend;

    // Determine effective target: for x86_64 Mach-O, relocations are encoded with
    // the difference from the section base; for ARM64 they use addends.
    let effective = (target_addr as i64).wrapping_add(addend);

    match (reloc_kind, reloc_size) {
        // x86_64 relocation types (from macho::X86_64_RELOC_*)
        (0, 32) | (0, 64) => {
            // X86_64_RELOC_UNSIGNED: absolute address
            unsafe { std::ptr::write_unaligned(place as *mut u64, effective as u64); }
        }
        (1, 32) | (1, 64) => {
            // X86_64_RELOC_SIGNED: pc-relative signed
            let rel = effective.wrapping_sub(base_addr.wrapping_add(place_off) as i64);
            unsafe { std::ptr::write_unaligned(place as *mut i32, rel as i32); }
        }
        (2, 32) => {
            // X86_64_RELOC_BRANCH: 32-bit pc-relative for call/jmp
            let rel = effective.wrapping_sub(base_addr.wrapping_add(place_off) as i64);
            unsafe { std::ptr::write_unaligned(place as *mut i32, rel as i32); }
        }
        (3, 32) | (3, 64) => {
            // X86_64_RELOC_GOT_LOAD: GOT entry offset
            unsafe { std::ptr::write_unaligned(place as *mut u64, effective as u64); }
        }
        (4, 32) | (4, 64) => {
            // X86_64_RELOC_GOT: GOT entry address
            unsafe { std::ptr::write_unaligned(place as *mut u64, effective as u64); }
        }
        (5, 32) | (5, 64) => {
            // X86_64_RELOC_SUBTRACTOR: paired with UNSIGNED for movq@GOTPCREL
            let val = effective.wrapping_sub(base_addr.wrapping_add(place_off) as i64);
            unsafe { std::ptr::write_unaligned(place as *mut i32, val as i32); }
        }
        (6, 32) | (6, 64) => {
            // X86_64_RELOC_SIGNED_1: pc-relative, subtract 1
            let rel = effective.wrapping_sub(base_addr.wrapping_add(place_off) as i64).wrapping_sub(1);
            unsafe { std::ptr::write_unaligned(place as *mut i32, rel as i32); }
        }
        (7, 32) | (7, 64) => {
            // X86_64_RELOC_SIGNED_2: pc-relative, subtract 2
            let rel = effective.wrapping_sub(base_addr.wrapping_add(place_off) as i64).wrapping_sub(2);
            unsafe { std::ptr::write_unaligned(place as *mut i32, rel as i32); }
        }
        (8, 32) | (8, 64) => {
            // X86_64_RELOC_SIGNED_4: pc-relative, subtract 4
            let rel = effective.wrapping_sub(base_addr.wrapping_add(place_off) as i64).wrapping_sub(4);
            unsafe { std::ptr::write_unaligned(place as *mut i32, rel as i32); }
        }
        // ARM64 relocation types (from macho::ARM64_RELOC_*)
        (0, _) => {
            // ARM64_RELOC_UNSIGNED: absolute
            unsafe { std::ptr::write_unaligned(place as *mut u64, effective as u64); }
        }
        (1, _) => {
            // ARM64_RELOC_SUBTRACTOR: paired subtractor
            unsafe { std::ptr::write_unaligned(place as *mut u64, effective as u64); }
        }
        (3, _) => {
            // ARM64_RELOC_PAGEOFF12: page offset into GOT-like structure
            unsafe { std::ptr::write_unaligned(place as *mut u32, effective as u32); }
        }
        (4, _) => {
            // ARM64_RELOC_GOT_LOAD_PAGE21: ADRP page for GOT
            let pc_page = ((base_addr.wrapping_add(place_off)) as u64) & !0xFFF;
            let target_page = effective as u64 & !0xFFF;
            let diff = target_page.wrapping_sub(pc_page);
            let imm = (diff as i64 >> 12) as u32 & 0x3;
            let lo = ((diff as i64 >> 12) as u32 >> 2) & 0x7FFFF;
            let val = (imm << 29) | (lo << 5);
            unsafe {
                let existing = std::ptr::read_unaligned(place as *mut u32);
                std::ptr::write_unaligned(place as *mut u32, existing | val);
            }
        }
        (5, _) => {
            // ARM64_RELOC_GOT_LOAD_PAGEOFF12: LDR offset for GOT
            let scaled = (effective as u64 & 0xFFF) >> 3;
            unsafe {
                let existing = std::ptr::read_unaligned(place as *mut u32);
                let val = (existing & !(0xFFF << 10)) | ((scaled as u32 & 0xFFF) << 10);
                std::ptr::write_unaligned(place as *mut u32, val);
            }
        }
        (6, _) => {
            // ARM64_RELOC_POINTER_TO_GOT: pointer to GOT entry
            unsafe { std::ptr::write_unaligned(place as *mut u64, effective as u64); }
        }
        (8, _) => {
            // ARM64_RELOC_BRANCH26: 26-bit B/BL offset
            let offset = effective.wrapping_sub(base_addr.wrapping_add(place_off) as i64);
            let imm26 = (offset as u32 >> 2) & 0x3FF_FFFF;
            unsafe {
                let existing = std::ptr::read_unaligned(place as *mut u32);
                std::ptr::write_unaligned(place as *mut u32, existing | imm26);
            }
        }
        (9, _) => {
            // ARM64_RELOC_PAGE21: ADRP page
            let pc_page = (base_addr.wrapping_add(place_off) as u64) & !0xFFF;
            let target_page = effective as u64 & !0xFFF;
            let diff = target_page.wrapping_sub(pc_page);
            let imm = (diff as i64 >> 12) as u32 & 0x3;
            let lo = ((diff as i64 >> 12) as u32 >> 2) & 0x7FFFF;
            let val = (imm << 29) | (lo << 5);
            unsafe {
                let existing = std::ptr::read_unaligned(place as *mut u32);
                std::ptr::write_unaligned(place as *mut u32, (existing & 0x9F_0000_1F) | val);
            }
        }
        _ => {
            if reloc_trace::trace_basic() {
                reloc_trace::trace_line(format_args!(
                    "macho-reloc: unsupported kind={} size={}",
                    reloc_kind, reloc_size
                ));
            }
            return None;
        }
    }
    Some(())
}

fn resolve_symbol_addr_macho(
    obj: &object::File<'_>,
    sym_idx: object::SymbolIndex,
    layouts: &HashMap<object::SectionIndex, (usize, usize)>,
    base_addr: usize,
    runtime_symbols: &HashMap<String, *const u8>,
) -> Option<u64> {
    let sym = obj.symbol_by_index(sym_idx).ok()?;
    match sym.section() {
        object::SymbolSection::Section(sec_idx) => {
            let (sec_off, _) = *layouts.get(&sec_idx)?;
            let sym_val = usize::try_from(sym.address()).ok()?;
            Some((base_addr.saturating_add(sec_off).saturating_add(sym_val)) as u64)
        }
        object::SymbolSection::Undefined => {
            let name = sym.name().ok()?;
            if let Some(&ptr) = runtime_symbols.get(name) {
                return Some(ptr as u64);
            }
            // Try OS resolver (dlsym equivalent)
            let cname = std::ffi::CString::new(name).ok()?;
            unsafe {
                let addr = libloading::os::unix::Library::open(None, libloading::os::unix::RTLD_NOLOAD | 1)
                    .ok()
                    .and_then(|lib| {
                        lib.get::<*const u8>(cname.as_bytes()).ok()
                    });
                match addr {
                    Some(f) => Some(*f as u64),
                    None => {
                        if reloc_trace::trace_basic() {
                            reloc_trace::trace_line(format_args!(
                                "macho-reloc: undefined symbol '{}' not found",
                                name
                            ));
                        }
                        None
                    }
                }
            }
        }
        object::SymbolSection::Absolute => Some(sym.address()),
        _ => None,
    }
}

fn align_up(x: usize, align: usize) -> usize {
    if align == 0 || align == 1 {
        return x;
    }
    (x + align - 1) & !(align - 1)
}

pub(crate) fn load_macho_object_exec_macos(
    raw: &[u8],
    runtime_symbols: &HashMap<String, *const u8>,
    trace_ctx: &str,
) -> Option<(ExecCodeBuffer, usize, Vec<Box<usize>>)> {
    if reloc_trace::trace_basic() {
        reloc_trace::trace_line(format_args!(
            "{} load_macho_object raw_len={} runtime_syms={}",
            trace_ctx,
            raw.len(),
            runtime_symbols.len()
        ));
    }
    let obj = match object::File::parse(raw) {
        Ok(o) => o,
        Err(e) => {
            if reloc_trace::trace_basic() {
                reloc_trace::trace_line(format_args!("{} Mach-O parse failed: {e}", trace_ctx));
            }
            return None;
        }
    };

    let mut layouts: HashMap<object::SectionIndex, (usize, usize)> = HashMap::new();
    let mut total_len = 0usize;

    for section in obj.sections() {
        let size = usize::try_from(section.size()).ok()?;
        if size == 0 {
            continue;
        }
        let name = section.name().ok().unwrap_or("");
        if !include_section_in_image(name) {
            continue;
        }
        let align = usize::try_from(section.align()).ok()?.max(1);
        total_len = align_up(total_len, align);
        layouts.insert(section.index(), (total_len, size));
        total_len = total_len.saturating_add(size);
    }

    if total_len == 0 {
        if reloc_trace::trace_basic() {
            reloc_trace::trace_line(format_args!("{} abort: empty image after section filter", trace_ctx));
        }
        return None;
    }

    let mut exec = ExecCodeBuffer::alloc_rw(total_len)?;

    // Copy section data
    for section in obj.sections() {
        let Some((base_off, size)) = layouts.get(&section.index()).copied() else {
            continue;
        };
        let data = section.uncompressed_data().ok()?;
        let to_copy = data.len().min(size);
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), exec.as_mut_ptr().add(base_off), to_copy);
        }
    }

    let base_addr = exec.as_ptr() as usize;
    let mut import_slots: Vec<Box<usize>> = Vec::new();

    // Apply relocations
    for section in obj.sections() {
        let Some((base_off, _)) = layouts.get(&section.index()).copied() else {
            continue;
        };
        for (rel_off, reloc) in section.relocations() {
            let place_off = base_off.saturating_add(usize::try_from(rel_off).ok()?);
            let target_addr = match reloc.target() {
                RelocationTarget::Symbol(sym_idx) => {
                    resolve_symbol_addr_macho(
                        &obj,
                        sym_idx,
                        &layouts,
                        base_addr,
                        runtime_symbols,
                    )?
                }
                RelocationTarget::Section(sec_idx) => {
                    let (sec_off, _) = *layouts.get(&sec_idx)?;
                    (base_addr.saturating_add(sec_off)) as u64
                }
                _ => return None,
            };
            let addend = match reloc.has_implicit_addend() {
                true => {
                    // Read current value at place as addend
                    let place = unsafe { exec.as_ptr().add(place_off) };
                    let cur = unsafe { std::ptr::read_unaligned(place as *const i64) };
                    cur
                }
                false => reloc.addend(),
            };
            apply_relocation_macho(
                exec.as_mut_ptr(),
                total_len,
                base_addr,
                place_off,
                target_addr,
                reloc.kind(),
                reloc.size(),
                addend,
            )?;
        }
    }

    let text_offset = obj
        .sections()
        .find(|s| s.name().map_or(false, |n| n == "__text" || n.starts_with("__TEXT")))
        .and_then(|s| layouts.get(&s.index()))
        .map(|(off, _)| *off)
        .unwrap_or(0);

    // Make executable (on macOS this is done via mprotect by ExecCodeBuffer::make_exec)
    let func_offset = text_offset;
    Some((exec, func_offset, import_slots))
}
