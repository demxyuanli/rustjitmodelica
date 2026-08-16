use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct RunFlags {
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    step_once: Arc<AtomicBool>,
}

static ACTIVE: Lazy<Mutex<HashMap<String, RunFlags>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register(
    run_id: &str,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    step_once: Arc<AtomicBool>,
) {
    if let Ok(mut g) = ACTIVE.lock() {
        g.insert(
            run_id.to_string(),
            RunFlags {
                cancel,
                pause,
                step_once,
            },
        );
    }
}

pub fn unregister(run_id: &str) {
    if let Ok(mut g) = ACTIVE.lock() {
        g.remove(run_id);
    }
}

pub fn request_cancel(run_id: &str) -> bool {
    if let Ok(g) = ACTIVE.lock() {
        if let Some(f) = g.get(run_id) {
            f.cancel.store(true, Ordering::SeqCst);
            f.pause.store(false, Ordering::SeqCst);
            f.step_once.store(true, Ordering::SeqCst);
            return true;
        }
    }
    false
}

pub fn request_pause(run_id: &str) -> bool {
    if let Ok(g) = ACTIVE.lock() {
        if let Some(f) = g.get(run_id) {
            f.pause.store(true, Ordering::SeqCst);
            return true;
        }
    }
    false
}

pub fn request_resume(run_id: &str) -> bool {
    if let Ok(g) = ACTIVE.lock() {
        if let Some(f) = g.get(run_id) {
            f.step_once.store(false, Ordering::SeqCst);
            f.pause.store(false, Ordering::SeqCst);
            return true;
        }
    }
    false
}

pub fn request_step(run_id: &str) -> bool {
    if let Ok(g) = ACTIVE.lock() {
        if let Some(f) = g.get(run_id) {
            f.step_once.store(true, Ordering::SeqCst);
            f.pause.store(true, Ordering::SeqCst);
            return true;
        }
    }
    false
}

pub fn is_paused(run_id: &str) -> bool {
    if let Ok(g) = ACTIVE.lock() {
        if let Some(f) = g.get(run_id) {
            return f.pause.load(Ordering::SeqCst);
        }
    }
    false
}
