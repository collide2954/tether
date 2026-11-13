use crate::color::{Palette, Rgba};
use crate::math::Vec2;
use crate::mesh::Mesh;
use crate::ribbon::{Class, End, Ribbon, Side};
use crate::world::World;
use std::f32::consts::FRAC_PI_2;

pub const SHADOW_OFFSET: Vec2 = Vec2::new(0.05, -0.07);
pub const SHADOW_SOFT: f32 = 0.12;
pub const SHADOW_ALPHA: f32 = 0.16;

pub fn class_color(palette: &Palette, class: Class) -> Rgba {
    match class {
        Class::Ink => palette.ink,
        Class::Yellow => palette.yellow,
        Class::Red => palette.red,
        Class::Blue => palette.blue,
    }
}

pub fn half_widths(r: &Ribbon) -> Vec<f32> {
    let n = r.len();
    let strain: Vec<f32> = (0..n.saturating_sub(1))
        .map(|i| r.segment_strain(i).max(1.0))
        .collect();
    (0..n)
        .map(|i| {
            let before = i.checked_sub(1).and_then(|k| strain.get(k));
            let s = match (before, strain.get(i)) {
                (Some(a), Some(b)) => 0.5 * (a + b),
                (Some(a), None) | (None, Some(a)) => *a,
                (None, None) => 1.0,
            };
            r.half_width / s.sqrt()
        })
        .collect()
}

pub fn draw_world(world: &World, palette: &Palette, time: f32, mesh: &mut Mesh) {
    let soft = mesh.soft;
    mesh.soft = SHADOW_SOFT;
    let shadow = palette.ink.with_alpha(SHADOW_ALPHA);
    for r in &world.ribbons {
        let lifted: Vec<Vec2> = r.pos.iter().map(|&p| p + SHADOW_OFFSET).collect();
        mesh.band(&lifted, &half_widths(r), shadow);
    }
    mesh.soft = soft;
    for r in &world.ribbons {
        mesh.band(&r.pos, &half_widths(r), palette.ink);
        for side in [Side::Head, Side::Tail] {
            draw_end(r, side, palette, time, mesh);
        }
    }
}

