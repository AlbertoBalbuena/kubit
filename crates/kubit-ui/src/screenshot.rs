//! Offscreen screenshots of the whole UI: no window, no event loop.
//!
//! Builds the same `KubitApp` on a headless egui-wgpu render state, runs a few
//! egui frames at a fixed size, paints the last one with egui-wgpu into a texture
//! and reads it back. Works where a window cannot be shown (CI, a hidden or
//! occluded window, an agent asking for `--screenshot`).

use crate::app::{AppOptions, KubitApp};
use eframe::egui_wgpu::{self, wgpu};
use kubit_engine::KernelFactory;

/// Frames run before the captured one, so layout, fonts and textures settle.
const WARMUP_FRAMES: usize = 3;
/// Largest accepted side, pixels.
const MAX_SIDE: u32 = 8192;

/// Renders the UI at `size` logical points × `pixels_per_point` and returns a PNG.
pub fn screenshot_png(
    factory: KernelFactory,
    options: AppOptions,
    size: [u32; 2],
    pixels_per_point: f32,
) -> Result<Vec<u8>, String> {
    let ppp = if pixels_per_point.is_finite() {
        pixels_per_point.clamp(0.5, 4.0)
    } else {
        1.0
    };
    let px = size.map(|s| (s as f32 * ppp).round() as u32);
    if px.iter().any(|&p| p == 0 || p > MAX_SIDE) {
        return Err(format!("screenshot must be 1..={MAX_SIDE} pixels per side"));
    }
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let rs = pollster::block_on(egui_wgpu::RenderState::create(
        &egui_wgpu::WgpuConfiguration::default(),
        &instance,
        None,
        egui_wgpu::RendererOptions::default(),
    ))
    .map_err(|e| format!("GPU: {e}"))?;

    let ctx = egui::Context::default();
    let mut app = KubitApp::build(&ctx, Some(&rs), factory, options);
    let input = || {
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0] as f32, size[1] as f32),
            )),
            ..egui::RawInput::default()
        };
        if let Some(v) = raw.viewports.get_mut(&egui::ViewportId::ROOT) {
            v.native_pixels_per_point = Some(ppp);
        }
        raw
    };
    let mut output = None;
    for _ in 0..=WARMUP_FRAMES {
        let mut out = ctx.run_ui(input(), |ui| app.draw(ui));
        // epaint asserts that every texture delta is handled before it is dropped.
        let mut textures = std::mem::take(&mut out.textures_delta);
        let mut renderer = rs.renderer.write();
        for (id, deltas) in &textures.set {
            for delta in deltas {
                renderer.update_texture(&rs.device, &rs.queue, *id, delta);
            }
        }
        for id in &textures.free {
            renderer.free_texture(id);
        }
        textures.clear();
        output = Some(out);
    }
    let out = output.ok_or("no frame")?;
    let jobs = ctx.tessellate(out.shapes, ppp);
    paint(&rs, &jobs, px, ppp)
}

fn paint(
    rs: &egui_wgpu::RenderState,
    jobs: &[egui::ClippedPrimitive],
    [width, height]: [u32; 2],
    ppp: f32,
) -> Result<Vec<u8>, String> {
    let device = &rs.device;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kubit-ui screenshot"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: rs.target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [width, height],
        pixels_per_point: ppp,
    };
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("kubit-ui screenshot"),
    });
    let mut renderer = rs.renderer.write();
    let extra = renderer.update_buffers(device, &rs.queue, &mut encoder, jobs, &screen);
    {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("kubit-ui screenshot"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..wgpu::RenderPassDescriptor::default()
        });
        // egui-wgpu wants a `'static` pass; the pass ends before `encoder` is used again.
        renderer.render(&mut pass.forget_lifetime(), jobs, &screen);
    }
    drop(renderer);

    let row = width * 4;
    let padded =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("kubit-ui screenshot readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        texture.size(),
    );
    rs.queue.submit(extra.into_iter().chain([encoder.finish()]));

    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| format!("GPU poll: {e}"))?;
    rx.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("readback: {e}"))?;
    let mapped = slice
        .get_mapped_range()
        .map_err(|e| format!("readback: {e}"))?;
    let bgra = matches!(
        rs.target_format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    );
    let mut rgba = Vec::with_capacity((row * height) as usize);
    for line in mapped.chunks(padded as usize) {
        let pixels = line.get(..row as usize).ok_or("short readback row")?;
        let (px, _) = pixels.as_chunks::<4>();
        for &[r, g, b, a] in px {
            rgba.extend_from_slice(&if bgra { [b, g, r, a] } else { [r, g, b, a] });
        }
    }
    drop(mapped);
    buffer.unmap();
    kubit_render::Image {
        width,
        height,
        rgba,
    }
    .to_png()
    .map_err(|e| e.to_string())
}
