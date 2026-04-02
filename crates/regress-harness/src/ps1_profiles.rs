use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

fn snake_case_segment(s: &str) -> String {
    let mut out = String::new();
    let mut prev_lower_or_digit = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_lower_or_digit {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            if !out.ends_with('_') {
                out.push('_');
            }
            prev_lower_or_digit = false;
        }
    }
    out.trim_matches('_').to_string()
}

fn case_id_from_target(target: &str) -> String {
    let parts = target
        .split(|c| c == '/' || c == '.')
        .filter(|p| !p.trim().is_empty())
        .collect::<Vec<_>>();
    let mut chunks = Vec::new();
    for p in parts {
        if p == "TestLib" {
            chunks.push("testlib".to_string());
        } else if p == "ModelicaTest" {
            chunks.push("modelicatest".to_string());
        } else {
            chunks.push(snake_case_segment(p));
        }
    }
    chunks.join("_")
}

fn expect_obj(expect: &str) -> Value {
    if expect.eq_ignore_ascii_case("pass") {
        json!({"kind":"exit_zero"})
    } else {
        json!({"kind":"non_zero"})
    }
}

fn parse_case_extra_args(text: &str) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::<String, Vec<String>>::new();
    let block_re =
        Regex::new(r"(?ms)\$caseExtraArgs\s*=\s*@\{(.*?)^\}").expect("regex");
    let Some(block) = block_re.captures(text).and_then(|c| c.get(1)).map(|m| m.as_str()) else {
        return out;
    };
    let arg_re = Regex::new(r#""(?P<path>[^"]+)"\s*=\s*@\((?P<args>[^)]*)\)"#).expect("regex");
    let quoted_re = Regex::new(r#""([^"]*)""#).expect("regex");
    for cap in arg_re.captures_iter(block) {
        let path = cap.name("path").map(|m| m.as_str()).unwrap_or("").to_string();
        let args_str = cap.name("args").map(|m| m.as_str()).unwrap_or("");
        let mut args = Vec::new();
        for q in quoted_re.captures_iter(args_str) {
            args.push(q.get(1).map(|m| m.as_str()).unwrap_or("").to_string());
        }
        if !path.is_empty() && !args.is_empty() {
            out.insert(path, args);
        }
    }
    out
}

fn parse_run_regression_cases(text: &str) -> Result<Vec<(String, String)>> {
    let rx = Regex::new(r#"@\("([^"]+)",\s*"(pass|fail)"\)"#).expect("regex");
    let mut out = Vec::new();
    for m in rx.captures_iter(text) {
        let target = m.get(1).context("target")?.as_str().to_string();
        let expect = m.get(2).context("expect")?.as_str().to_string();
        out.push((target, expect));
    }
    if out.is_empty() {
        bail!("no case rows matched in run_regression.ps1");
    }
    Ok(out)
}

fn parse_mos_cases(text: &str) -> Result<Vec<String>> {
    let rx = Regex::new(r#""scripts/[^"]+\.mos""#).expect("regex");
    let mut out = Vec::new();
    for m in rx.find_iter(text) {
        out.push(m.as_str().trim_matches('"').to_string());
    }
    if out.is_empty() {
        bail!("no MOS scripts matched in run_mos_regression.ps1");
    }
    Ok(out)
}

fn write_json(path: &Path, v: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(v)?)?;
    Ok(())
}

pub fn generate_testlib_from_ps1(repo_root: &Path) -> Result<PathBuf> {
    let run_reg = repo_root.join("run_regression.ps1");
    let text = fs::read_to_string(&run_reg)
        .with_context(|| format!("read {}", run_reg.display()))?;
    let extra_args = parse_case_extra_args(&text);
    let cases = parse_run_regression_cases(&text)?;

    let mut seen = HashSet::<String>::new();
    let mut case_objs = Vec::<Value>::new();
    for (target, expect) in cases {
        let id = case_id_from_target(&target);
        if !seen.insert(id.clone()) {
            bail!("duplicate case id: {id} (target={target})");
        }
        let mut tags = vec!["testlib".to_string(), "core".to_string()];
        if expect.eq_ignore_ascii_case("fail") {
            tags.push("negative".to_string());
        }
        let mut obj = Map::<String, Value>::new();
        obj.insert("id".to_string(), Value::String(id));
        obj.insert("kind".to_string(), Value::String("model".to_string()));
        obj.insert("target".to_string(), Value::String(target.clone()));
        obj.insert(
            "tags".to_string(),
            Value::Array(tags.into_iter().map(Value::String).collect()),
        );
        obj.insert("expect".to_string(), expect_obj(&expect));
        if let Some(extra) = extra_args.get(&target) {
            obj.insert(
                "extra_rust_args".to_string(),
                Value::Array(extra.iter().cloned().map(Value::String).collect()),
            );
        }
        case_objs.push(Value::Object(obj));
    }

    let doc = json!({
        "version": 1,
        "defaults": {
            "repo_root": ".",
            "rustmodlica_exe": "jit-compiler/target_regression/release/rustmodlica.exe",
            "working_dir": "jit-compiler",
            "cargo_exe": "cargo",
            "solver": "rk4",
            "t_end": 10.0,
            "dt": 0.01,
            "regression_data_root": "build/regression_data_testlib"
        },
        "execution": { "workers": 4, "fail_fast": false },
        "incremental": { "baseline_path": null, "strategy": "none" },
        "tiers": {
            "core": { "include_tags": ["core"] },
            "negative": { "include_tags": ["negative"] }
        },
        "cases": case_objs
    });

    let out = repo_root
        .join("build")
        .join("regress_harness_profiles")
        .join("testlib_from_run_regression.ps1.json");
    write_json(&out, &doc)?;
    Ok(out)
}

pub fn generate_mos_from_ps1(repo_root: &Path) -> Result<PathBuf> {
    let mos_reg = repo_root.join("jit-compiler").join("scripts").join("run_mos_regression.ps1");
    let text = fs::read_to_string(&mos_reg)
        .with_context(|| format!("read {}", mos_reg.display()))?;
    let mos_paths = parse_mos_cases(&text)?;

    let mut seen = HashSet::<String>::new();
    let mut case_objs = Vec::<Value>::new();
    for rel in mos_paths {
        let p = PathBuf::from(&rel);
        let base_name = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("mos_case")
            .to_string();
        let id = format!("mos_{}", snake_case_segment(base_name.as_str()));
        if !seen.insert(id.clone()) {
            continue;
        }
        case_objs.push(json!({
            "id": id,
            "kind": "mos",
            "target": rel,
            "tags": ["mos","core"],
            "expect": { "kind": "exit_zero" }
        }));
    }

    let doc = json!({
        "version": 1,
        "defaults": {
            "repo_root": ".",
            "rustmodlica_exe": "jit-compiler/target_regression/release/rustmodlica.exe",
            "working_dir": "jit-compiler",
            "cargo_exe": "cargo",
            "cargo_run_prefix": [],
            "solver": "rk4",
            "t_end": 10.0,
            "dt": 0.01,
            "regression_data_root": "build/regression_data_mos"
        },
        "execution": { "workers": 1, "fail_fast": false },
        "incremental": { "baseline_path": null, "strategy": "none" },
        "tiers": { "mos": { "include_tags": ["mos"] } },
        "cases": case_objs
    });

    let out = repo_root
        .join("build")
        .join("regress_harness_profiles")
        .join("mos_from_run_mos_regression.ps1.json");
    write_json(&out, &doc)?;
    Ok(out)
}

pub fn ensure_profile(repo_root: &Path, name: &str) -> Result<Option<PathBuf>> {
    match name {
        "ps1_testlib" => Ok(Some(generate_testlib_from_ps1(repo_root)?)),
        "ps1_mos" => Ok(Some(generate_mos_from_ps1(repo_root)?)),
        _ => Ok(None),
    }
}

