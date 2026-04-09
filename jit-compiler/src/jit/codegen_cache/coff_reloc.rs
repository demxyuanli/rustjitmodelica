//! Windows COFF load and relocation for cached object artifacts.

use std::collections::HashMap;

use object::{Object, ObjectSection, ObjectSymbol, RelocationTarget, SymbolSection};

use super::exec_buffer::ExecCodeBuffer;

pub(crate) fn load_coff_object_exec_windows(
    raw: &[u8],
    runtime_symbols: &HashMap<String, *const u8>,
) -> Option<(ExecCodeBuffer, usize, Vec<Box<usize>>)> {
    let obj = object::File::parse(raw).ok()?;
    let mut layouts: HashMap<object::SectionIndex, (usize, usize)> = HashMap::new();
    let mut total_len = 0usize;

    let mut import_slots: Vec<Box<usize>> = Vec::new();
    for section in obj.sections() {
        let size = usize::try_from(section.size()).ok()?;
        if size == 0 {
            continue;
        }
        let name = section.name().ok().unwrap_or("");
        if name.starts_with(".debug") {
            continue;
        }
        let align = usize::try_from(section.align()).ok()?.max(1);
        total_len = align_up(total_len, align);
        layouts.insert(section.index(), (total_len, size));
        total_len = total_len.saturating_add(size);
    }

    if total_len == 0 {
        return None;
    }

    let exec = ExecCodeBuffer::alloc_rw(total_len)?;
    unsafe {
        std::ptr::write_bytes(exec.as_mut_ptr(), 0u8, total_len);
    }

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
    for section in obj.sections() {
        let Some((base_off, _)) = layouts.get(&section.index()).copied() else {
            continue;
        };
        for (rel_off, reloc) in section.relocations() {
            let place_off = base_off.saturating_add(usize::try_from(rel_off).ok()?);
            let target_addr = match reloc.target() {
                RelocationTarget::Symbol(sym_idx) => {
                    resolve_symbol_addr_windows(
                        &obj,
                        sym_idx,
                        &layouts,
                        base_addr,
                        runtime_symbols,
                        &mut import_slots,
                    )?
                }
                RelocationTarget::Section(sec_idx) => {
                    let (sec_off, _) = *layouts.get(&sec_idx)?;
                    base_addr.saturating_add(sec_off)
                }
                _ => return None,
            };
            apply_relocation_windows(
                exec.as_mut_ptr(),
                total_len,
                base_addr,
                place_off,
                target_addr,
                reloc.kind(),
                reloc.size(),
                reloc.addend(),
            )?;
        }
    }

    let sym = obj.symbol_by_name("calc_derivs")?;
    let func_offset = symbol_runtime_offset_windows(&sym, &layouts)?;
    if !exec.make_rx() {
        return None;
    }
    Some((exec, func_offset, import_slots))
}

fn symbol_runtime_offset_windows(
    sym: &object::Symbol<'_, '_>,
    layouts: &HashMap<object::SectionIndex, (usize, usize)>,
) -> Option<usize> {
    match sym.section() {
        SymbolSection::Section(sec_idx) => {
            let (sec_off, sec_size) = *layouts.get(&sec_idx)?;
            let in_sec = usize::try_from(sym.address()).ok()?;
            if in_sec >= sec_size {
                return None;
            }
            Some(sec_off.saturating_add(in_sec))
        }
        _ => None,
    }
}

fn resolve_symbol_addr_windows(
    obj: &object::File<'_>,
    sym_idx: object::SymbolIndex,
    layouts: &HashMap<object::SectionIndex, (usize, usize)>,
    base_addr: usize,
    runtime_symbols: &HashMap<String, *const u8>,
    import_slots: &mut Vec<Box<usize>>,
) -> Option<usize> {
    let sym = obj.symbol_by_index(sym_idx).ok()?;
    if let Some(off) = symbol_runtime_offset_windows(&sym, layouts) {
        return Some(base_addr.saturating_add(off));
    }
    resolve_external_symbol_windows(sym.name().ok()?, runtime_symbols, import_slots)
}

fn resolve_external_symbol_windows(
    raw_name: &str,
    runtime_symbols: &HashMap<String, *const u8>,
    import_slots: &mut Vec<Box<usize>>,
) -> Option<usize> {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    let mut candidates = Vec::new();
    candidates.push(raw_name.to_string());
    if let Some(s) = raw_name.strip_prefix("__imp_") {
        candidates.push(s.to_string());
    }
    if let Some(s) = raw_name.strip_prefix('_') {
        candidates.push(s.to_string());
    }
    for c in &candidates {
        if let Some(&ptr) = runtime_symbols.get(c) {
            let addr = ptr as usize;
            if raw_name.starts_with("__imp_") {
                import_slots.push(Box::new(addr));
                return import_slots.last().map(|b| &**b as *const usize as usize);
            }
            return Some(addr);
        }
    }
    for c in &candidates {
        let mut c_bytes = c.as_bytes().to_vec();
        c_bytes.push(0);
        let ptr = unsafe { GetProcAddress(GetModuleHandleA(std::ptr::null()), c_bytes.as_ptr()) };
        if let Some(p) = ptr {
            let addr = p as usize;
            if raw_name.starts_with("__imp_") {
                import_slots.push(Box::new(addr));
                return import_slots.last().map(|b| &**b as *const usize as usize);
            }
            return Some(addr);
        }
    }
    None
}

