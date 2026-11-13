use crate::color::{Palette, Rgba};
use crate::math::Vec2;
use crate::mesh::Mesh;
use crate::ribbon::{Class, End, Ribbon, Side};
use crate::traveler::{PAUSE, Phase, Span, Traveler, point_at};
use crate::world::{EndRef, World};
use std::f32::consts::FRAC_PI_2;

pub const SHADOW_OFFSET: Vec2 = Vec2::new(0.05, -0.07);
pub const SHADOW_SOFT: f32 = 0.12;
pub const SHADOW_ALPHA: f32 = 0.16;
pub const TETHER_HALF_WIDTH: f32 = 0.035;
pub const PULSE_RADIUS: f32 = 0.085;
pub const HALO_RADIUS: f32 = 0.6;
pub const HALO_ALPHA: f32 = 0.6;

pub fn draw_traveler(
    world: &World,
    traveler: &Traveler,
    palette: &Palette,
    time: f32,
    mesh: &mut Mesh,
) {
    for span in &traveler.tether {
        if (span.to - span.from).abs() < 1e-4 {
            continue;
        }
        let points = span_points(world, span);
        mesh.band(
            &points,
            &vec![TETHER_HALF_WIDTH; points.len()],
            palette.halo,
        );
    }
    let pulse = 1.0 + 0.08 * (time * 6.0).sin();
    mesh.disc(
        traveler.position(world),
        PULSE_RADIUS * pulse,
        palette.pulse,
    );
}

pub fn draw_halo(
    world: &World,
    traveler: &Traveler,
    palette: &Palette,
    time: f32,
    mesh: &mut Mesh,
) {
    let ready = match traveler.phase {
        Phase::Waiting { left } => (1.0 - left / PAUSE).clamp(0.0, 1.0),
        _ => 1.0,
    };
    let radius = HALO_RADIUS * (0.6 + 0.4 * ready) * (1.0 + 0.05 * (time * 6.0).sin());
    mesh.glow(
        traveler.position(world),
        radius,
        palette.halo.with_alpha(HALO_ALPHA),
    );
}

fn span_points(world: &World, span: &Span) -> Vec<Vec2> {
    let (a, b) = (span.from, span.to);
    let mut us = vec![a];
    if b > a {
        let mut k = a.floor() + 1.0;
        while k < b {
            us.push(k);
            k += 1.0;
        }
    } else {
        let mut k = a.ceil() - 1.0;
        while k > b {
            us.push(k);
            k -= 1.0;
        }
    }
    us.push(b);
    us.into_iter()
        .map(|u| point_at(world, span.ribbon, u))
        .collect()
}

