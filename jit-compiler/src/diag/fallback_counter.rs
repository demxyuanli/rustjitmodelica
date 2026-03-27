use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct FallbackCounterSnapshot {
    pub jit_builtin: u64,
    pub jit_variable: u64,
    pub jit_derivative: u64,
    pub jit_equation_skip: u64,
    pub jit_multi_assign: u64,
    pub newton_init_accept: u64,
    pub newton_event_accept: u64,
    pub clock_degrade: u64,
}

static JIT_BUILTIN: AtomicU64 = AtomicU64::new(0);
static JIT_VARIABLE: AtomicU64 = AtomicU64::new(0);
static JIT_DERIVATIVE: AtomicU64 = AtomicU64::new(0);
static JIT_EQUATION_SKIP: AtomicU64 = AtomicU64::new(0);
static JIT_MULTI_ASSIGN: AtomicU64 = AtomicU64::new(0);
static NEWTON_INIT_ACCEPT: AtomicU64 = AtomicU64::new(0);
static NEWTON_EVENT_ACCEPT: AtomicU64 = AtomicU64::new(0);
static CLOCK_DEGRADE: AtomicU64 = AtomicU64::new(0);

#[inline]
pub fn inc_jit_builtin() {
    JIT_BUILTIN.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn inc_jit_variable() {
    JIT_VARIABLE.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn inc_jit_derivative() {
    JIT_DERIVATIVE.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn inc_jit_equation_skip() {
    JIT_EQUATION_SKIP.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn inc_jit_multi_assign() {
    JIT_MULTI_ASSIGN.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn inc_newton_init_accept() {
    NEWTON_INIT_ACCEPT.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn inc_newton_event_accept() {
    NEWTON_EVENT_ACCEPT.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn inc_clock_degrade() {
    CLOCK_DEGRADE.fetch_add(1, Ordering::Relaxed);
}

pub fn snapshot() -> FallbackCounterSnapshot {
    FallbackCounterSnapshot {
        jit_builtin: JIT_BUILTIN.load(Ordering::Relaxed),
        jit_variable: JIT_VARIABLE.load(Ordering::Relaxed),
        jit_derivative: JIT_DERIVATIVE.load(Ordering::Relaxed),
        jit_equation_skip: JIT_EQUATION_SKIP.load(Ordering::Relaxed),
        jit_multi_assign: JIT_MULTI_ASSIGN.load(Ordering::Relaxed),
        newton_init_accept: NEWTON_INIT_ACCEPT.load(Ordering::Relaxed),
        newton_event_accept: NEWTON_EVENT_ACCEPT.load(Ordering::Relaxed),
        clock_degrade: CLOCK_DEGRADE.load(Ordering::Relaxed),
    }
}

pub fn total(snapshot: &FallbackCounterSnapshot) -> u64 {
    snapshot.jit_builtin
        + snapshot.jit_variable
        + snapshot.jit_derivative
        + snapshot.jit_equation_skip
        + snapshot.jit_multi_assign
        + snapshot.newton_init_accept
        + snapshot.newton_event_accept
        + snapshot.clock_degrade
}
