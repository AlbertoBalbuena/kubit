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