pub fn class_color(palette: &Palette, class: Class) -> Rgba {
    match class {
        Class::Ink => palette.neutral,
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
    draw_shadows(world, palette, mesh);
    draw_ribbons(world, palette, time, mesh);
}

pub fn draw_shadows(world: &World, palette: &Palette, mesh: &mut Mesh) {
    let soft = mesh.soft;
    mesh.soft = SHADOW_SOFT;
    let shadow = palette.neutral.with_alpha(SHADOW_ALPHA);
    for r in &world.ribbons {
        let lifted: Vec<Vec2> = r.pos.iter().map(|&p| p + SHADOW_OFFSET).collect();
        mesh.band(&lifted, &half_widths(r), shadow);
    }
    mesh.soft = soft;
}

pub fn draw_ribbons(world: &World, palette: &Palette, time: f32, mesh: &mut Mesh) {
    for (ribbon, r) in world.ribbons.iter().enumerate() {
        mesh.band(&r.pos, &half_widths(r), palette.neutral);
        for side in [Side::Head, Side::Tail] {
            if world.partner(EndRef { ribbon, side }).is_none() {
                draw_end(r, side, palette, time, mesh);
            }
        }
    }
    for weld in &world.welds {
        let r = &world.ribbons[weld.a.ribbon];
        let color = r
            .end(weld.a.side)
            .class()
            .map_or(palette.neutral, |c| class_color(palette, c));
        mesh.disc(world.end_pos(weld.a), r.half_width * 1.1, color);
    }
}

fn draw_end(r: &Ribbon, side: Side, palette: &Palette, time: f32, mesh: &mut Mesh) {
    let p = r.end_pos(side);
    let out = r.outward(side);
    let hw = r.half_width;
    match r.end(side) {
        End::Source => mesh.disc(p, hw * 2.2, palette.neutral),
        End::Goal => mesh.ring(p, hw * 2.6, hw * 0.7, palette.neutral),
        End::Cap => {
            mesh.disc(p, hw * 1.5, palette.neutral);
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
        draw_world(&World::new(vec![r]), &Palette::video(), 0.0, &mut mesh);
        mesh
    }

    fn quads(mesh: &Mesh, kind: Kind, color: Rgba) -> Vec<Vec2> {
        let want = color.premultiplied();
        let hits: Vec<Vec2> = mesh
            .vertices
            .iter()
            .filter(|v| v.shape[0] == kind as u32 as f32 && v.color == want)
            .map(|v| Vec2::new(v.pos[0], v.pos[1]))
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
        let p = Palette::video();
        assert_eq!(class_color(&p, Class::Ink), p.neutral);
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
        let p = Palette::video();
        assert_eq!(band_vertices(&mesh, p.neutral), 42);
        assert_eq!(band_vertices(&mesh, p.neutral.with_alpha(SHADOW_ALPHA)), 42);
        let shadow = mesh
            .vertices
            .iter()
            .find(|v| v.color == p.neutral.with_alpha(SHADOW_ALPHA).premultiplied())
            .unwrap();
        assert_eq!(shadow.shape[3], SHADOW_SOFT);
    }

    #[test]
    fn source_is_a_disc_and_goal_is_a_ring() {
        let p = Palette::video();
        let mesh = draw(ribbon(End::Source, End::Goal));
        let source = quads(&mesh, Kind::Disc, p.neutral);
        assert_eq!(source.len(), 1);
        assert!(source[0].distance(Vec2::new(-1.0, 0.0)) < 1e-4);
        let goal = quads(&mesh, Kind::Ring, p.neutral);
        assert_eq!(goal.len(), 1);
        assert!(goal[0].distance(Vec2::new(1.0, 0.0)) < 1e-4);
    }

    #[test]
    fn dead_end_is_a_rivet() {
        let p = Palette::video();
        let mesh = draw(ribbon(End::Cap, End::Goal));
        assert_eq!(quads(&mesh, Kind::Disc, p.neutral).len(), 1);
        assert_eq!(quads(&mesh, Kind::Disc, p.ground).len(), 1);
    }

    #[test]
    fn open_ends_wear_their_class_cap_beyond_the_end() {
        let p = Palette::video();
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
        let p = Palette::video();
        let mesh = draw(ribbon(End::Source, End::Open(Class::Ink)));
        assert_eq!(band_vertices(&mesh, p.neutral), 42 + 4);
    }

    #[test]
    fn welded_ends_show_a_bead_and_no_caps() {
        use crate::world::{EndRef, Weld};
        let p = Palette::video();
        let a = Ribbon::new(
            &[Vec2::new(-1.0, 0.0), Vec2::ZERO],
            0.1,
            0.1,
            (End::Source, End::Open(Class::Red)),
            0.5,
        );
        let b = Ribbon::new(
            &[Vec2::ZERO, Vec2::new(1.0, 0.0)],
            0.1,
            0.1,
            (End::Open(Class::Red), End::Goal),
            0.5,
        );
        let mut world = World::new(vec![a, b]);
        world.welds.push(Weld {
            a: EndRef {
                ribbon: 0,
                side: Side::Tail,
            },
            b: EndRef {
                ribbon: 1,
                side: Side::Head,
            },
        });
        let mut mesh = Mesh::new(0.01);
        draw_world(&world, &p, 0.0, &mut mesh);
        assert!(quads(&mesh, Kind::Square, p.red).is_empty());
        let beads = quads(&mesh, Kind::Disc, p.red);
        assert_eq!(beads.len(), 1);
        assert!(beads[0].length() < 1e-4);
    }

    fn line_world() -> World {
        World::new(vec![Ribbon::new(
            &[Vec2::new(-2.0, 0.0), Vec2::new(2.0, 0.0)],
            0.1,
            0.1,
            (End::Source, End::Open(Class::Ink)),
            1.0,
        )])
    }

    #[test]
    fn traveler_draws_a_white_core_without_a_halo() {
        let p = Palette::video();
        let world = line_world();
        let traveler = Traveler::new(&world).unwrap();
        let mut mesh = Mesh::new(0.01);
        draw_traveler(&world, &traveler, &p, 0.0, &mut mesh);
        let core = quads(&mesh, Kind::Disc, p.pulse);
        assert_eq!(core.len(), 1);
        assert!(core[0].distance(Vec2::new(-2.0, 0.0)) < 1e-4);
        assert!(quads(&mesh, Kind::Glow, p.halo.with_alpha(HALO_ALPHA)).is_empty());
    }

    #[test]
    fn halo_glows_at_the_traveler() {
        let p = Palette::video();
        let world = line_world();
        let traveler = Traveler::new(&world).unwrap();
        let mut mesh = Mesh::new(0.01);
        draw_halo(&world, &traveler, &p, 0.0, &mut mesh);
        let glow = quads(&mesh, Kind::Glow, p.halo.with_alpha(HALO_ALPHA));
        assert_eq!(glow.len(), 1);
        assert!(glow[0].distance(Vec2::new(-2.0, 0.0)) < 1e-4);
    }

    #[test]
    fn world_draws_shadows_before_ribbons() {
        let p = Palette::video();
        let world = line_world();
        let mut shadows = Mesh::new(0.01);
        draw_shadows(&world, &p, &mut shadows);
        let mut ribbons = Mesh::new(0.01);
        draw_ribbons(&world, &p, 0.0, &mut ribbons);
        let mut both = Mesh::new(0.01);
        draw_world(&world, &p, 0.0, &mut both);
        assert_eq!(
            both.vertices.len(),
            shadows.vertices.len() + ribbons.vertices.len()
        );
        assert_eq!(
            both.vertices[..shadows.vertices.len()],
            shadows.vertices[..]
        );
    }

    #[test]
    fn tether_follows_each_traveled_span() {
        use crate::traveler::Span;
        let p = Palette::video();
        let world = line_world();
        let mut traveler = Traveler::new(&world).unwrap();
        traveler.tether = vec![Span {
            ribbon: 0,
            from: 0.0,
            to: 10.5,
        }];
        traveler.u = 10.5;
        let mut mesh = Mesh::new(0.01);
        draw_traveler(&world, &traveler, &p, 0.0, &mut mesh);
        let line: Vec<&crate::mesh::Vertex> = mesh
            .vertices
            .iter()
            .filter(|v| v.shape[0] == Kind::Band as u32 as f32 && v.color == p.halo.premultiplied())
            .collect();
        assert_eq!(line.len(), 2 * 12);
        assert!((line.last().unwrap().local[0] - 1.05).abs() < 1e-4);
        assert!((line[0].shape[1] - TETHER_HALF_WIDTH).abs() < 1e-6);
    }
}
