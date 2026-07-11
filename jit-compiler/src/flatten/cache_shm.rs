//! Cross-process shared-memory cache tier.
//!
//! Segment OS identifiers embed [`crate::cache::ir_epoch::IR_SCHEMA_EPOCH`]. When the epoch is
//! bumped, new processes use a fresh segment namespace so stale serialized blobs are not reused.
//!
//! Record keys stored in the arena must match the strings passed to [`shm_get`] / [`shm_put`].
//! Callers should use qualified keys from [`crate::cache::cache_key::CacheKeyV2::to_qualified_key`]
//! (they include `L0` / `L1` / `L2` scope) and any `RUSTMODLICA_QUERY_CACHE_NAMESPACE` prefix applied
//! by the query cache layer.
//!
//! Index entries use a stable 64-bit hash of the full key string ([`hash_key64`]); writers update
//! the arena before publishing `seg`/`off`/`len`, and readers verify the embedded key bytes match.

use crate::cache::build_id::binary_build_id;
use crate::cache::ir_epoch::IR_SCHEMA_EPOCH;
use shared_memory::{Shmem, ShmemConf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use xxhash_rust::xxh64::Xxh64;

const MAGIC: u32 = 0x4D_4F_44_43; // "MODC"
const VERSION: u32 = 1;

fn parse_bool_env(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

fn shm_base_name() -> String {
    std::env::var("RUSTMODLICA_CACHE_SHM_NAME")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "rustmodlica_cache".to_string())
}

fn shm_seg_name(seg: u32) -> String {
    let bid = binary_build_id();
    let bid_short = &bid[..8.min(bid.len())];
    format!("{}_e{}_v{}_b{}_seg{}", shm_base_name(), IR_SCHEMA_EPOCH, VERSION, bid_short, seg)
}

#[repr(C)]
struct HeaderV1 {
    magic: u32,
    version: u32,
    /// Read-write lock for cross-process synchronization.
    /// Bits 0-15: writer flag (0=none, 1=writing)
    /// Bits 16-31: reader count
    rwlock: AtomicU32,
    index_cap: u32,
    index_off: u32,
    arena_off: u32,
    arena_len: u32,
    current_seg: u32,
    current_off: u32,
}

const WRITER_MASK: u32 = 0x0000_FFFF;
#[allow(dead_code)]
const READER_MASK: u32 = 0xFFFF_0000;
const READER_ONE: u32 = 0x0001_0000;

#[repr(C)]
#[derive(Clone, Copy)]
struct IndexEntryV1 {
    key_hash: u64,
    seg: u32,
    off: u32,
    len: u32,
    _pad: u32,
}

unsafe fn as_mut<T>(base: *mut u8, off: u32) -> *mut T {
    base.add(off as usize) as *mut T
}

unsafe fn as_slice_mut<T>(base: *mut u8, off: u32, n: usize) -> *mut [T] {
    std::ptr::slice_from_raw_parts_mut(base.add(off as usize) as *mut T, n)
}

struct ShmemWrap(Shmem);

// shared_memory's Shmem is not marked Send/Sync on Windows due to raw pointers.
// We guard access via an in-shared-memory lock and only expose safe byte-copy operations.
// SAFETY: ShmemWrap owns the shared memory segment. The underlying OS shmem handle
// is process-scoped and safe to transfer across threads. All accesses are serialized
// through the in-shared-memory lock (lock_read/lock_write on the segment header).
unsafe impl Send for ShmemWrap {}
// SAFETY: All mutable state inside ShmemWrap is behind the in-shared-memory lock.
// The shared memory segment supports concurrent reads from different threads.
unsafe impl Sync for ShmemWrap {}

struct ShmState {
    seg0: ShmemWrap,
}

// SAFETY: ShmState delegates to ShmemWrap which is Send/Sync.
unsafe impl Send for ShmState {}
// SAFETY: ShmState delegates to ShmemWrap which is Sync.
unsafe impl Sync for ShmState {}

fn seg_size_bytes() -> usize {
    let mb = std::env::var("RUSTMODLICA_CACHE_SHM_SEG_MB")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(100);
    mb * 1024 * 1024
}

fn index_capacity() -> u32 {
    std::env::var("RUSTMODLICA_CACHE_SHM_INDEX_CAP")
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|&v| v >= 1024)
        .unwrap_or(65_536)
}

