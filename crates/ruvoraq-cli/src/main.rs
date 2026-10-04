mod project;

use std::{env, ffi::OsString, process::ExitCode};

const HELP: &str = "\
Ruvoraq — Experiment 001

Usage: ruvoraq <COMMAND>
       ruvoraq [--help | --version]

Commands:
  new <project-name>  Create a minimal Rust project in the current directory

Options:
  -h, --help         Print help
  -V, --version      Print version

Use 'ruvoraq new --help' for project-name rules.
";

fn run(args: &[OsString]) -> Result<(), String> {
    let args: Vec<&str> = args
        .iter()
        .map(|arg| arg.to_str().ok_or("arguments must be valid UTF-8"))
        .collect::<Result<_, _>>()?;

    match args.as_slice() {
        [] | ["--help" | "-h"] => print!("{HELP}"),
        ["--version" | "-V"] => println!("ruvoraq {}", env!("CARGO_PKG_VERSION")),
        ["new", "--help" | "-h"] => println!(
            "Usage: ruvoraq new <project-name>\n\n\
             Names must start with an ASCII letter or underscore, contain only\n\
             ASCII letters, digits, hyphens or underscores, and be at most 64 bytes.\n\
             Rust keywords, Cargo build directory names and Windows device names\n\
             are reserved. Paths are not accepted. Existing directories must be empty."
        ),
        ["new", name] => {
            let cwd = env::current_dir()
                .map_err(|error| format!("cannot read current directory: {error}"))?;
            let target = project::create(&cwd, name)?;
            println!("Created project at {}", target.display());
            println!("Next: cd {name} && cargo run");
        }
        ["new"] => return Err("missing project name; usage: ruvoraq new <project-name>".into()),
        ["new", ..] => {
            return Err("expected one project name; usage: ruvoraq new <project-name>".into());
        }
        _ => return Err("unknown command or option; run 'ruvoraq --help' for usage".into()),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(&env::args_os().skip(1).collect::<Vec<_>>()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
