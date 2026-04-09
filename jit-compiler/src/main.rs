mod cli;

use std::env;
use std::process::ExitCode;
use std::thread;

use rustmodlica::error;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    const STACK_SIZE: usize = 8 * 1024 * 1024;
    let child = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || cli::run(args))
        .map_err(|e| error::AppError::ThreadSpawn(e.to_string()));
    let child = match child {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(1);
        }
    };
    match child.join() {
        Err(_) => {
            eprintln!("{}", error::AppError::ThreadPanic);
            ExitCode::from(1)
        }
        Ok(Err(e)) => {
            eprintln!("{}", e);
            ExitCode::from(1)
        }
        Ok(Ok(())) => ExitCode::from(0),
    }
}