fn draw_end(r: &Ribbon, side: Side, palette: &Palette, time: f32, mesh: &mut Mesh) {
    let p = r.end_pos(side);
    let out = r.outward(side);
    let hw = r.half_width;
    match r.end(side) {
        End::Source => mesh.disc(p, hw * 2.2, palette.ink),
        End::Goal => mesh.ring(p, hw * 2.6, hw * 0.7, palette.ink),
        End::Cap => {
            mesh.disc(p, hw * 1.5, palette.ink);
            mesh.disc(p, hw * 0.55, palette.ground);
        }
        End::Open(class) => {
            let size = hw * 1.7 * (1.0 + 0.06 * (time * 2.4).sin());
            let color = class_color(palette, class);
            let angle = out.y.atan2(out.x);
            let center = p + out * size * 0.6;
            match class {
                Class::Ink => {
                    let across = out.perp() * size;
                    let c = p + out * hw * 0.3;
                    mesh.band(&[c - across, c + across], &[hw * 0.5; 2], color);
                }
                Class::Yellow => mesh.triangle(center, size * 1.15, angle - FRAC_PI_2, color),
                Class::Red => mesh.square(center, size * 0.75, angle, color),
                Class::Blue => mesh.disc(center, size * 0.85, color),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Kind;

    fn ribbon(head: End, tail: End) -> Ribbon {
        Ribbon::new(
            &[Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0)],
            0.1,
            0.1,
            (head, tail),
            0.5,
        )
    }

    fn draw(r: Ribbon) -> Mesh {
        let mut mesh = Mesh::new(0.01);
        draw_world(&World::new(vec![r]), &Palette::paper(), 0.0, &mut mesh);
        mesh
    }

    fn quads(mesh: &Mesh, kind: Kind, color: Rgba) -> Vec<Vec2> {
        let want = color.premultiplied();
        let hits: Vec<Vec2> = mesh
            .vertices
            .iter()
            .filter(|v| v.shape[0] == kind as u32 as f32 && v.color == want)
            .map(|v| Vec2::from(v.pos))
            .collect();
        hits.chunks(4)
            .map(|c| c.iter().sum::<Vec2>() / 4.0)
            .collect()
    }

    fn band_vertices(mesh: &Mesh, color: Rgba) -> usize {
        let want = color.premultiplied();
        mesh.vertices
            .iter()
            .filter(|v| v.shape[0] == Kind::Band as u32 as f32 && v.color == want)
            .count()
    }

    #[test]
    fn classes_use_palette_colors() {
        let p = Palette::paper();
        assert_eq!(class_color(&p, Class::Ink), p.ink);
        assert_eq!(class_color(&p, Class::Yellow), p.yellow);
        assert_eq!(class_color(&p, Class::Red), p.red);
        assert_eq!(class_color(&p, Class::Blue), p.blue);
    }

    #[test]
    fn half_widths_thin_where_the_ribbon_stretches() {
        let mut r = ribbon(End::Source, End::Goal);
        assert!(half_widths(&r).iter().all(|&w| (w - 0.1).abs() < 1e-6));
        r.pos[20].x += 0.3;
        let w = half_widths(&r);
        assert!(w[20] < 0.08);
        assert!((w[5] - 0.1).abs() < 1e-6);
    }

    #[test]
    fn ribbon_draws_a_soft_shadow_and_an_ink_band() {
        let mesh = draw(ribbon(End::Source, End::Goal));
        let p = Palette::paper();
        assert_eq!(band_vertices(&mesh, p.ink), 42);
        assert_eq!(band_vertices(&mesh, p.ink.with_alpha(SHADOW_ALPHA)), 42);
        let shadow = mesh
            .vertices
            .iter()
            .find(|v| v.color == p.ink.with_alpha(SHADOW_ALPHA).premultiplied())
            .unwrap();
        assert_eq!(shadow.shape[3], SHADOW_SOFT);
    }

    #[test]
    fn source_is_a_disc_and_goal_is_a_ring() {
        let p = Palette::paper();
        let mesh = draw(ribbon(End::Source, End::Goal));
        let source = quads(&mesh, Kind::Disc, p.ink);
        assert_eq!(source.len(), 1);
        assert!(source[0].distance(Vec2::new(-1.0, 0.0)) < 1e-4);
        let goal = quads(&mesh, Kind::Ring, p.ink);
        assert_eq!(goal.len(), 1);
        assert!(goal[0].distance(Vec2::new(1.0, 0.0)) < 1e-4);
    }

    #[test]
    fn dead_end_is_a_rivet() {
        let p = Palette::paper();
        let mesh = draw(ribbon(End::Cap, End::Goal));
        assert_eq!(quads(&mesh, Kind::Disc, p.ink).len(), 1);
        assert_eq!(quads(&mesh, Kind::Disc, p.ground).len(), 1);
    }

    #[test]
    fn open_ends_wear_their_class_cap_beyond_the_end() {
        let p = Palette::paper();
        let cases = [
            (Class::Red, Kind::Square, p.red),
            (Class::Yellow, Kind::Triangle, p.yellow),
            (Class::Blue, Kind::Disc, p.blue),
        ];
        for (class, kind, color) in cases {
            let mesh = draw(ribbon(End::Source, End::Open(class)));
            let caps = quads(&mesh, kind, color);
            assert_eq!(caps.len(), 1, "{class:?}");
            assert!(caps[0].x > 1.0 && caps[0].y.abs() < 1e-4, "{class:?}");
        }
    }

    #[test]
    fn ink_open_end_is_a_flat_bar_across_the_end() {
        let p = Palette::paper();
        let mesh = draw(ribbon(End::Source, End::Open(Class::Ink)));
        assert_eq!(band_vertices(&mesh, p.ink), 42 + 4);
    }
}
