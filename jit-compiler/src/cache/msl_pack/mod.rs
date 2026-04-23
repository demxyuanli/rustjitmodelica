//! Pre-baked Modelica Standard Library cache packs (parse / model_ast / flat tiers in SQLite).

mod builder;
pub mod context;
mod hotness;
pub mod hydrate;
pub mod leaves;
pub mod manifest;
pub mod tree_digest;
pub mod version;

pub use builder::bake_msl_pack;
pub use hotness::on_flatten_success;
pub use hydrate::on_msl_library_path_added;
