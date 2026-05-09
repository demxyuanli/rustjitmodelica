use crate::jit::context::TranslationContext;
use std::sync::OnceLock;

pub(super) fn warn_stream_semantics_once(kind: &'static str) {
    static INSTREAM_WARNED: OnceLock<()> = OnceLock::new();
    static ACTUAL_WARNED: OnceLock<()> = OnceLock::new();
    static PEER_WARNED: OnceLock<()> = OnceLock::new();
    match kind {
        "inStream" => {
            let _ = INSTREAM_WARNED.get_or_init(|| {
                eprintln!("[stream] inStream(): using MSL 3.1 flow-weighted mixing formula")
            });
        }
        "actualStream" => {
            let _ = ACTUAL_WARNED.get_or_init(|| {
                eprintln!("[stream] actualStream(): using MSL 3.1 semantics (positive flow -> self, negative -> inStream)")
            });
        }
        "peerMissing" => {
            let _ = PEER_WARNED.get_or_init(|| {
                eprintln!("[stream] stream peer/flow mapping not found, fallback to passthrough for this model path")
            });
        }
        _ => {}
    }
}

fn stream_flow_name_heuristic(stream_name: &str) -> Option<String> {
    stream_name
        .strip_suffix("_h_outflow")
        .map(|prefix| format!("{}_m_flow", prefix))
}

pub(super) fn stream_flow_name_for(ctx: &TranslationContext, stream_name: &str) -> Option<String> {
    ctx.stream_flow_map
        .get(stream_name)
        .cloned()
        .or_else(|| stream_flow_name_heuristic(stream_name))
}

pub(super) fn stream_peer_names(ctx: &TranslationContext, stream_name: &str) -> Vec<String> {
    ctx.stream_connection_set
        .get(stream_name)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn value_name_exists(ctx: &TranslationContext, name: &str) -> bool {
    ctx.state_index(name).is_some()
        || ctx.discrete_index(name).is_some()
        || ctx.output_index(name).is_some()
        || ctx.param_index(name).is_some()
        || ctx.stack_slots.contains_key(name)
        || ctx.var_map.contains_key(name)
}
