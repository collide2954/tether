use crate::math::Vec2;

pub const FRAME: Vec2 = Vec2::new(16.0, 9.0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub center: Vec2,
    pub scale: f32,
    pub viewport: Vec2,
}

impl Camera {
    pub fn fit(frame: Vec2, viewport: Vec2) -> Self {
        let viewport = viewport.max(Vec2::ONE);
        let scale = (viewport.x / frame.x).min(viewport.y / frame.y);
        Self {
            center: Vec2::ZERO,
            scale,
            viewport,
        }
    }

    pub fn world_to_screen(&self, p: Vec2) -> Vec2 {
        let d = (p - self.center) * self.scale;
        Vec2::new(self.viewport.x * 0.5 + d.x, self.viewport.y * 0.5 - d.y)
    }

    pub fn screen_to_world(&self, s: Vec2) -> Vec2 {
        let d = Vec2::new(s.x - self.viewport.x * 0.5, self.viewport.y * 0.5 - s.y);
        self.center + d / self.scale
    }

    pub fn clip_transform(&self) -> [f32; 4] {
        let sx = 2.0 * self.scale / self.viewport.x;
        let sy = 2.0 * self.scale / self.viewport.y;
        [sx, sy, -self.center.x * sx, -self.center.y * sy]
    }

    pub fn pixel_size(&self) -> f32 {
        1.0 / self.scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: Vec2, b: Vec2) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn fit_uses_the_limiting_axis() {
        assert_eq!(Camera::fit(FRAME, Vec2::new(1920.0, 1080.0)).scale, 120.0);
        assert_eq!(Camera::fit(FRAME, Vec2::new(1920.0, 1200.0)).scale, 120.0);
        assert_eq!(Camera::fit(FRAME, Vec2::new(2560.0, 1080.0)).scale, 120.0);
        assert_eq!(Camera::fit(FRAME, Vec2::new(1080.0, 1920.0)).scale, 67.5);
    }

    #[test]
    fn fit_survives_a_zero_viewport() {
        let c = Camera::fit(FRAME, Vec2::ZERO);
        assert!(c.scale > 0.0 && c.scale.is_finite());
    }

    #[test]
    fn world_to_screen_puts_origin_at_center_with_y_down() {
        let c = Camera::fit(FRAME, Vec2::new(1920.0, 1080.0));
        assert!(near(c.world_to_screen(Vec2::ZERO), Vec2::new(960.0, 540.0)));
        assert!(near(
            c.world_to_screen(Vec2::new(8.0, 4.5)),
            Vec2::new(1920.0, 0.0)
        ));
        assert!(near(
            c.world_to_screen(Vec2::new(-8.0, -4.5)),
            Vec2::new(0.0, 1080.0)
        ));
    }

    #[test]
    fn screen_to_world_inverts_world_to_screen() {
        let c = Camera::fit(FRAME, Vec2::new(1280.0, 1024.0));
        let p = Vec2::new(-3.25, 2.5);
        assert!(near(c.screen_to_world(c.world_to_screen(p)), p));
    }

    #[test]
    fn clip_transform_maps_frame_corner_to_clip_corner() {
        let c = Camera::fit(FRAME, Vec2::new(1920.0, 1080.0));
        let [sx, sy, tx, ty] = c.clip_transform();
        let corner = Vec2::new(8.0, 4.5);
        assert!(near(
            Vec2::new(corner.x * sx + tx, corner.y * sy + ty),
            Vec2::ONE
        ));
    }

    #[test]
    fn pixel_size_is_inverse_scale() {
        let c = Camera::fit(FRAME, Vec2::new(1920.0, 1080.0));
        assert!((c.pixel_size() - 1.0 / 120.0).abs() < 1e-7);
    }
}
