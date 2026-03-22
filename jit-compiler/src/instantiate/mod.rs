//! Instantiate-oriented helpers: subtype / `constrainedby` via extends closure.
//!
//! Used by default when a `ModelLoader` is available; disable with CLI `--coarse-constrainedby`.
//! Tier S/O acceptance workflow is documented in [`crate::flatten::flat_snapshot`].
//! Full `inner`/`outer` instance lookup is still handled in the flatten pipeline; this module does not replace it.

mod subtype;

pub use subtype::constrainedby_holds_extends;
