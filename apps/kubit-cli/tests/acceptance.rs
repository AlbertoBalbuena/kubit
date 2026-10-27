//! Phase 0 acceptance: `kubit-cli run examples/box.json --out box.step` on the real
//! backend, and the exported STEP re-imports with the same volume.

use kubit_kernel_api::{Guarded, Kernel};
use kubit_kernel_arris::ArrisKernel;
use std::path::PathBuf;
use std::process::Command;

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/box.json")
}

#[test]
fn run_box_example_exports_step_that_reimports_with_same_volume() {
    let dir = std::env::temp_dir().join(format!("kubit-cli-accept-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let step = dir.join("box.step");
    let kubit = dir.join("box.kubit");
    let out = Command::new(env!("CARGO_BIN_EXE_kubit-cli"))
        .arg("run")
        .arg(example())
        .arg("--out")
        .arg(&step)
        .arg("--out")
        .arg(&kubit)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("300000.000 mm³"), "{stdout}");

    let text = std::fs::read_to_string(&step).unwrap();
    let mut k = Guarded::new(ArrisKernel::new());
    let read = k.import_step(&text).unwrap();
    assert_eq!(read.len(), 1);
    let v = k.mass_properties(read[0].body).unwrap().volume;
    assert!(
        (v - 300_000.0).abs() / 300_000.0 < 1e-9,
        "re-imported volume {v}"
    );

    // The saved document reopens and rebuilds to the same volume.
    let out = Command::new(env!("CARGO_BIN_EXE_kubit-cli"))
        .args(["exec", "body.measure", "{}", "--open"])
        .arg(&kubit)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("300000.000 mm³"));
    std::fs::remove_dir_all(&dir).ok();
}

/// Milestone 1.3: box → fillet the four vertical edges → push/pull the top →
/// shell open at the top, driven by `run` with no selection.
#[test]
fn run_bracket_direct_example_has_the_analytic_volume() {
    let corner = 1.0 - std::f64::consts::PI / 4.0;
    let outer = 6000.0 - 4.0 * corner * 25.0;
    let inner = 5376.0 - 4.0 * corner * 9.0;
    let expected = outer * 50.0 - inner * 48.0;

    let out = Command::new(env!("CARGO_BIN_EXE_kubit-cli"))
        .arg("run")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bracket-direct.json"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let volume: f64 = stdout
        .split("Total volume ")
        .nth(1)
        .and_then(|rest| rest.split(" mm³").next())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or_else(|| panic!("no total volume in:\n{stdout}"));
    // The summary prints three decimals.
    assert!((volume - expected).abs() < 2e-3, "{volume} vs {expected}");
}

/// Milestone 1.4b: a plate with a hole extruded from an explicit profile, plus a
/// joined boss, driven by `run`.
#[test]
fn run_extrude_profile_example_has_the_analytic_volume() {
    let pi = std::f64::consts::PI;
    let expected = (1200.0 - 16.0 * pi) * 10.0 + pi * 9.0 * 5.0;

    let out = Command::new(env!("CARGO_BIN_EXE_kubit-cli"))
        .arg("run")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/extrude-profile.json"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let volume: f64 = stdout
        .split("Total volume ")
        .nth(1)
        .and_then(|rest| rest.split(" mm³").next())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or_else(|| panic!("no total volume in:\n{stdout}"));
    assert!((volume - expected).abs() < 2e-3, "{volume} vs {expected}");
}

/// Milestone 1.4: sketch → extrude → sketch on the top face → cut → edit a dimension,
/// driven by `run`; the cut survives the rebuild.
#[test]
fn run_sketch_extrude_example_rebuilds_with_the_cut() {
    let expected = (4500.0 - 125.0 * std::f64::consts::PI) * 20.0;
    let out = Command::new(env!("CARGO_BIN_EXE_kubit-cli"))
        .arg("run")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/sketch-extrude.json"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let volume: f64 = stdout
        .split("Total volume ")
        .nth(1)
        .and_then(|rest| rest.split(" mm³").next())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or_else(|| panic!("no total volume in:\n{stdout}"));
    assert!((volume - expected).abs() < 2e-3, "{volume} vs {expected}");
}
