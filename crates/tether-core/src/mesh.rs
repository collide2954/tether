use crate::color::Rgba;
use crate::math::Vec2;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
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
    Solid = 6,
    Rect = 7,
}

const MITER_LIMIT: f32 = 2.5;

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub feather: f32,
    pub soft: f32,
    pub z: f32,
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
                    pos: (points[i] + normal * reach * side).extend(self.z).into(),
                    local: [along, reach * side],
                    color,
                    shape: [Kind::Band as u32 as f32, half_widths[i], 0.0, self.soft],
                });
            }
        }
        self.strip_indices(base, n);
    }

    pub fn strip(&mut self, points: &[Vec3], sides: &[Vec3], half_widths: &[f32], colors: &[Rgba]) {
        let n = points
            .len()
            .min(sides.len())
            .min(half_widths.len())
            .min(colors.len());
        if n < 2 {
            return;
        }
        let base = self.vertices.len() as u32;
        let pad = self.feather + self.soft;
        let mut along = 0.0;
        for i in 0..n {
            if i > 0 {
                along += points[i].distance(points[i - 1]);
            }
            let reach = half_widths[i] + pad;
            let color = colors[i].premultiplied();
            for side in [1.0, -1.0] {
                self.vertices.push(Vertex {
                    pos: (points[i] + sides[i] * reach * side).into(),
                    local: [along, reach * side],
                    color,
                    shape: [Kind::Band as u32 as f32, half_widths[i], 0.0, self.soft],
                });
            }
        }
        self.strip_indices(base, n);
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

    pub fn quad3(
        &mut self,
        center: Vec3,
        u: Vec3,
        v: Vec3,
        half: f32,
        color: Rgba,
        shape: (Kind, f32, f32),
    ) {
        let shape = [shape.0 as u32 as f32, shape.1, shape.2, self.soft];
        self.quad_extents(center, (u, v), Vec2::splat(half), color, shape);
    }

    pub fn rect(&mut self, center: Vec3, u: Vec3, v: Vec3, half: Vec2, color: Rgba) {
        let pad = Vec2::splat(self.feather + self.soft);
        let shape = [Kind::Rect as u32 as f32, half.x, half.y, self.soft];
        self.quad_extents(center, (u, v), half + pad, color, shape);
    }

    pub fn solid(&mut self, points: &[Vec3], color: Rgba) {
        if points.len() < 3 {
            return;
        }
        let base = self.vertices.len() as u32;
        let color = color.premultiplied();
        for p in points {
            self.vertices.push(Vertex {
                pos: (*p).into(),
                local: [0.0, 0.0],
                color,
                shape: [Kind::Solid as u32 as f32, 0.0, 0.0, 0.0],
            });
        }
        for k in 1..points.len() as u32 - 1 {
            self.indices
                .extend_from_slice(&[base, base + k, base + k + 1]);
        }
    }

    fn quad(&mut self, center: Vec2, half: f32, angle: f32, color: Rgba, shape: (Kind, f32, f32)) {
        let (sin, cos) = angle.sin_cos();
        let u = Vec3::new(cos, sin, 0.0);
        let v = Vec3::new(-sin, cos, 0.0);
        self.quad3(center.extend(self.z), u, v, half, color, shape);
    }

    fn quad_extents(
        &mut self,
        center: Vec3,
        axes: (Vec3, Vec3),
        half: Vec2,
        color: Rgba,
        shape: [f32; 4],
    ) {
        let base = self.vertices.len() as u32;
        let color = color.premultiplied();
        for corner in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let local = Vec2::new(corner.0 * half.x, corner.1 * half.y);
            self.vertices.push(Vertex {
                pos: (center + axes.0 * local.x + axes.1 * local.y).into(),
                local: local.into(),
                color,
                shape,
            });
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn strip_indices(&mut self, base: u32, n: usize) {
        for i in 0..n as u32 - 1 {
            let a = base + 2 * i;
            self.indices
                .extend_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
        }
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
        Vec2::new(v.pos[0], v.pos[1])
    }

    fn pos3(v: &Vertex) -> Vec3 {
        Vec3::from(v.pos)
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

    #[test]
    fn flat_shapes_sit_at_the_mesh_height() {
        let mut m = Mesh::new(0.1);
        m.z = 0.3;
        m.disc(Vec2::ZERO, 1.0, INK);
        m.band(&[Vec2::ZERO, Vec2::X], &[0.5; 2], INK);
        assert!(m.vertices.iter().all(|v| near(v.pos[2], 0.3)));
    }

    #[test]
    fn strip_offsets_along_its_side_vectors_with_point_colors() {
        let mut m = Mesh::new(0.0);
        let red = Rgba::new(1.0, 0.0, 0.0, 1.0);
        m.strip(
            &[Vec3::ZERO, Vec3::X],
            &[Vec3::Z, Vec3::Z],
            &[0.5, 0.25],
            &[INK, red],
        );
        assert_eq!((m.vertices.len(), m.indices.len()), (4, 6));
        assert!(pos3(&m.vertices[0]).distance(Vec3::new(0.0, 0.0, 0.5)) < 1e-5);
        assert!(pos3(&m.vertices[1]).distance(Vec3::new(0.0, 0.0, -0.5)) < 1e-5);
        assert!(pos3(&m.vertices[2]).distance(Vec3::new(1.0, 0.0, 0.25)) < 1e-5);
        assert_eq!(m.vertices[3].color, red.premultiplied());
        assert!(near(m.vertices[2].local[0], 1.0));
        assert!(near(m.vertices[2].shape[1], 0.25));
    }

    #[test]
    fn quad3_spans_its_two_axes() {
        let mut m = Mesh::new(0.0);
        m.quad3(
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::X,
            Vec3::Z,
            1.0,
            INK,
            (Kind::Disc, 1.0, 0.0),
        );
        let ps: Vec<Vec3> = m.vertices.iter().map(pos3).collect();
        assert!(ps.iter().all(|p| p.y.abs() < 1e-6));
        assert!(
            ps.iter()
                .any(|p| p.distance(Vec3::new(1.0, 0.0, 2.0)) < 1e-5)
        );
        assert!(
            ps.iter()
                .any(|p| p.distance(Vec3::new(-1.0, 0.0, 0.0)) < 1e-5)
        );
        assert!(near(
            Vec2::from(m.vertices[0].local).abs().max_element(),
            1.0
        ));
        assert_eq!(m.vertices[0].shape[0], Kind::Disc as u32 as f32);
    }

    #[test]
    fn solid_fans_a_convex_polygon() {
        let mut m = Mesh::new(0.1);
        let square = [Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y];
        m.solid(&square, INK);
        assert_eq!(m.vertices.len(), 4);
        assert_eq!(m.indices, vec![0, 1, 2, 0, 2, 3]);
        assert!(
            m.vertices
                .iter()
                .all(|v| v.shape[0] == Kind::Solid as u32 as f32)
        );
    }

    #[test]
    fn rect_has_two_half_extents() {
        let mut m = Mesh::new(0.1);
        m.rect(Vec3::ZERO, Vec3::X, Vec3::Y, Vec2::new(2.0, 0.5), INK);
        assert_eq!(m.vertices[0].shape[0], Kind::Rect as u32 as f32);
        assert!(near(m.vertices[0].shape[1], 2.0) && near(m.vertices[0].shape[2], 0.5));
        assert!(
            m.vertices
                .iter()
                .all(|v| near(v.local[0].abs(), 2.1) && near(v.local[1].abs(), 0.6))
        );
    }
}
