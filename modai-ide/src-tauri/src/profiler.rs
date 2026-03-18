use std::time::Instant;

#[allow(dead_code)]
pub struct ScopedTimer {
    name: &'static str,
    start: Instant,
}

impl ScopedTimer {
    #[inline]
    #[allow(dead_code)]
    pub fn new(name: &'static str) -> Option<Self> {
        #[cfg(debug_assertions)]
        {
            Some(Self {
                name,
                start: Instant::now(),
            })
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = name;
            None
        }
    }
}

impl Drop for ScopedTimer {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            let elapsed = self.start.elapsed();
            eprintln!(
                "[modai-prof] {} took {} ms",
                self.name,
                elapsed.as_millis()
            );
        }
    }
}

