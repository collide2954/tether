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
    pub neutral: Rgba,
    pub red: Rgba,
    pub yellow: Rgba,
    pub blue: Rgba,
    pub pulse: Rgba,
    pub halo: Rgba,
    pub lattice: Rgba,
    pub weld: Rgba,
}

impl Palette {
    pub fn video() -> Self {
        Self {
            ground: Rgba::hex(0x1C1E22),
            neutral: Rgba::hex(0xE6DCC4),
            red: Rgba::hex(0xC8563C),
            yellow: Rgba::hex(0xD9A441),
            blue: Rgba::hex(0x34406A),
            pulse: Rgba::new(6.0, 6.0, 6.0, 1.0),
            halo: Rgba::new(1.0, 0.92, 0.82, 1.0),
            lattice: Rgba::hex(0x2C3036),
            weld: Rgba::hex(0x3FE6F0),
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
    fn video_palette_uses_design_values() {
        let p = Palette::video();
        assert_eq!(p.ground, Rgba::hex(0x1C1E22));
        assert_eq!(p.lattice, Rgba::hex(0x2C3036));
        assert_eq!(p.neutral, Rgba::hex(0xE6DCC4));
        assert_eq!(p.red, Rgba::hex(0xC8563C));
        assert_eq!(p.yellow, Rgba::hex(0xD9A441));
        assert_eq!(p.blue, Rgba::hex(0x34406A));
        assert_eq!(p.weld, Rgba::hex(0x3FE6F0));
    }

    #[test]
    fn video_palette_pulse_is_bright_enough_to_bloom() {
        let p = Palette::video();
        assert!(p.pulse.r > 1.0 && p.pulse.g > 1.0 && p.pulse.b > 1.0);
        assert_eq!(p.pulse.a, 1.0);
    }
}
