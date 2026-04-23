//! Windows COFF load and relocation for cached object artifacts.

use std::collections::HashMap;

use object::{Object, ObjectSection, ObjectSymbol, RelocationTarget, SymbolSection};

use super::exec_buffer::ExecCodeBuffer;
use super::reloc_trace;

/// Sections omitted from the in-memory image for both **disk** and **AOT** COFF loads.
///
/// Cranelift emits unwind/metadata sections (`.pdata` / `.xdata`) that are not required to
/// execute `calc_derivs` from an anonymous RX mapping. More importantly, **`load_aot_blob`
/// and `load_coff_object` must use the same inclusion rules** so the same on-disk object
/// bytes map to the **same** virtual layout whether the loader is the tiered AOT path or the
/// `jit-codegen` disk cache path. Previously `load_coff_object` kept `.pdata` / `.xdata` while
/// `load_aot_blob` dropped them, which could make one path accept a blob and produce a
/// different `base_addr` / relocation outcome than the other — a prime suspect for intermittent
/// warm-cache access violations when the artifact tier flips between runs.
fn include_section_in_image(section_name: &str) -> bool {
    if section_name.starts_with(".debug") {
        return false;
    }
    if section_name.starts_with(".pdata") {
        return false;
    }
    if section_name.starts_with(".xdata") {
        return false;
    }
    true
}

