//! `kubit-cli snapshot`: offscreen render of the current model to PNG.
//!
//! Used to check the viewport without a window (CI, MCP agents, bug reports).

use crate::CliError;
use kubit_engine::Session;
use kubit_render::{Camera, MeshId, RenderStyle, Renderer, StandardView, Viewport};
use std::path::Path;

/// Chord tolerance of the snapshot tessellation, mm.
const CHORD_MM: f64 = 0.1;
/// Largest accepted image side, pixels.
const MAX_SIDE: u32 = 8192;

/// Whether `path` holds a command script (`{"commands": [...]}`) rather than a document.
pub fn is_script(path: &Path) -> Result<bool, CliError> {
    if path.extension().and_then(|e| e.to_str()) == Some("kubit") {
        return Ok(false);
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::Io(format!("{}: {e}", path.display())))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| CliError::Usage(format!("{} is not JSON: {e}", path.display())))?;
    Ok(value.get("commands").is_some())
}

fn standard_view(name: &str) -> Result<StandardView, CliError> {
    Ok(match name {
        "iso" => StandardView::Iso,
        "top" => StandardView::Top,
        "bottom" => StandardView::Bottom,
        "front" => StandardView::Front,
        "back" => StandardView::Back,
        "left" => StandardView::Left,
        "right" => StandardView::Right,
        other => return Err(CliError::Usage(format!("unknown view `{other}`"))),
    })
}

/// Renders every body of the session and returns the PNG bytes.
pub fn render(s: &mut Session, view: &str, width: u32, height: u32) -> Result<Vec<u8>, CliError> {
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(CliError::Usage(format!(
            "image size must be 1..={MAX_SIDE} pixels per side"
        )));
    }
    let view = standard_view(view)?;
    let render_err = |e: kubit_render::RenderError| CliError::Io(format!("render: {e}"));
    let mut renderer = Renderer::headless().map_err(render_err)?;
    let bodies: Vec<_> = s.model().bodies.keys().copied().collect();
    for body in bodies {
        let mesh = s
            .body_mesh(body, CHORD_MM)
            .map_err(|error| CliError::Command {
                step: format!("tessellate {body}"),
                error,
            })?;
        renderer
            .upload(MeshId(u64::from(body.0)), mesh)
            .map_err(render_err)?;
    }
    let size = Viewport::new(width, height);
    let mut camera = Camera::default();
    camera.set_view(view);
    if let Some(bounds) = renderer.bounds() {
        camera.fit(&bounds, size.aspect());
    }
    renderer
        .render_to_png(&camera, size, &RenderStyle::default())
        .map_err(render_err)
}
