mod apps;
mod dev;
mod project;

use std::{env, ffi::OsString, process::ExitCode};

const HELP: &str = "\
Ruvoraq — Experiment 007

Usage: ruvoraq <COMMAND>
       ruvoraq [--help | --version]

Commands:
  new <project-name>  Create a three-file Rust HTTP application
  add app <name>     Add an optional routes/models/services module
  dev                 Build and run the current Ruvoraq project with Cargo

Options:
  -h, --help           Print help
  -V, --version        Print version

Use 'ruvoraq new --help' for project-name rules.
";

fn run(args: &[OsString]) -> Result<ExitCode, String> {
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
            println!("Next: cd {name} && ruvoraq dev");
        }
        ["new"] => return Err("missing project name; usage: ruvoraq new <project-name>".into()),
        ["new", ..] => {
            return Err("expected one project name; usage: ruvoraq new <project-name>".into());
        }
        ["add", "--help" | "-h"] | ["add", "app", "--help" | "-h"] => println!(
            "Usage: ruvoraq add app <name>\n\nRun from the project's root. Names use lowercase ASCII letters, digits and\nunderscores, starting with a letter or underscore. Existing apps are refused.\nCreates src/apps/<name>/ with routes.rs, models.rs, services.rs and mod.rs.\nWires modules through settings.rs; main.rs stays unchanged."
        ),
        ["add", "app", name] => {
            let cwd = env::current_dir()
                .map_err(|error| format!("cannot read current directory: {error}"))?;
            let target = apps::add(&cwd, name)?;
            println!("Added app at {}", target.display());
            println!("Route: GET /{name}");
            println!("Next: ruvoraq dev");
        }
        ["add", ..] => return Err("usage: ruvoraq add app <name>".into()),
        ["dev", "--help" | "-h"] => println!(
            "Usage: ruvoraq dev\n\n\
             Run from the generated project's root directory. Builds and runs the\n\
             application using cargo run with inherited terminal output and input.\n\
             Press Ctrl+C to stop. File watching is not implemented yet."
        ),
        ["dev"] => {
            let cwd = env::current_dir()
                .map_err(|error| format!("cannot read current directory: {error}"))?;
            return dev::run(&cwd);
        }
        ["dev", ..] => return Err("unexpected arguments; usage: ruvoraq dev".into()),
        _ => return Err("unknown command or option; run 'ruvoraq --help' for usage".into()),
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match run(&env::args_os().skip(1).collect::<Vec<_>>()) {
        Ok(status) => status,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
