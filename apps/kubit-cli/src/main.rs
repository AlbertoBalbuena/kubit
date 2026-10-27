//! Kubit headless CLI: run, export, mcp, snapshot.
//!
//! Every subcommand goes through the same command registry as the UI and MCP;
//! nothing here opens a dialog.

mod backend;
mod snapshot;

use clap::{Parser, Subcommand};
use kubit_engine::{CommandError, Session};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "kubit-cli", version, about = "Kubit headless CLI")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
    /// Print command results as JSON lines instead of text.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a JSON script of commands, then write every `--out` file.
    Run {
        /// Script: `{"commands": [{"command": "solid.box", "params": {...}}, ...]}`.
        script: PathBuf,
        /// Output files by extension: .step/.stp, .stl, .kubit, .json. Repeatable.
        #[arg(long = "out")]
        out: Vec<PathBuf>,
    },
    /// Open a document and export it.
    Export {
        /// A .kubit or .json document.
        document: PathBuf,
        /// Output files by extension (see `run`). Repeatable.
        #[arg(long = "out", required = true)]
        out: Vec<PathBuf>,
    },
    /// Run one command: `kubit-cli exec solid.box '{"size":[10,10,10]}'`.
    Exec {
        /// Command id.
        command: String,
        /// JSON parameters.
        #[arg(default_value = "{}")]
        params: String,
        /// Document to open first.
        #[arg(long)]
        open: Option<PathBuf>,
        /// Save the document afterwards.
        #[arg(long)]
        save: Option<PathBuf>,
    },
    /// List every registered command with its description.
    Commands,
    /// Serve the command registry as MCP tools over stdin/stdout.
    Mcp {
        /// Hide and refuse commands that change the document.
        #[arg(long)]
        read_only: bool,
        /// Expose commands that read or write files on this machine.
        #[arg(long)]
        allow_files: bool,
    },
    /// Render a document or script offscreen to a PNG (no window).
    Snapshot {
        /// A .kubit/.json document, or a JSON script like `run` takes.
        input: PathBuf,
        /// PNG to write.
        #[arg(long)]
        out: PathBuf,
        /// Standard view: iso, top, bottom, front, back, left, right.
        #[arg(long, default_value = "iso")]
        view: String,
        /// Image width in pixels.
        #[arg(long, default_value_t = 800)]
        width: u32,
        /// Image height in pixels.
        #[arg(long, default_value_t = 600)]
        height: u32,
    },
}

#[derive(Deserialize)]
struct Script {
    commands: Vec<Step>,
}

#[derive(Deserialize)]
struct Step {
    command: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug)]
pub(crate) enum CliError {
    Io(String),
    Script(String),
    Command { step: String, error: CommandError },
    Usage(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Io(m) => write!(f, "{m}"),
            CliError::Script(m) => write!(f, "invalid script: {m}"),
            CliError::Command { step, error } => write!(f, "{step}: {error}"),
            CliError::Usage(m) => write!(f, "{m}"),
        }
    }
}

fn exec(s: &mut Session, json_out: bool, id: &str, params: Value) -> Result<(), CliError> {
    match s.execute(id, params) {
        Ok(outcome) => {
            if json_out {
                println!(
                    "{}",
                    json!({ "command": id, "summary": outcome.summary, "data": outcome.data })
                );
            } else {
                println!("{id}: {}", outcome.summary);
            }
            Ok(())
        }
        Err(error) => Err(CliError::Command {
            step: id.to_owned(),
            error,
        }),
    }
}

/// Maps an output path to the export command for its extension.
fn export_command(path: &Path) -> Result<(&'static str, Value), CliError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let command = match ext.as_str() {
        "step" | "stp" => "file.export_step",
        "stl" => "file.export_stl",
        "kubit" | "json" => "doc.save",
        other => {
            return Err(CliError::Usage(format!(
                "unknown output extension `.{other}` for {}",
                path.display()
            )));
        }
    };
    Ok((command, json!({ "path": path })))
}

fn write_outputs(s: &mut Session, json_out: bool, outs: &[PathBuf]) -> Result<(), CliError> {
    for out in outs {
        let (command, params) = export_command(out)?;
        exec(s, json_out, command, params)?;
    }
    Ok(())
}

fn run_script(s: &mut Session, json_out: bool, script: &Path) -> Result<(), CliError> {
    let text = std::fs::read_to_string(script)
        .map_err(|e| CliError::Io(format!("{}: {e}", script.display())))?;
    let script: Script =
        serde_json::from_str(&text).map_err(|e| CliError::Script(e.to_string()))?;
    for step in script.commands {
        exec(s, json_out, &step.command, step.params)?;
    }
    Ok(())
}

/// `kubit-cli mcp`: the session lives on the server's actor thread.
fn serve_mcp(read_only: bool, allow_files: bool) -> Result<(), CliError> {
    let options = kubit_mcp::ServerOptions {
        read_only,
        allow_files,
    };
    // A current-thread runtime is enough: the session runs on its own actor thread.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Io(format!("tokio runtime: {e}")))?;
    runtime
        .block_on(kubit_mcp::serve_stdio(backend::factory, options))
        .map_err(|e| CliError::Io(e.to_string()))
}

fn run(cli: Cli) -> Result<(), CliError> {
    let mut s = Session::new(backend::factory());
    match cli.command {
        Cmd::Run { script, out } => {
            run_script(&mut s, cli.json, &script)?;
            write_outputs(&mut s, cli.json, &out)
        }
        Cmd::Mcp {
            read_only,
            allow_files,
        } => serve_mcp(read_only, allow_files),
        Cmd::Snapshot {
            input,
            out,
            view,
            width,
            height,
        } => {
            if snapshot::is_script(&input)? {
                run_script(&mut s, cli.json, &input)?;
            } else {
                exec(&mut s, cli.json, "doc.open", json!({ "path": input }))?;
            }
            let png = snapshot::render(&mut s, &view, width, height)?;
            std::fs::write(&out, png)
                .map_err(|e| CliError::Io(format!("{}: {e}", out.display())))?;
            if !cli.json {
                println!(
                    "snapshot: wrote {} ({width}×{height}, {view})",
                    out.display()
                );
            }
            Ok(())
        }
        Cmd::Export { document, out } => {
            exec(&mut s, cli.json, "doc.open", json!({ "path": document }))?;
            write_outputs(&mut s, cli.json, &out)
        }
        Cmd::Exec {
            command,
            params,
            open,
            save,
        } => {
            if let Some(path) = open {
                exec(&mut s, cli.json, "doc.open", json!({ "path": path }))?;
            }
            let params: Value = serde_json::from_str(&params)
                .map_err(|e| CliError::Usage(format!("params are not JSON: {e}")))?;
            exec(&mut s, cli.json, &command, params)?;
            if let Some(path) = save {
                exec(&mut s, cli.json, "doc.save", json!({ "path": path }))?;
            }
            Ok(())
        }
        Cmd::Commands => {
            for c in s.registry().iter() {
                let flags = match (c.mutates, c.file_access) {
                    (true, true) => " [mutates, files]",
                    (true, false) => " [mutates]",
                    (false, true) => " [files]",
                    (false, false) => "",
                };
                println!("{:<20} {}{flags}", c.id, c.description);
            }
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
