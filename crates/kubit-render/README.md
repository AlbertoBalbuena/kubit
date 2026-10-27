# kubit-render

Window-less wgpu renderer (layer 1). Draws `kubit_mesh::TriMesh` bodies with flat
headlight shading, feature edges, hover/selection highlights; picks faces through an
id pass; renders offscreen to PNG. No winit, no egui: the UI passes in eframe's
`wgpu::Device`, and the CLI and MCP use `Renderer::headless()`.

## Dependencies (checked on crates.io, 2026-10-26)

| Crate | Version | Licence | Why |
|---|---|---|---|
| `wgpu` | 30.0.1 | MIT OR Apache-2.0 | GPU API. **Pinned to the major `egui-wgpu` 0.37.0 / `eframe` 0.37.0 depend on (`wgpu ^30.0`)**, so the UI ends up with a single wgpu in the tree. Bump together with eframe. |
| `bytemuck` | 1.25.2 | Zlib OR Apache-2.0 OR MIT | `Pod` casts for vertex/uniform buffers (wgpu already depends on it). |
| `png` | 0.18.1 | MIT OR Apache-2.0 | PNG encoding for snapshots (pure Rust, builds for wasm32). |
| `pollster` | 1.0.1 | Apache-2.0 OR MIT | Native-only `block_on` for the blocking helpers (egui-wgpu uses it too). |
| `thiserror` | workspace | MIT OR Apache-2.0 | Errors. |

wgpu uses its default features (Metal, Vulkan, DX12, GLES, WebGPU on wasm32, WGSL).

## Conventions

- World units: millimetres, **Z up**. Positions are converted f64 → f32 on upload
  (fine for parts within ±10⁵ mm of the origin; a per-mesh origin offset is the fix
  if we ever need larger coordinates).
- Clip space as wgpu: depth in [0, 1], standard (not reversed) Z with `Depth32Float`.
- Colours in `RenderStyle` are sRGB (theme tokens); they are converted to linear when
  the target format is sRGB.

## API

- `Renderer::new(&device, &queue, color_format, sample_count)` — share an external
  device (sample count 1 or 4). `Renderer::headless()` (native) /
  `Renderer::headless_async()` (any target) create their own device:
  `Rgba8UnormSrgb`, 4× MSAA; `Err(RenderError::NoAdapter)` when no GPU exists.
- `upload(MeshId, &TriMesh)` / `remove(MeshId)` / `bounds()`.
- `set_hovered(Option<(MeshId, FaceTag)>)`, `set_selected(&[(MeshId, FaceTag)])`
  (up to `MAX_SELECTED` = 16 highlighted).
- `render(&mut encoder, &FrameTargets, &Camera, Viewport, &RenderStyle)` records
  the shaded pass. `RenderTarget` allocates matching colour/resolve/depth textures.
- Picking: `pick_submit` → `pick_resolve` (non-blocking, all targets) or `pick`
  (blocking, native).
- Snapshots (native): `render_to_image` → `Image { width, height, rgba }`,
  `render_to_png`.
- `Camera` (pure f64 math): `orbit`, `pan`, `zoom` (to cursor), `fit`, `set_view`,
  `view_projection`, `pick_projection`, `ray`. `Viewport::pixel_to_ndc`.
- `feature_edges(&TriMesh, crease_angle)`.

## How it works

- **Faces**: unwelded vertex buffer, three vertices per triangle with the triangle's
  normal (flat shading) and a mesh-local face index. Lighting is a two-sided
  headlight Lambert plus ambient.
- **Edges**: line list of feature edges — boundary or non-manifold edges, edges
  between different `FaceTag`s, and creases over 30°. Faces get a depth bias so
  coplanar edges win (WebGPU forbids depth bias on lines).
- **Ids**: each mesh owns a contiguous range of global `u32` ids (0 = background).
  The base is a per-instance vertex attribute in a 4-byte buffer, so re-packing the
  id space never rewrites vertex data. `FaceTag(u64)` maps to a dense local index.
- **Picking**: instead of rendering a full-size id buffer, `Camera::pick_projection`
  maps the pixel under the cursor onto a 1×1 `R32Uint` target (a pick matrix), then
  one texel is copied and read back.
- **Readback**: `map_async` only runs its callback when the device is polled. Native
  helpers block with `Device::poll(PollType::wait_indefinitely())`. On the web
  `poll` is a no-op and the browser resolves the map on its event loop, so the web
  UI calls `pick_submit` and then `pick_resolve` on later frames until it returns
  `Some`. The blocking helpers are `cfg(not(target_arch = "wasm32"))`.
- **Errors**: pipeline creation, picking and snapshots run inside a validation error
  scope (resolved synchronously on native). The headless device installs an
  uncaptured-error handler that records errors instead of wgpu's default panic.

## UI integration (later, `kubit-ui`)

Render into a `RenderTarget` from an egui-wgpu callback's `prepare` (it receives a
`CommandEncoder`) and show `output_view()` as an egui texture. Record at most one
`render` per renderer per queue submission: uniforms are written with
`Queue::write_buffer`.

## Status (2026-10-27)

- **Done:** camera, edges, ids, shaders, renderer, picking, PNG snapshots; 28 tests pass on Metal; fmt, clippy `-D warnings` (native and wasm32), wasm build and `cargo deny` pass.
- **Needs (outside this crate):** `layers.toml` must allow `kubit-render = { layer = 1, wasm = true, same_layer = ["kubit-mesh"] }`, or `cargo xtask layers` fails.
