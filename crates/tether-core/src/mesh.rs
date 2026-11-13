use crate::color::Rgba;
use crate::math::Vec2;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub local: [f32; 2],
    pub color: [f32; 4],
    pub shape: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Band = 0,
    Disc = 1,
    Ring = 2,
    Square = 3,
    Triangle = 4,
    Glow = 5,
}

const MITER_LIMIT: f32 = 2.5;

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub feather: f32,
    pub soft: f32,
}

impl Mesh {
    pub fn new(feather: f32) -> Self {
        Self {
            feather,
            ..Self::default()
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    pub fn band(&mut self, points: &[Vec2], half_widths: &[f32], color: Rgba) {
        let n = points.len().min(half_widths.len());
        if n < 2 {
            return;
        }
        let base = self.vertices.len() as u32;
        let color = color.premultiplied();
        let pad = self.feather + self.soft;
        let mut along = 0.0;
        for i in 0..n {
            if i > 0 {
                along += points[i].distance(points[i - 1]);
            }
            let normal = miter(&points[..n], i);
            let reach = half_widths[i] + pad;
            for side in [1.0, -1.0] {
                self.vertices.push(Vertex {
                    pos: (points[i] + normal * reach * side).into(),
                    local: [along, reach * side],
                    color,
                    shape: [Kind::Band as u32 as f32, half_widths[i], 0.0, self.soft],
                });
            }
        }
        for i in 0..n as u32 - 1 {
            let a = base + 2 * i;
            self.indices
                .extend_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
        }
    }

    pub fn disc(&mut self, center: Vec2, radius: f32, color: Rgba) {
        let pad = self.feather + self.soft;
        self.quad(center, radius + pad, 0.0, color, (Kind::Disc, radius, 0.0));
    }

    pub fn ring(&mut self, center: Vec2, radius: f32, thickness: f32, color: Rgba) {
        let reach = radius + 0.5 * thickness + self.feather + self.soft;
        self.quad(center, reach, 0.0, color, (Kind::Ring, radius, thickness));
    }

    pub fn square(&mut self, center: Vec2, half: f32, angle: f32, color: Rgba) {
        let pad = self.feather + self.soft;
        self.quad(center, half + pad, angle, color, (Kind::Square, half, 0.0));
    }

    pub fn triangle(&mut self, center: Vec2, radius: f32, angle: f32, color: Rgba) {
        let pad = self.feather + self.soft;
        self.quad(
            center,
            radius + pad,
            angle,
            color,
            (Kind::Triangle, radius, 0.0),
        );
    }

    pub fn glow(&mut self, center: Vec2, radius: f32, color: Rgba) {
        self.quad(center, radius, 0.0, color, (Kind::Glow, radius, 0.0));
    }

    fn quad(&mut self, center: Vec2, half: f32, angle: f32, color: Rgba, shape: (Kind, f32, f32)) {
        let base = self.vertices.len() as u32;
        let (sin, cos) = angle.sin_cos();
        let color = color.premultiplied();
        let shape = [shape.0 as u32 as f32, shape.1, shape.2, self.soft];
        for corner in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let local = Vec2::new(corner.0, corner.1) * half;
            let turned = Vec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos);
            self.vertices.push(Vertex {
                pos: (center + turned).into(),
                local: local.into(),
                color,
                shape,
            });
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

fn miter(points: &[Vec2], i: usize) -> Vec2 {
    let n = points.len();
    let dir = |a: usize, b: usize| (points[b] - points[a]).normalize_or_zero();
    let incoming = if i > 0 { dir(i - 1, i) } else { dir(0, 1) };
    let outgoing = if i + 1 < n {
        dir(i, i + 1)
    } else {
        dir(n - 2, n - 1)
    };
    let out_normal = outgoing.perp();
    let sum = incoming.perp() + out_normal;
    let length = sum.length();
    let normal = if length > 1e-6 {
        sum / length
    } else {
        out_normal
    };
    normal / normal.dot(out_normal).max(1.0 / MITER_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INK: Rgba = Rgba::new(0.2, 0.4, 0.6, 0.5);

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    fn pos(v: &Vertex) -> Vec2 {
        Vec2::from(v.pos)
    }

    #[test]
    fn band_needs_two_points() {
        let mut m = Mesh::new(0.1);
        m.band(&[Vec2::ZERO], &[0.5], INK);
        assert!(m.vertices.is_empty() && m.indices.is_empty());
    }

    #[test]
    fn straight_band_has_two_vertices_per_point_and_feathered_edges() {
        let mut m = Mesh::new(0.1);
        let pts = [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(2.0, 0.0),
        ];
        m.band(&pts, &[0.5; 3], INK);
        assert_eq!(m.vertices.len(), 6);
        assert_eq!(m.indices.len(), 12);
        assert!(m.indices.iter().all(|&i| i < 6));
        for v in &m.vertices {
            assert!(near(v.pos[1].abs(), 0.6));
            assert!(near(v.local[1].abs(), 0.6));
            assert_eq!(v.shape[0], Kind::Band as u32 as f32);
            assert!(near(v.shape[1], 0.5));
        }
        assert!(near(m.vertices[4].local[0], 2.0));
    }

    #[test]
    fn band_corner_uses_a_limited_miter() {
        let mut m = Mesh::new(0.0);
        let pts = [Vec2::new(-1.0, 0.0), Vec2::ZERO, Vec2::new(0.0, 1.0)];
        m.band(&pts, &[0.5; 3], INK);
        let offset = pos(&m.vertices[2]) - pts[1];
        let expected = 0.5 * std::f32::consts::SQRT_2;
        assert!(near(offset.length(), expected));
        assert!(near(
            offset
                .normalize()
                .dot(Vec2::new(-1.0, 1.0).normalize())
                .abs(),
            1.0
        ));

        let mut sharp = Mesh::new(0.0);
        let hairpin = [Vec2::new(-1.0, 0.0), Vec2::ZERO, Vec2::new(-1.0, 0.01)];
        sharp.band(&hairpin, &[0.5; 3], INK);
        let offset = pos(&sharp.vertices[2]) - hairpin[1];
        assert!(offset.length() <= 0.5 * MITER_LIMIT + 1e-4);
    }

    #[test]
    fn band_stores_premultiplied_color() {
        let mut m = Mesh::new(0.1);
        m.band(&[Vec2::ZERO, Vec2::X], &[0.5; 2], INK);
        assert_eq!(m.vertices[0].color, INK.premultiplied());
    }

    #[test]
    fn disc_is_a_feathered_quad() {
        let mut m = Mesh::new(0.1);
        m.disc(Vec2::new(2.0, 3.0), 1.0, INK);
        assert_eq!(m.vertices.len(), 4);
        assert_eq!(m.indices, vec![0, 1, 2, 0, 2, 3]);
        for v in &m.vertices {
            assert!(near(
                (pos(v) - Vec2::new(2.0, 3.0)).abs().max_element(),
                1.1
            ));
            assert!(near(Vec2::from(v.local).abs().max_element(), 1.1));
            assert_eq!(v.shape[0], Kind::Disc as u32 as f32);
            assert!(near(v.shape[1], 1.0));
        }
    }

    #[test]
    fn later_shapes_offset_their_indices() {
        let mut m = Mesh::new(0.1);
        m.disc(Vec2::ZERO, 1.0, INK);
        m.ring(Vec2::ZERO, 1.0, 0.2, INK);
        assert_eq!(&m.indices[6..], &[4, 5, 6, 4, 6, 7]);
        assert_eq!(m.vertices[4].shape[0], Kind::Ring as u32 as f32);
        assert!(near(m.vertices[4].shape[2], 0.2));
    }

    #[test]
    fn rotated_square_keeps_local_coordinates_unrotated() {
        let mut m = Mesh::new(0.0);
        m.square(Vec2::ZERO, 1.0, std::f32::consts::FRAC_PI_4, INK);
        let corner = pos(&m.vertices[0]);
        assert!(near(corner.length(), std::f32::consts::SQRT_2));
        assert!(near(corner.x.abs().min(corner.y.abs()), 0.0));
        assert!(near(
            Vec2::from(m.vertices[0].local).abs().max_element(),
            1.0
        ));
        assert_eq!(m.vertices[0].shape[0], Kind::Square as u32 as f32);
    }

    #[test]
    fn triangle_and_glow_set_their_kinds() {
        let mut m = Mesh::new(0.1);
        m.triangle(Vec2::ZERO, 1.0, 0.0, INK);
        m.glow(Vec2::ZERO, 2.0, INK);
        assert_eq!(m.vertices[0].shape[0], Kind::Triangle as u32 as f32);
        assert_eq!(m.vertices[4].shape[0], Kind::Glow as u32 as f32);
        assert!(near(
            Vec2::from(m.vertices[4].local).abs().max_element(),
            2.0
        ));
    }

    #[test]
    fn soft_shapes_grow_their_quads_and_record_softness() {
        let mut m = Mesh::new(0.1);
        m.soft = 0.3;
        m.disc(Vec2::ZERO, 1.0, INK);
        m.band(&[Vec2::ZERO, Vec2::X], &[0.5; 2], INK);
        assert!(near(
            Vec2::from(m.vertices[0].local).abs().max_element(),
            1.4
        ));
        assert!(near(m.vertices[0].shape[3], 0.3));
        assert!(near(m.vertices[4].local[1].abs(), 0.9));
        assert!(near(m.vertices[4].shape[3], 0.3));
    }

    #[test]
    fn clear_keeps_feather() {
        let mut m = Mesh::new(0.25);
        m.disc(Vec2::ZERO, 1.0, INK);
        m.clear();
        assert!(m.vertices.is_empty() && m.indices.is_empty());
        assert_eq!(m.feather, 0.25);
    }
}
