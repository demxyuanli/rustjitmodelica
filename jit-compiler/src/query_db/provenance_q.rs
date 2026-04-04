//! Salsa query: provenance index derived from `flattened_model_q` (pre-inline flat; see module docs).

use std::sync::Arc;

use crate::analysis::provenance_index_from_flat_model;

use super::{ProvenanceIndexResPtr, ProvenanceQResult, QueryDb};

pub(super) fn provenance_index_q(db: &dyn QueryDb, model_name: String) -> ProvenanceIndexResPtr {
    let flat_r = db.flattened_model_q(model_name.clone());
    let inner = flat_r.0.as_ref();
    if let Some(e) = &inner.err {
        return ProvenanceIndexResPtr(Arc::new(ProvenanceQResult {
            index: None,
            err: Some(e.clone()),
        }));
    }
    let Some(flat_arc) = &inner.flat else {
        return ProvenanceIndexResPtr(Arc::new(ProvenanceQResult {
            index: None,
            err: Some("flattened_model_q missing flat".to_string()),
        }));
    };
    let st = db.source_text(model_name);
    let path_hint = if st.path.is_empty() {
        None
    } else {
        Some(st.path.as_str())
    };
    let idx = Arc::new(provenance_index_from_flat_model(flat_arc.as_ref(), path_hint));
    ProvenanceIndexResPtr(Arc::new(ProvenanceQResult {
        index: Some(idx),
        err: None,
    }))
}
