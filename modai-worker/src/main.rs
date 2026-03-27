use modai_protocol::RegressionPlanRequest;
use std::path::PathBuf;

fn print_usage() {
    println!("modai-worker usage:");
    println!("  modai-worker plan <repo_root> <request_json>");
    println!("  modai-worker run <repo_root> <workspace_id>");
    println!("  modai-worker state <repo_root> <workspace_id>");
    println!("  modai-worker list <repo_root>");
}

fn execute(args: &[String]) -> Result<String, String> {
    if args.len() < 3 {
        return Err("insufficient arguments".to_string());
    }
    let cmd = args[1].as_str();
    let repo_root = PathBuf::from(&args[2]);
    match cmd {
        "plan" => {
            if args.len() < 4 {
                Err("missing request_json".to_string())
            } else {
                let raw_req = if std::path::Path::new(&args[3]).is_file() {
                    std::fs::read_to_string(&args[3]).map_err(|e| e.to_string())?
                } else {
                    args[3].clone()
                };
                let req: RegressionPlanRequest =
                    serde_json::from_str(&raw_req).map_err(|e| e.to_string())?;
                let state = modai_worker::create_workspace(&repo_root, req)?;
                serde_json::to_string(&state).map_err(|e| e.to_string())
            }
        }
        "run" => {
            if args.len() < 4 {
                Err("missing workspace_id".to_string())
            } else {
                let state = modai_worker::run_workspace(&repo_root, &args[3])?;
                serde_json::to_string(&state).map_err(|e| e.to_string())
            }
        }
        "state" => {
            if args.len() < 4 {
                Err("missing workspace_id".to_string())
            } else {
                let state = modai_worker::get_workspace_state(&repo_root, &args[3])?;
                serde_json::to_string(&state).map_err(|e| e.to_string())
            }
        }
        "list" => {
            let list = modai_worker::list_workspaces(&repo_root)?;
            serde_json::to_string(&list).map_err(|e| e.to_string())
        }
        _ => Err(format!("unknown command: {cmd}")),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        print_usage();
        std::process::exit(2);
    }
    let result = execute(&args);
    match result {
        Ok(text) => {
            println!("{text}");
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