pub(crate) fn load_coff_object_exec_windows(
    raw: &[u8],
    runtime_symbols: &HashMap<String, *const u8>,
    trace_ctx: &str,
) -> Option<(ExecCodeBuffer, usize, Vec<Box<usize>>)> {
    if reloc_trace::trace_basic() {
        reloc_trace::trace_line(format_args!(
            "{} load_coff_object raw_len={} runtime_syms={}",
            trace_ctx,
            raw.len(),
            runtime_symbols.len()
        ));
    }
    let obj = match object::File::parse(raw) {
        Ok(o) => o,
        Err(e) => {
            if reloc_trace::trace_basic() {
                reloc_trace::trace_line(format_args!("{} COFF parse failed: {e}", trace_ctx));
            }
            return None;
        }
    };
    let mut layouts: HashMap<object::SectionIndex, (usize, usize)> = HashMap::new();
    let mut total_len = 0usize;

    let mut import_slots: Vec<Box<usize>> = Vec::new();
    let mut section_kept = 0usize;
    for section in obj.sections() {
        let size = usize::try_from(section.size()).ok()?;
        if size == 0 {
            continue;
        }
        let name = section.name().ok().unwrap_or("");
        if !include_section_in_image(name) {
            continue;
        }
        section_kept += 1;
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
    let mut reloc_total = 0usize;
    let mut reloc_by_section: HashMap<String, usize> = HashMap::new();
    for section in obj.sections() {
        let Some((base_off, _)) = layouts.get(&section.index()).copied() else {
            continue;
        };
        let sec_name = section.name().ok().unwrap_or("").to_string();
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
            reloc_total += 1;
            if reloc_trace::trace_sections() {
                *reloc_by_section.entry(sec_name.clone()).or_insert(0) += 1;
            }
        }
    }

    let sym = obj.symbol_by_name("calc_derivs")?;
    let func_offset = symbol_runtime_offset_windows(&sym, &layouts)?;
    if !exec.make_rx() {
        if reloc_trace::trace_basic() {
            reloc_trace::trace_line(format_args!(
                "{} VirtualProtect RX failed func_offset=0x{:x}",
                trace_ctx, func_offset
            ));
        }
        return None;
    }
    if reloc_trace::trace_basic() {
        reloc_trace::trace_line(format_args!(
            "{} ok sections={} image_len={} relocs={} import_slots={} func_off=0x{:x} base=0x{:x}",
            trace_ctx,
            section_kept,
            total_len,
            reloc_total,
            import_slots.len(),
            func_offset,
            base_addr
        ));
    }
    if reloc_trace::trace_sections() && !reloc_by_section.is_empty() {
        let mut pairs: Vec<_> = reloc_by_section.into_iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        for (n, c) in pairs {
            reloc_trace::trace_line(format_args!("{}   reloc section {:?} count={}", trace_ctx, n, c));
        }
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

/// Load a code blob from an AOT archive, performing COFF relocation and symbol
/// resolution.  Returns the executable buffer, function offset within the buffer,
/// and import-slot keep-alives.
///
/// `import_symbols` lists the external symbols the code blob expects.  They are
/// resolved against `runtime_symbols` and (on Windows) the current process.
pub(crate) fn load_aot_blob_exec_windows(
    raw: &[u8],
    import_symbols: &[String],
    runtime_symbols: &HashMap<String, *const u8>,
    trace_ctx: &str,
) -> Option<(ExecCodeBuffer, usize, Vec<Box<usize>>)> {
    if raw.is_empty() {
        return None;
    }

    if reloc_trace::trace_basic() {
        reloc_trace::trace_line(format_args!(
            "{} load_aot_blob raw_len={} import_syms={} runtime_syms={}",
            trace_ctx,
            raw.len(),
            import_symbols.len(),
            runtime_symbols.len()
        ));
    }

    let obj = match object::File::parse(raw) {
        Ok(o) => o,
        Err(e) => {
            if reloc_trace::trace_basic() {
                reloc_trace::trace_line(format_args!(
                    "{} COFF parse failed ({e}); trying raw blob fallback",
                    trace_ctx
                ));
            }
            return load_raw_blob_fallback(raw, trace_ctx);
        }
    };

    let mut layouts: HashMap<object::SectionIndex, (usize, usize)> = HashMap::new();
    let mut total_len = 0usize;
    let mut section_kept = 0usize;

    for section in obj.sections() {
        let size = usize::try_from(section.size()).ok()?;
        if size == 0 {
            continue;
        }
        let name = section.name().ok().unwrap_or("");
        if !include_section_in_image(name) {
            continue;
        }
        section_kept += 1;
        let alignment = usize::try_from(section.align()).ok()?.max(1);
        total_len = align_up(total_len, alignment);
        layouts.insert(section.index(), (total_len, size));
        total_len = total_len.saturating_add(size);
    }

    if total_len == 0 {
        if reloc_trace::trace_basic() {
            reloc_trace::trace_line(format_args!(
                "{} empty image after section filter; raw fallback",
                trace_ctx
            ));
        }
        return load_raw_blob_fallback(raw, trace_ctx);
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

    let mut import_slots: Vec<Box<usize>> = Vec::new();
    let mut merged_symbols = runtime_symbols.clone();
    let mut prefetch_ok = 0usize;
    let mut prefetch_miss = 0usize;
    for sym_name in import_symbols {
        if !merged_symbols.contains_key(sym_name) {
            if let Some(addr) = resolve_external_symbol_windows(
                sym_name,
                runtime_symbols,
                &mut import_slots,
            ) {
                merged_symbols.insert(sym_name.clone(), addr as *const u8);
                prefetch_ok += 1;
            } else {
                prefetch_miss += 1;
                if reloc_trace::trace_basic() {
                    reloc_trace::trace_line(format_args!(
                        "{} prefetch miss for import symbol {:?}",
                        trace_ctx, sym_name
                    ));
                }
            }
        }
    }
    if reloc_trace::trace_basic() {
        reloc_trace::trace_line(format_args!(
            "{} import prefetch ok={} miss={} merged_syms={}",
            trace_ctx,
            prefetch_ok,
            prefetch_miss,
            merged_symbols.len()
        ));
    }

    let base_addr = exec.as_ptr() as usize;
    let mut reloc_total = 0usize;
    let mut reloc_by_section: HashMap<String, usize> = HashMap::new();
    for section in obj.sections() {
        let Some((base_off, _)) = layouts.get(&section.index()).copied() else {
            continue;
        };
        let sec_name = section.name().ok().unwrap_or("").to_string();
        for (rel_off, reloc) in section.relocations() {
            let place_off = base_off.saturating_add(usize::try_from(rel_off).ok()?);
            let target_addr = match reloc.target() {
                RelocationTarget::Symbol(sym_idx) => resolve_symbol_addr_windows(
                    &obj,
                    sym_idx,
                    &layouts,
                    base_addr,
                    &merged_symbols,
                    &mut import_slots,
                )?,
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
            reloc_total += 1;
            if reloc_trace::trace_sections() {
                *reloc_by_section.entry(sec_name.clone()).or_insert(0) += 1;
            }
        }
    }

    let sym = obj.symbol_by_name("calc_derivs")?;
    let func_offset = symbol_runtime_offset_windows(&sym, &layouts)?;
    if !exec.make_rx() {
        if reloc_trace::trace_basic() {
            reloc_trace::trace_line(format_args!(
                "{} VirtualProtect RX failed func_offset=0x{:x}",
                trace_ctx, func_offset
            ));
        }
        return None;
    }
    if reloc_trace::trace_basic() {
        reloc_trace::trace_line(format_args!(
            "{} ok sections={} image_len={} relocs={} import_slots={} func_off=0x{:x} base=0x{:x}",
            trace_ctx,
            section_kept,
            total_len,
            reloc_total,
            import_slots.len(),
            func_offset,
            base_addr
        ));
        let show = import_slots.len().min(16);
        for (i, slot) in import_slots.iter().take(show).enumerate() {
            reloc_trace::trace_line(format_args!(
                "{}   import_slot[{}] *slot={:p} value=0x{:x}",
                trace_ctx,
                i,
                slot.as_ref() as *const usize,
                **slot
            ));
        }
        if import_slots.len() > show {
            reloc_trace::trace_line(format_args!(
                "{}   ... {} more import_slots omitted (trace shows first {})",
                trace_ctx,
                import_slots.len() - show,
                show
            ));
        }
    }
    if reloc_trace::trace_sections() && !reloc_by_section.is_empty() {
        let mut pairs: Vec<_> = reloc_by_section.into_iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        for (n, c) in pairs {
            reloc_trace::trace_line(format_args!("{}   reloc section {:?} count={}", trace_ctx, n, c));
        }
    }
    Some((exec, func_offset, import_slots))
}

fn load_raw_blob_fallback(
    raw: &[u8],
    trace_ctx: &str,
) -> Option<(ExecCodeBuffer, usize, Vec<Box<usize>>)> {
    if raw.is_empty() {
        return None;
    }
    if reloc_trace::trace_basic() {
        reloc_trace::trace_line(format_args!(
            "{} raw_fallback len={} (no COFF reloc; func_off=0)",
            trace_ctx,
            raw.len()
        ));
    }
    let exec = ExecCodeBuffer::alloc_rw(raw.len())?;
    unsafe {
        std::ptr::copy_nonoverlapping(raw.as_ptr(), exec.as_mut_ptr(), raw.len());
    }
    if !exec.make_rx() {
        if reloc_trace::trace_basic() {
            reloc_trace::trace_line(format_args!("{} raw_fallback VirtualProtect failed", trace_ctx));
        }
        return None;
    }
    Some((exec, 0, Vec::new()))
}