fn init_seg0(seg0: &ShmemWrap) {
    // SAFETY: seg0.as_ptr() returns a valid pointer to the shared memory segment.
    // The segment is guaranteed to be at least the size of HeaderV1 + index +
    // arena. All pointer arithmetic stays within the segment bounds.
    unsafe {
        let base = seg0.0.as_ptr() as *mut u8;
        let hdr = &mut *as_mut::<HeaderV1>(base, 0);
        if hdr.magic == MAGIC && hdr.version == VERSION {
            return;
        }
        let cap = index_capacity();
        let index_off = std::mem::size_of::<HeaderV1>() as u32;
        let index_bytes = cap as usize * std::mem::size_of::<IndexEntryV1>();
        let arena_off = index_off + index_bytes as u32;
        let arena_len = (seg0.0.len() as u32).saturating_sub(arena_off);
        *hdr = HeaderV1 {
            magic: MAGIC,
            version: VERSION,
            rwlock: AtomicU32::new(0),
            index_cap: cap,
            index_off,
            arena_off,
            arena_len,
            current_seg: 1,
            current_off: 0,
        };
        let idx = as_slice_mut::<IndexEntryV1>(base, index_off, cap as usize);
        let idx = &mut *idx;
        for e in idx.iter_mut() {
            *e = IndexEntryV1 {
                key_hash: 0,
                seg: 0,
                off: 0,
                len: 0,
                _pad: 0,
            };
        }
    }
}

fn state() -> Option<&'static ShmState> {
    static STATE: OnceLock<ShmState> = OnceLock::new();
    if let Some(s) = STATE.get() {
        return Some(s);
    }
    if !parse_bool_env("RUSTMODLICA_CACHE_SHM") {
        return None;
    }
    let seg0 = ShmemConf::new()
        .os_id(&shm_seg_name(0))
        .size(seg_size_bytes())
        .create()
        .or_else(|_| {
            ShmemConf::new()
                .os_id(&shm_seg_name(0))
                .open()
        })
        .ok()?;
    let s = ShmState { seg0: ShmemWrap(seg0) };
    Some(STATE.get_or_init(|| s))
}

fn hash_key64(key: &str) -> u64 {
    let mut h = Xxh64::new(0);
    h.update(key.as_bytes());
    h.digest()
}

fn pack_record(key: &str, payload: &[u8]) -> Vec<u8> {
    let kb = key.as_bytes();
    let klen = kb.len().min(u32::MAX as usize) as u32;
    let mut out = Vec::with_capacity(8 + kb.len() + payload.len());
    out.extend_from_slice(&klen.to_le_bytes());
    out.extend_from_slice(kb);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

fn unpack_record<'a>(buf: &'a [u8], expect_key: &str) -> Option<&'a [u8]> {
    if buf.len() < 8 {
        return None;
    }
    let klen = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() < 4 + klen + 4 {
        return None;
    }
    let key_bytes = &buf[4..4 + klen];
    if key_bytes != expect_key.as_bytes() {
        return None;
    }
    let p_off = 4 + klen;
    let plen = u32::from_le_bytes([buf[p_off], buf[p_off + 1], buf[p_off + 2], buf[p_off + 3]]) as usize;
    let payload_off = p_off + 4;
    if buf.len() < payload_off + plen {
        return None;
    }
    Some(&buf[payload_off..payload_off + plen])
}

fn load_seg(seg: u32) -> Option<Shmem> {
    ShmemConf::new()
        .os_id(&shm_seg_name(seg))
        .size(seg_size_bytes())
        .create()
        .or_else(|_| ShmemConf::new().os_id(&shm_seg_name(seg)).open())
        .ok()
}

/// Acquire read lock for shared access (multiple readers allowed).
/// Waits until no writer is active, then increments reader count.
fn lock_read(hdr: &HeaderV1) {
    let mut spins = 0u32;
    loop {
        let state = hdr.rwlock.load(Ordering::Acquire);
        // Check if writer is active
        if (state & WRITER_MASK) == 0 {
            // Try to increment reader count
            let new_state = state.wrapping_add(READER_ONE);
            if hdr
                .rwlock
                .compare_exchange(state, new_state, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                // Double-check no writer appeared
                if hdr.rwlock.load(Ordering::Acquire) & WRITER_MASK == 0 {
                    return;
                }
                // Writer appeared, release read lock and retry
                hdr.rwlock.fetch_sub(READER_ONE, Ordering::Release);
            }
        }
        spins += 1;
        if spins % 10_000 == 0 {
            std::thread::sleep(std::time::Duration::from_millis(1));
        } else {
            std::hint::spin_loop();
        }
    }
}

/// Release read lock.
fn unlock_read(hdr: &HeaderV1) {
    hdr.rwlock.fetch_sub(READER_ONE, Ordering::Release);
}

