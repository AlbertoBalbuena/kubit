//! Project automation: cargo xtask ci|layers|wasm|deny.

mod layers;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

const USAGE: &str = "usage: cargo xtask <command>

commands:
  ci       fmt --check, clippy, test, layers, wasm, deny (stops at the first failure)
  layers   check the dependency graph against layers.toml
  wasm     build every `wasm = true` crate for wasm32-unknown-unknown
  deny     cargo deny check (licenses, bans, advisories, sources)";

type Result<T> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let arg = std::env::args().nth(1);
    let result = match arg.as_deref() {
        Some("ci") => ci(),
        Some("layers") => layers_cmd(),
        Some("wasm") => wasm(),
        Some("deny") => deny(),
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    // xtask lives in <root>/xtask.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn cargo() -> Command {
    let mut cmd = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cmd.current_dir(root());
    cmd
}

fn run(mut cmd: Command) -> Result<()> {
    let shown = format!("{cmd:?}").replace('"', "");
    eprintln!("$ {shown}");
    let status = cmd
        .status()
        .map_err(|e| format!("cannot run `{shown}`: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{shown}` failed ({status})"))
    }
}

fn load() -> Result<(layers::LayerMap, layers::Graph)> {
    let text = std::fs::read_to_string(root().join("layers.toml"))
        .map_err(|e| format!("cannot read layers.toml: {e}"))?;
    let map = layers::LayerMap::parse(&text)?;
    let metadata = cargo_metadata::MetadataCommand::new()
        .current_dir(root())
        .exec()
        .map_err(|e| format!("cargo metadata failed: {e}"))?;
    let graph = layers::Graph::from_metadata(&metadata)?;
    Ok((map, graph))
}

fn layers_cmd() -> Result<()> {
    let (map, graph) = load()?;
    let report = layers::check(&map, &graph);
    for w in &report.warnings {
        eprintln!("warning: {w}");
    }
    if report.violations.is_empty() {
        println!("layers: ok ({} crates checked)", report.checked);
        Ok(())
    } else {
        for v in &report.violations {
            eprintln!("violation: {v}");
        }
        Err(format!("{} layer violation(s)", report.violations.len()))
    }
}

fn wasm() -> Result<()> {
    let (map, graph) = load()?;
    let crates = layers::wasm_crates(&map, &graph);
    if crates.is_empty() {
        return Err("no crate has `wasm = true` in layers.toml".into());
    }
    let mut cmd = cargo();
    cmd.args(["build", "--target", "wasm32-unknown-unknown"]);
    for c in &crates {
        cmd.args(["-p", c]);
    }
    run(cmd)
}

fn deny() -> Result<()> {
    let mut probe = cargo();
    probe.args(["deny", "--version"]);
    let found = probe.output().map(|o| o.status.success()).unwrap_or(false);
    if !found {
        return Err("cargo-deny is not installed. Install it with \
                    `cargo install --locked cargo-deny` (or `cargo binstall cargo-deny`)."
            .into());
    }
    let mut cmd = cargo();
    cmd.args(["deny", "--all-features", "check"]);
    run(cmd)
}

fn ci() -> Result<()> {
    type Step = (&'static str, fn() -> Result<()>);
    let steps: [Step; 6] = [
        ("fmt", || run_args(&["fmt", "--all", "--check"])),
        ("clippy", || {
            run_args(&[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ])
        }),
        ("test", || run_args(&["test", "--workspace"])),
        ("layers", layers_cmd),
        ("wasm", wasm),
        ("deny", deny),
    ];
    let mut summary: Vec<String> = Vec::new();
    for (name, step) in steps {
        eprintln!("\n=== {name} ===");
        let start = Instant::now();
        let outcome = step();
        let secs = start.elapsed().as_secs_f32();
        match outcome {
            Ok(()) => summary.push(format!("  ok    {name} ({secs:.1}s)")),
            Err(e) => {
                summary.push(format!("  FAIL  {name} ({secs:.1}s)"));
                eprintln!("\nci summary:\n{}", summary.join("\n"));
                return Err(format!("{name}: {e}"));
            }
        }
    }
    println!("\nci summary:\n{}\nci: all green", summary.join("\n"));
    Ok(())
}

fn run_args(args: &[&str]) -> Result<()> {
    let mut cmd = cargo();
    cmd.args(args);
    run(cmd)
}
