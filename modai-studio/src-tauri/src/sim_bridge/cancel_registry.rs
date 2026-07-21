use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

static ACTIVE_CANCEL: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register(run_id: &str, flag: Arc<AtomicBool>) {
    if let Ok(mut g) = ACTIVE_CANCEL.lock() {
        g.insert(run_id.to_string(), flag);
    }
}

pub fn unregister(run_id: &str) {
    if let Ok(mut g) = ACTIVE_CANCEL.lock() {
        g.remove(run_id);
    }
}

pub fn request_cancel(run_id: &str) -> bool {
    if let Ok(g) = ACTIVE_CANCEL.lock() {
        if let Some(flag) = g.get(run_id) {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            return true;
        }
    }
    false
}
