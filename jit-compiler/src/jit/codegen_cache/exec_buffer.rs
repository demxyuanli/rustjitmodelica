//! Executable anonymous RWX/RX buffer for cached JIT bytes.

/// RWX anonymous mapping: file-backed `mmap` is not executable on Windows (and often not on Unix).
pub(crate) struct ExecCodeBuffer {
    ptr: *mut u8,
    len: usize,
}

impl ExecCodeBuffer {
    pub(crate) fn alloc_rw(len: usize) -> Option<Self> {
        if len == 0 {
            return None;
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Memory::{
                VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
            };
            let ptr = unsafe {
                VirtualAlloc(
                    std::ptr::null(),
                    len,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_READWRITE,
                )
            };
            if ptr.is_null() {
                return None;
            }
            return Some(Self {
                ptr: ptr as *mut u8,
                len,
            });
        }
        #[cfg(unix)]
        {
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    len,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            if ptr == libc::MAP_FAILED {
                return None;
            }
            let ptr = ptr as *mut u8;
            return Some(Self { ptr, len });
        }
        #[cfg(not(any(windows, unix)))]
        {
            None
        }
    }

    #[cfg_attr(windows, allow(dead_code))]
    pub(crate) fn copy_from_bytes(bytes: &[u8]) -> Option<Self> {
        let exec = Self::alloc_rw(bytes.len())?;
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), exec.as_mut_ptr(), bytes.len());
        }
        if !exec.make_rx() {
            return None;
        }
        Some(exec)
    }

    pub(crate) fn make_rx(&self) -> bool {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READ};
            let mut old_protect = 0u32;
            unsafe { VirtualProtect(self.ptr as *mut _, self.len, PAGE_EXECUTE_READ, &mut old_protect) != 0 }
        }
        #[cfg(unix)]
        {
            unsafe {
                libc::mprotect(
                    self.ptr as *mut libc::c_void,
                    self.len,
                    libc::PROT_READ | libc::PROT_EXEC,
                ) == 0
            }
        }
        #[cfg(not(any(windows, unix)))]
        {
            false
        }
    }

    pub(crate) fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr
    }

    pub(crate) fn as_ptr(&self) -> *const u8 {
        self.ptr
    }
}

impl Drop for ExecCodeBuffer {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Memory::{VirtualFree, MEM_RELEASE};
            if !self.ptr.is_null() && self.len > 0 {
                unsafe {
                    VirtualFree(self.ptr as *mut _, 0, MEM_RELEASE);
                }
            }
        }
        #[cfg(unix)]
        {
            if !self.ptr.is_null() && self.len > 0 {
                unsafe {
                    libc::munmap(self.ptr as *mut libc::c_void, self.len);
                }
            }
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = (self.ptr, self.len);
        }
    }
}
