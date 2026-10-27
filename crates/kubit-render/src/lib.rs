//! Window-less wgpu renderer (L1).
//!
//! Draws tessellated bodies ([`kubit_mesh::TriMesh`]) with flat headlight shading,
//! feature edges and hover/selection highlights, picks faces through an id pass,
//! and renders offscreen to PNG for snapshots. It never opens a window: the UI hands
//! in its own `wgpu::Device` ([`Renderer::new`]) and the CLI/MCP use
//! [`Renderer::headless`]. The [`Camera`] is pure math and input-agnostic.
//!
//! Layer and rules: `docs/ARCHITECTURE.md`. Versions and licences: `README.md`.

mod camera;
mod edge_pick;
mod edges;
mod grid;
mod ids;
pub mod math;
mod readback;
mod renderer;
mod style;
mod target;

pub use camera::{Camera, Frame, Projection, StandardView, Viewport};
pub use edge_pick::{EDGE_PICK_TOLERANCE_PX, nearest_edge, polyline_distance_px, project_to_pixel};
pub use edges::{DEFAULT_CREASE_ANGLE, feature_edges};
pub use grid::{GridVertex, MAX_GRID_VERTICES, grid_lines, grid_spacing};
pub use ids::{BACKGROUND_ID, FaceTable, IdMap, MeshId};
pub use kubit_mesh::{EdgeTag, FaceTag};
pub use readback::ReadbackStatus;
pub use renderer::{FrameTargets, Image, MAX_SELECTED, PendingPick, Renderer};
pub use style::{RenderStyle, Srgba};
pub use target::RenderTarget;
pub use wgpu;

/// Depth format every pass uses; a caller-provided depth view must have it.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Renderer errors. GPU validation messages are kept as text.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// No GPU adapter (e.g. CI without a software rasteriser).
    #[error("no GPU adapter available: {0}")]
    NoAdapter(String),
    /// The adapter refused to create a device.
    #[error("could not create a GPU device: {0}")]
    Device(String),
    /// wgpu reported a validation, out-of-memory or internal error.
    #[error("GPU error: {0}")]
    Gpu(String),
    /// Waiting on the device failed.
    #[error("GPU poll failed: {0}")]
    Poll(String),
    /// Mapping a readback buffer failed.
    #[error("GPU readback failed")]
    Readback,
    /// The mesh is structurally invalid.
    #[error("invalid mesh: {0}")]
    InvalidMesh(String),
    /// The mesh has more vertices or faces than the GPU index space allows.
    #[error("mesh too large for the GPU: {0}")]
    MeshTooLarge(String),
    /// Every pick id is in use (practically unreachable: 2³² faces).
    #[error("face id space exhausted")]
    IdSpaceExhausted,
    /// Unsupported sample count (WebGPU guarantees only 1 and 4).
    #[error("unsupported MSAA sample count {0} (use 1 or 4)")]
    SampleCount(u32),
    /// The colour format cannot be read back as RGBA8.
    #[error("cannot read back colour format {0:?} (use an RGBA8 or BGRA8 format)")]
    ReadbackFormat(wgpu::TextureFormat),
    /// Requested image size is zero or above the device limit.
    #[error("image size {width}×{height} is outside 1..={max}")]
    ImageSize {
        /// Width, px.
        width: u32,
        /// Height, px.
        height: u32,
        /// Device limit, px.
        max: u32,
    },
    /// PNG encoding failed.
    #[error("PNG encoding failed: {0}")]
    Png(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, RenderError>;
