//! Offscreen UI screenshot (needs a GPU; skips with a note when there is none).

#![allow(clippy::unwrap_used, clippy::panic)]

use kubit_engine::KernelFactory;
use kubit_ui::{AppOptions, Lang, ScriptStep};

fn factory() -> KernelFactory {
    Box::new(|| Box::new(kubit_kernel_api::testing::BoxKernel::new()))
}

#[test]
fn screenshot_renders_the_ui() {
    let options = AppOptions {
        lang: Some(Lang::Es),
        startup: vec![ScriptStep {
            command: "solid.box".into(),
            params: serde_json::json!({ "size": [100, 60, 40] }),
        }],
        ..AppOptions::default()
    };
    let png = match kubit_ui::screenshot_png(factory(), options, [640, 400], 1.0) {
        Ok(png) => png,
        Err(e) if e.starts_with("GPU") => {
            eprintln!("skipping: no GPU adapter ({e})");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    assert_eq!(&png[1..4], b"PNG");
    // IHDR width and height (big endian) right after the signature and chunk header.
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 640);
    assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 400);
}

#[test]
fn screenshot_rejects_bad_sizes() {
    let err = kubit_ui::screenshot_png(factory(), AppOptions::default(), [0, 10], 1.0);
    assert!(err.is_err());
}
