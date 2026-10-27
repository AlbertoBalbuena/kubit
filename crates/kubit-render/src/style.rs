//! Colours and shading parameters. The UI owns the theme and passes a `RenderStyle`
//! in; `RenderStyle::default()` is the only place colours are written down here.

/// An sRGB colour with linear alpha, components in [0, 1] (what theme tokens hold).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Srgba(pub [f32; 4]);

impl Srgba {
    /// From 8-bit sRGB components.
    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        ])
    }

    /// Linear-light RGB (alpha unchanged), what the shader and sRGB targets expect.
    pub fn to_linear(self) -> [f32; 4] {
        fn channel(c: f32) -> f32 {
            let c = c.clamp(0.0, 1.0);
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        let [r, g, b, a] = self.0;
        [channel(r), channel(g), channel(b), a.clamp(0.0, 1.0)]
    }
}

/// Everything visual the viewport needs from the theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderStyle {
    /// Clear colour.
    pub background: Srgba,
    /// Base colour of faces.
    pub face: Srgba,
    /// Colour of the hovered face (mixed in by `hover_mix`).
    pub hover: Srgba,
    /// Colour of selected faces.
    pub selected: Srgba,
    /// Feature-edge colour.
    pub edge: Srgba,
    /// Minor grid lines (alpha is the strongest they get).
    pub grid_minor: Srgba,
    /// Major grid lines, every ten minor ones.
    pub grid_major: Srgba,
    /// World X axis.
    pub axis_x: Srgba,
    /// World Y axis.
    pub axis_y: Srgba,
    /// World Z axis.
    pub axis_z: Srgba,
    /// Whether the ground grid and axes are drawn.
    pub show_grid: bool,
    /// Whether B-rep edges are drawn over the shading.
    pub show_edges: bool,
    /// Ambient light, 0..1.
    pub ambient: f32,
    /// Headlight diffuse strength, 0..1.
    pub diffuse: f32,
    /// How much of `hover` replaces the base colour, 0..1.
    pub hover_mix: f32,
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            background: Srgba::from_rgba8(0x24, 0x27, 0x2e, 0xff),
            face: Srgba::from_rgba8(0xb8, 0xc0, 0xcc, 0xff),
            hover: Srgba::from_rgba8(0x7c, 0xc4, 0xff, 0xff),
            selected: Srgba::from_rgba8(0x2f, 0x8f, 0xff, 0xff),
            edge: Srgba::from_rgba8(0x10, 0x12, 0x16, 0xff),
            grid_minor: Srgba::from_rgba8(0x8a, 0x93, 0xa3, 0x30),
            grid_major: Srgba::from_rgba8(0x8a, 0x93, 0xa3, 0x70),
            axis_x: Srgba::from_rgba8(0xe5, 0x4b, 0x4b, 0xd0),
            axis_y: Srgba::from_rgba8(0x4c, 0xc2, 0x6a, 0xd0),
            axis_z: Srgba::from_rgba8(0x4a, 0x8c, 0xf0, 0xd0),
            show_grid: true,
            show_edges: true,
            ambient: 0.35,
            diffuse: 0.65,
            hover_mix: 0.55,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_to_linear_endpoints_and_midpoint() {
        assert_eq!(
            Srgba([0.0, 1.0, 0.0, 0.5]).to_linear(),
            [0.0, 1.0, 0.0, 0.5]
        );
        let mid = Srgba([0.5; 4]).to_linear()[0];
        assert!((mid - 0.214).abs() < 1e-3, "{mid}");
    }
}
