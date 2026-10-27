//! Offscreen render targets (colour, optional MSAA resolve, depth).

use crate::{DEPTH_FORMAT, FrameTargets, Viewport};

/// Textures for one offscreen viewport. The UI can create one per viewport panel,
/// render into it and show [`RenderTarget::output`] as an egui texture.
#[derive(Debug)]
pub struct RenderTarget {
    size: Viewport,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    resolve: Option<(wgpu::Texture, wgpu::TextureView)>,
    depth_view: wgpu::TextureView,
}

impl RenderTarget {
    /// Allocates textures of `size`. With `sample_count > 1` the multisampled colour is
    /// resolved into a single-sample texture. Outputs are `TEXTURE_BINDING | COPY_SRC`.
    pub fn new(
        device: &wgpu::Device,
        size: Viewport,
        format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Self {
        let extent = wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        };
        let output_usage = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC;
        let texture = |label, samples, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let msaa = sample_count > 1;
        let color = texture(
            "kubit-render colour",
            sample_count,
            format,
            if msaa {
                wgpu::TextureUsages::RENDER_ATTACHMENT
            } else {
                output_usage
            },
        );
        let resolve = msaa.then(|| {
            let t = texture("kubit-render resolve", 1, format, output_usage);
            let v = t.create_view(&wgpu::TextureViewDescriptor::default());
            (t, v)
        });
        let depth = texture(
            "kubit-render depth",
            sample_count,
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        Self {
            size,
            color_view: color.create_view(&wgpu::TextureViewDescriptor::default()),
            color,
            resolve,
            depth_view: depth.create_view(&wgpu::TextureViewDescriptor::default()),
        }
    }

    /// Size in pixels.
    pub fn size(&self) -> Viewport {
        self.size
    }

    /// The single-sample texture holding the final image.
    pub fn output(&self) -> &wgpu::Texture {
        self.resolve.as_ref().map_or(&self.color, |(t, _)| t)
    }

    /// View of [`RenderTarget::output`].
    pub fn output_view(&self) -> &wgpu::TextureView {
        self.resolve.as_ref().map_or(&self.color_view, |(_, v)| v)
    }

    /// Attachments for [`crate::Renderer::render`].
    pub fn targets(&self) -> FrameTargets<'_> {
        FrameTargets {
            color: &self.color_view,
            resolve: self.resolve.as_ref().map(|(_, v)| v),
            depth: &self.depth_view,
        }
    }
}
