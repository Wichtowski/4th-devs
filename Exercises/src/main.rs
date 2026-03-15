use std::env;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(target) = args.next() else {
        eprintln!("Usage: cargo run <exercise-directory> [-- <exercise-args...>]");
        eprintln!("Examples:");
        eprintln!("  cargo run S01E01");
        eprintln!("  cargo run S01E02");
        eprintln!("  cargo run S01E03");
        eprintln!("  cargo run S01E04");
        eprintln!("  cargo run S01E05");
        return ExitCode::from(2);
    };

    let binary = match normalize_target(&target).as_str() {
        "s01e01" => "s01e01",
        "s01e02" => "s01e02",
        "s01e03" => "s01e03",
        "s01e04" => "s01e04",
        "s01e05" => "s01e05",
        unknown => {
            eprintln!("Unknown exercise target: {unknown}");
            eprintln!("Available targets: S01E01, S01E02, S01E03, S01E04, S01E05");
            return ExitCode::from(2);
        }
    };

    let status = Command::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg(binary)
        .arg("--")
        .args(args)
        .status();

    match status {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!("Failed to launch {binary}: {error}");
            ExitCode::from(1)
        }
    }
}

fn normalize_target(target: &str) -> String {
    target
        .trim()
        .trim_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(target)
        .to_ascii_lowercase()
}