fn apply_relocation_windows(
    image: *mut u8,
    image_len: usize,
    base_addr: usize,
    place_off: usize,
    target_addr: usize,
    kind: object::RelocationKind,
    size_bits: u8,
    addend: i64,
) -> Option<()> {
    let target = (target_addr as i128).saturating_add(addend as i128);
    match (kind, size_bits) {
        (object::RelocationKind::Absolute, 8) | (object::RelocationKind::ImageOffset, 8) => {
            write_u8(image, image_len, place_off, target as u8)
        }
        (object::RelocationKind::Absolute, 16) | (object::RelocationKind::ImageOffset, 16) => {
            write_u16(image, image_len, place_off, target as u16)
        }
        (object::RelocationKind::Absolute, 64) | (object::RelocationKind::ImageOffset, 64) => {
            write_u64(image, image_len, place_off, target as u64)
        }
        (object::RelocationKind::Absolute, 32) | (object::RelocationKind::ImageOffset, 32) => {
            write_u32(image, image_len, place_off, target as u32)
        }
        (object::RelocationKind::Relative, 8) => {
            let place_next = (base_addr as i128).saturating_add(place_off as i128).saturating_add(1);
            let disp = target.saturating_sub(place_next);
            let disp8 = i8::try_from(disp).ok()?;
            write_u8(image, image_len, place_off, disp8 as u8)
        }
        (object::RelocationKind::Relative, 16) => {
            let place_next = (base_addr as i128).saturating_add(place_off as i128).saturating_add(2);
            let disp = target.saturating_sub(place_next);
            let disp16 = i16::try_from(disp).ok()?;
            write_u16(image, image_len, place_off, disp16 as u16)
        }
        (object::RelocationKind::Relative, 32) => {
            let place_next = (base_addr as i128).saturating_add(place_off as i128).saturating_add(4);
            let disp = target.saturating_sub(place_next);
            if disp < i32::MIN as i128 || disp > i32::MAX as i128 {
                return None;
            }
            write_u32(image, image_len, place_off, disp as i32 as u32)
        }
        (object::RelocationKind::Relative, 64) => {
            let place_next = (base_addr as i128).saturating_add(place_off as i128).saturating_add(8);
            let disp = target.saturating_sub(place_next);
            let disp64 = i64::try_from(disp).ok()?;
            write_u64(image, image_len, place_off, disp64 as u64)
        }
        (object::RelocationKind::PltRelative, 32) => {
            let place_next = (base_addr as i128).saturating_add(place_off as i128).saturating_add(4);
            let disp = target.saturating_sub(place_next);
            write_u32(image, image_len, place_off, disp as i32 as u32)
        }
        (object::RelocationKind::PltRelative, 64) => {
            write_u64(image, image_len, place_off, target as u64)
        }
        (object::RelocationKind::GotRelative, 32) => {
            let place_next = (base_addr as i128).saturating_add(place_off as i128).saturating_add(4);
            let disp = target.saturating_sub(place_next);
            write_u32(image, image_len, place_off, disp as i32 as u32)
        }
        (object::RelocationKind::GotRelative, 64) => {
            write_u64(image, image_len, place_off, target as u64)
        }
        (object::RelocationKind::GotBaseRelative, 32) => {
            write_u32(image, image_len, place_off, target as u32)
        }
        (object::RelocationKind::GotBaseOffset, 64) => {
            write_u64(image, image_len, place_off, target as u64)
        }
        (object::RelocationKind::GotBaseOffset, 32) => {
            write_u32(image, image_len, place_off, target as u32)
        }
        _ => None,
    }
}

fn write_u32(image: *mut u8, image_len: usize, off: usize, v: u32) -> Option<()> {
    if off.checked_add(4)? > image_len {
        return None;
    }
    let bytes = v.to_le_bytes();
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), image.add(off), 4);
    }
    Some(())
}

fn write_u8(image: *mut u8, image_len: usize, off: usize, v: u8) -> Option<()> {
    if off.checked_add(1)? > image_len {
        return None;
    }
    unsafe {
        std::ptr::write(image.add(off), v);
    }
    Some(())
}

fn write_u16(image: *mut u8, image_len: usize, off: usize, v: u16) -> Option<()> {
    if off.checked_add(2)? > image_len {
        return None;
    }
    let bytes = v.to_le_bytes();
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), image.add(off), 2);
    }
    Some(())
}

fn write_u64(image: *mut u8, image_len: usize, off: usize, v: u64) -> Option<()> {
    if off.checked_add(8)? > image_len {
        return None;
    }
    let bytes = v.to_le_bytes();
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), image.add(off), 8);
    }
    Some(())
}

fn align_up(value: usize, align: usize) -> usize {
    let rem = value % align;
    if rem == 0 {
        value
    } else {
        value + (align - rem)
    }
}
