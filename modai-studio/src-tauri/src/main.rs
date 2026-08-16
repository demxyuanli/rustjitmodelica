#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    if let Some(code) = modai_studio_lib::try_run_catalog_gate_cli() {
        return code;
    }
    if let Some(code) = modai_studio_lib::try_run_emit_golden_cli() {
        return code;
    }
    if let Some(code) = modai_studio_lib::try_run_smoke_cli() {
        return code;
    }
    modai_studio_lib::run();
    std::process::ExitCode::SUCCESS
}
