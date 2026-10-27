//! Kubit desktop application.

mod backend;

use clap::{Parser, ValueEnum};
use kubit_ui::{AppOptions, Lang, ScriptStep, ThemeMode};
use std::path::PathBuf;
use std::process::ExitCode;

/// Kubit: direct and parametric 3D CAD.
#[derive(Debug, Parser)]
#[command(name = "kubit", version)]
struct Args {
    /// Run a command script at startup (`{"commands": [{"command": …, "params": …}]}`,
    /// the `kubit-cli run` format).
    #[arg(long)]
    run: Option<PathBuf>,
    /// Render the UI offscreen (no window), save it to this PNG and quit.
    #[arg(long)]
    screenshot: Option<PathBuf>,
    /// Open this sketch feature (e.g. `F1`) in sketch mode after `--run`.
    #[arg(long)]
    edit_sketch: Option<String>,
    /// Start with the side panels collapsed.
    #[arg(long)]
    collapsed: bool,
    /// Pixels per point of `--screenshot`.
    #[arg(long, default_value_t = 2.0)]
    scale: f32,
    /// UI language (default: system locale, else English).
    #[arg(long, value_enum)]
    lang: Option<LangArg>,
    /// Appearance.
    #[arg(long, value_enum, default_value_t = ThemeArg::Dark)]
    theme: ThemeArg,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum LangArg {
    En,
    Es,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ThemeArg {
    Dark,
    Light,
}

fn read_script(path: &PathBuf) -> Result<Vec<ScriptStep>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let steps = value
        .get("commands")
        .and_then(|c| c.as_array())
        .ok_or_else(|| format!("{}: expected {{\"commands\": [...]}}", path.display()))?;
    steps
        .iter()
        .map(|s| {
            let command = s
                .get("command")
                .and_then(|c| c.as_str())
                .ok_or_else(|| format!("{}: a step has no `command`", path.display()))?;
            Ok(ScriptStep {
                command: command.to_owned(),
                params: s.get("params").cloned().unwrap_or(serde_json::Value::Null),
            })
        })
        .collect()
}

fn main() -> ExitCode {
    let args = Args::parse();
    let startup = match args.run.as_ref().map(read_script).transpose() {
        Ok(steps) => steps.unwrap_or_default(),
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let options = AppOptions {
        lang: args.lang.map(|l| match l {
            LangArg::En => Lang::En,
            LangArg::Es => Lang::Es,
        }),
        theme: match args.theme {
            ThemeArg::Dark => ThemeMode::Dark,
            ThemeArg::Light => ThemeMode::Light,
        },
        startup,
        edit_sketch: args.edit_sketch,
        collapsed: args.collapsed,
    };
    if let Some(path) = args.screenshot {
        let written = kubit_ui::screenshot_png(
            backend::factory(),
            options,
            kubit_ui::WINDOW_SIZE,
            args.scale,
        )
        .and_then(|png| std::fs::write(&path, png).map_err(|e| e.to_string()));
        return match written {
            Ok(()) => {
                println!("wrote {}", path.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                ExitCode::FAILURE
            }
        };
    }
    match kubit_ui::run(backend::factory(), options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
