//! FMU import: parse .fmu (ZIP) modelDescription.xml, catalog node, FMI2 CS host cosim.

mod cosim;
mod host;
mod parse;

pub use cosim::{any_fmu, simulate_with_fmu};
pub use parse::{import_fmu, list_imported_fmus, FmuImportResult};
