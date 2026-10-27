//! egui/eframe user interface and viewport (L5).
//!
//! The UI is a front end over `kubit_engine::Session`: toolbar buttons, shortcuts
//! and viewport clicks all become `Session::execute` calls. Layer and rules:
//! `docs/ARCHITECTURE.md`.

mod app;
mod handle;
pub mod i18n;
mod icons;
mod screenshot;
mod sketch_ui;
pub mod theme;
mod triad;
mod value_input;
pub mod viewport;

pub use app::{AppOptions, KubitApp, ScriptStep, parse_shortcut};
pub use i18n::{I18n, Lang};
pub use screenshot::screenshot_png;
pub use theme::{Theme, ThemeMode};

use kubit_engine::KernelFactory;

/// Default window size, logical points.
pub const WINDOW_SIZE: [u32; 2] = [1280, 800];

/// Opens the main window and runs until it closes.
pub fn run(factory: KernelFactory, options: AppOptions) -> eframe::Result {
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Kubit")
            .with_inner_size([WINDOW_SIZE[0] as f32, WINDOW_SIZE[1] as f32])
            .with_min_inner_size([640.0, 400.0]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "Kubit",
        native,
        Box::new(move |cc| Ok(Box::new(KubitApp::new(cc, factory, options)))),
    )
}
