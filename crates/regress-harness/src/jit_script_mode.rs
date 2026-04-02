use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn discover_rustmodlica_exe(repo_root: &Path) -> Option<PathBuf> {
    let candidates = [
        repo_root.join("jit-compiler/target_regression/release/rustmodlica.exe"),
        repo_root.join("jit-compiler/target_regression/debug/rustmodlica.exe"),
        repo_root.join("target/release/rustmodlica.exe"),
        repo_root.join("target/debug/rustmodlica.exe"),
        repo_root.join("jit-compiler/target_regression/release/rustmodlica"),
        repo_root.join("jit-compiler/target_regression/debug/rustmodlica"),
        repo_root.join("target/release/rustmodlica"),
        repo_root.join("target/debug/rustmodlica"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn script_text_for_case(case: &str) -> Result<&'static str> {
    match case {
        "init_dummy" => Ok("load TestLib/InitDummy\nsimulate\nquit\n"),
        "init_with_param_setparam" => Ok("load TestLib/InitWithParam\nsetParameter a 5\nsimulate\nquit\n"),
        "multi_model_use" => Ok(
            "load TestLib/InitWithParam\nload TestLib/SimpleTest\nuse TestLib/InitWithParam\nsetParameter a 5\nsimulate\nquit\n",
        ),
        "set_start_value" => Ok("load TestLib/InitWithParam\nsetStartValue x 1.0\nsimulate\nquit\n"),
        "get_parameter" => Ok("load TestLib/InitWithParam\nsetParameter a 5\ngetParameter a\nsimulate\nquit\n"),
        "set_stop_time" => Ok("load TestLib/InitWithParam\nsetStopTime 2.0\nsimulate\nquit\n"),
        "set_tolerance" => Ok("load TestLib/InitWithParam\nsetTolerance 1e-6 1e-5\nsimulate\nquit\n"),
        "save_result" => Ok(
            "load TestLib/InitWithParam\nsetResultFile build_regress_script_result.csv\nsimulate\nquit\n",
        ),
        "plot" => Ok("load TestLib/InitWithParam\nplot x\nsimulate\nquit\n"),
        "eval" => Ok("load TestLib/InitWithParam\nsetParameter a 3\nsimulate\neval x\neval a + 1\nquit\n"),
        "load_class" => Ok("loadClass TestLib/InitWithParam\nsimulate\nquit\n"),
        "switch_model" => Ok(
            "load TestLib/InitWithParam\nload TestLib/SimpleTest\nswitchModel TestLib/InitWithParam\nsetParameter a 4\nsimulate\nquit\n",
        ),
        _ => bail!("unknown script-mode case: {case}"),
    }
}

pub fn run_script_mode_case(repo_root: &Path, case: &str) -> Result<i32> {
    let exe = discover_rustmodlica_exe(repo_root)
        .ok_or_else(|| anyhow::anyhow!("rustmodlica binary not found (auto discovery failed)"))?;
    let script = script_text_for_case(case)?;

    let jit = repo_root.join("jit-compiler");
    if !jit.is_dir() {
        bail!("missing directory: {}", jit.display());
    }

    let mut cmd = Command::new(&exe);
    cmd.current_dir(&jit);
    cmd.arg("--script=-");
    // Keep defaults aligned with phase1 config.
    cmd.arg("--solver=rk4");
    cmd.arg("--t-end=10.0");
    cmd.arg("--dt=0.01");
    // The repo's script_mode *.txt uses the legacy line-based syntax.
    cmd.env("RUSTMODLICA_SCRIPT_ENGINE", "legacy");
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().with_context(|| format!("spawn {}", exe.display()))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(script.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    let code = out.status.code().unwrap_or(1);
    if code != 0 {
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        return Err(anyhow::anyhow!(text));
    }
    Ok(code)
}

