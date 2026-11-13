#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

impl Rgba {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn hex(rgb: u32) -> Self {
        let channel = |shift: u32| srgb_to_linear(((rgb >> shift) & 0xFF) as f32 / 255.0);
        Self::new(channel(16), channel(8), channel(0), 1.0)
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
            mix(self.a, other.a),
        )
    }

    pub fn premultiplied(self) -> [f32; 4] {
        [self.r * self.a, self.g * self.a, self.b * self.a, self.a]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub ground: Rgba,
    pub ink: Rgba,
    pub red: Rgba,
    pub yellow: Rgba,
    pub blue: Rgba,
    pub pulse: Rgba,
    pub halo: Rgba,
}

impl Palette {
    pub fn paper() -> Self {
        Self {
            ground: Rgba::hex(0xEFE9DD),
            ink: Rgba::hex(0x18171C),
            red: Rgba::hex(0xD1301F),
            yellow: Rgba::hex(0xF0B21A),
            blue: Rgba::hex(0x1F4FA3),
            pulse: Rgba::new(1.0, 1.0, 1.0, 1.0),
            halo: Rgba::hex(0xF39C34),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn srgb_to_linear_matches_reference_values() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!(near(srgb_to_linear(1.0), 1.0));
        assert!(near(srgb_to_linear(0.5), 0.21404));
        assert!(near(srgb_to_linear(0.02), 0.02 / 12.92));
    }

    #[test]
    fn hex_reads_srgb_channels_into_linear_space() {
        let c = Rgba::hex(0xFF8000);
        assert!(near(c.r, 1.0));
        assert!(near(c.g, 0.21586));
        assert_eq!(c.b, 0.0);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn with_alpha_changes_only_alpha() {
        let c = Rgba::new(0.1, 0.2, 0.3, 1.0).with_alpha(0.25);
        assert_eq!(c, Rgba::new(0.1, 0.2, 0.3, 0.25));
    }

    #[test]
    fn lerp_blends_each_channel() {
        let a = Rgba::new(0.0, 0.0, 0.0, 0.0);
        let b = Rgba::new(1.0, 0.5, 0.25, 1.0);
        assert_eq!(a.lerp(b, 0.5), Rgba::new(0.5, 0.25, 0.125, 0.5));
    }

    #[test]
    fn premultiplied_scales_color_by_alpha() {
        let c = Rgba::new(0.5, 0.2, 0.1, 0.5).premultiplied();
        assert!(near(c[0], 0.25) && near(c[1], 0.1) && near(c[2], 0.05) && near(c[3], 0.5));
    }

    #[test]
    fn paper_palette_uses_design_values() {
        let p = Palette::paper();
        assert_eq!(p.ground, Rgba::hex(0xEFE9DD));
        assert_eq!(p.ink, Rgba::hex(0x18171C));
        assert_eq!(p.red, Rgba::hex(0xD1301F));
        assert_eq!(p.yellow, Rgba::hex(0xF0B21A));
        assert_eq!(p.blue, Rgba::hex(0x1F4FA3));
    }
}