/// Acquire write lock for exclusive access (only one writer, no readers).
/// Waits until no readers or writers are active.
fn lock_write(hdr: &HeaderV1) {
    let mut spins = 0u32;
    loop {
        let state = hdr.rwlock.load(Ordering::Acquire);
        // Check if any reader or writer is active
        if state == 0 {
            // Try to set writer flag (no need to wait, state was 0)
            if hdr
                .rwlock
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return;
            }
        }
        spins += 1;
        if spins % 10_000 == 0 {
            std::thread::sleep(std::time::Duration::from_millis(1));
        } else {
            std::hint::spin_loop();
        }
    }
}

/// Release write lock.
fn unlock_write(hdr: &HeaderV1) {
    hdr.rwlock.store(0, Ordering::Release);
}

// Legacy aliases for backward compatibility
#[allow(dead_code)]
fn lock_hdr(hdr: &HeaderV1) {
    lock_write(hdr);
}

#[allow(dead_code)]
fn unlock_hdr(hdr: &HeaderV1) {
    unlock_write(hdr);
}

pub fn shm_get(key: &str) -> Option<Vec<u8>> {
    let st = state()?;
    // SAFETY: init_seg0 ensures the header is valid. All pointer offsets
    // (index_off, arena_off) are set by init_seg0 and remain within the
    // segment. Access is protected by a read lock on the shared-memory header.
    unsafe {
        init_seg0(&st.seg0);
        let base0 = st.seg0.0.as_ptr() as *mut u8;
        let hdr = &*as_mut::<HeaderV1>(base0, 0);
        lock_read(hdr);  // Use read lock for concurrent reads
        let cap = hdr.index_cap as usize;
        let idx = &*as_slice_mut::<IndexEntryV1>(base0, hdr.index_off, cap);
        let h = hash_key64(key);
        let mut i = (h as usize) % cap;
        for _ in 0..cap {
            let e = idx[i];
            if e.key_hash == 0 {
                unlock_read(hdr);
                return None;
            }
            if e.key_hash == h && e.len > 0 {
                let seg = load_seg(e.seg)?;
                let start = e.off as usize;
                let end = start + e.len as usize;
                if end > seg.len() {
                    unlock_read(hdr);
                    return None;
                }
                let base = seg.as_ptr() as *mut u8;
                let bytes = std::slice::from_raw_parts(base.add(start), e.len as usize);
                let payload = unpack_record(bytes, key)?;
                unlock_read(hdr);
                return Some(payload.to_vec());
            }
            i = (i + 1) % cap;
        }
        unlock_read(hdr);
        None
    }
}

pub fn shm_put(key: &str, payload: &[u8]) -> bool {
    let st = match state() {
        Some(s) => s,
        None => return false,
    };
    // SAFETY: init_seg0 ensures the header is valid. All pointer arithmetic
    // stays within segment bounds. Access is serialized via a write lock
    // on the shared-memory header.
    unsafe {
        init_seg0(&st.seg0);
        let base0 = st.seg0.0.as_ptr() as *mut u8;
        let hdr = &mut *as_mut::<HeaderV1>(base0, 0);
        lock_write(hdr);  // Use write lock for exclusive access
        let cap = hdr.index_cap as usize;
        let idx = &mut *as_slice_mut::<IndexEntryV1>(base0, hdr.index_off, cap);
        let h = hash_key64(key);
        let mut i = (h as usize) % cap;
        for _ in 0..cap {
            if idx[i].key_hash == 0 || idx[i].key_hash == h {
                break;
            }
            i = (i + 1) % cap;
        }
        if idx[i].key_hash == 0 {
            idx[i].key_hash = h;
        }

        let rec = pack_record(key, payload);
        let rec_len = rec.len();
        let mut seg_id = hdr.current_seg;
        let mut off = hdr.current_off as usize;
        let mut seg = match load_seg(seg_id) {
            Some(s) => s,
            None => {
                unlock_write(hdr);
                return false;
            }
        };
        if off + rec_len > seg.len() {
            seg_id += 1;
            hdr.current_seg = seg_id;
            hdr.current_off = 0;
            off = 0;
            seg = match load_seg(seg_id) {
                Some(s) => s,
                None => {
                    unlock_write(hdr);
                    return false;
                }
            };
            if rec_len > seg.len() {
                unlock_write(hdr);
                return false;
            }
        }
        let base = seg.as_ptr() as *mut u8;
        std::ptr::copy_nonoverlapping(rec.as_ptr(), base.add(off), rec_len);
        hdr.current_off = (off + rec_len) as u32;
        idx[i].seg = seg_id;
        idx[i].off = off as u32;
        idx[i].len = rec_len as u32;
        unlock_write(hdr);
        true
    }
}

