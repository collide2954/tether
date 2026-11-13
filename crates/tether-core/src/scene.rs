use crate::color::{Palette, Rgba};
use crate::game::Game;
use crate::look::Look;
use crate::math::{Vec2, Vec3, smoothstep};
use crate::mesh::{Kind, Mesh};
use crate::ribbon::{Class, End, Ribbon, Side};
use crate::traveler::{PAUSE, Phase, Span, Traveler, point_at};
use crate::view::View;
use crate::world::{EndRef, World};

pub const BASE_Z: f32 = 0.03;
pub const LIFT_HEIGHT: f32 = 0.9;
pub const TWIST: f32 = 1.1;
pub const SHADOW_ALPHA: f32 = 0.55;
pub const SHADOW_SOFT: f32 = 0.06;
pub const TETHER_HALF_WIDTH: f32 = 0.035;
pub const PULSE_RADIUS: f32 = 0.085;
pub const PULSE_HOVER: f32 = 0.11;
pub const HALO_RADIUS: f32 = 0.7;

pub fn light() -> Vec3 {
    Vec3::new(-0.45, 0.35, 0.82).normalize()
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

pub fn end_color(palette: &Palette, r: &Ribbon, side: Side) -> Rgba {
    match r.end(side).class().or(r.end(side.other()).class()) {
        Some(class) => class_color(palette, class),
        None => palette.neutral,
    }
}

pub fn ribbon_colors(r: &Ribbon, palette: &Palette) -> Vec<Rgba> {
    let head = end_color(palette, r, Side::Head);
    let tail = end_color(palette, r, Side::Tail);
    let last = r.len().saturating_sub(1).max(1) as f32;
    (0..r.len())
        .map(|i| match smoothstep(0.35, 0.65, i as f32 / last) {
            s if s <= 0.0 => head,
            s if s >= 1.0 => tail,
            s => head.lerp(tail, s),
        })
        .collect()
}

pub fn heights(r: &Ribbon, lift: [f32; 2]) -> Vec<f32> {
    let n = r.len() as f32;
    (0..r.len())
        .map(|i| {
            let free = (1.0 - r.stiffness[i]).powi(2);
            let head = smoothstep(0.65 * n, 0.35 * n, i as f32);
            BASE_Z + LIFT_HEIGHT * free * (lift[0] * head + lift[1] * (1.0 - head))
        })
        .collect()
}

pub fn sides(r: &Ribbon, heights: &[f32]) -> Vec<Vec3> {
    let n = r.len();
    (0..n)
        .map(|i| {
            let tangent =
                (r.pos[(i + 1).min(n - 1)] - r.pos[i.saturating_sub(1)]).normalize_or(Vec2::X);
            let angle = TWIST * ((heights[i] - BASE_Z) / LIFT_HEIGHT).clamp(0.0, 1.0);
            (tangent.perp() * angle.cos()).extend(angle.sin())
        })
        .collect()
}

pub fn shade(normal: Vec3) -> f32 {
    0.62 + 0.38 * normal.normalize_or_zero().dot(light()).abs()
}

pub fn shadow_offset(z: f32) -> Vec2 {
    let l = light();
    -l.truncate() / l.z * z
}

pub fn draw(game: &Game, look: &Look, view: &View, palette: &Palette, mesh: &mut Mesh) {
    let world = &game.world;
    let raised: Vec<Vec<f32>> = world
        .ribbons
        .iter()
        .enumerate()
        .map(|(k, r)| heights(r, look.lifts.get(k).copied().unwrap_or([0.0; 2])))
        .collect();
    draw_shadows(world, &raised, mesh);
    draw_ribbons(world, &raised, palette, game.time, mesh);
    if let Some(traveler) = &game.traveler {
        draw_tether(world, &raised, traveler, palette, mesh);
        draw_pulse(world, &raised, traveler, view, palette, game.time, mesh);
    }
    draw_veil(view, palette, game.veil(), mesh);
}

fn flat(mesh: &mut Mesh, center: Vec3, axes: (Vec3, Vec3), shape: (Kind, f32, f32), color: Rgba) {
    let reach = shape.1 + 0.5 * shape.2 + mesh.feather + mesh.soft;
    mesh.quad3(center, axes.0, axes.1, reach, color, shape);
}

fn draw_shadows(world: &World, raised: &[Vec<f32>], mesh: &mut Mesh) {
    let (soft, z) = (mesh.soft, mesh.z);
    mesh.soft = SHADOW_SOFT;
    mesh.z = 0.001;
    let shadow = Rgba::new(0.0, 0.0, 0.0, SHADOW_ALPHA);
    for (r, h) in world.ribbons.iter().zip(raised) {
        let points: Vec<Vec2> = r
            .pos
            .iter()
            .zip(h)
            .map(|(&p, &z)| p + shadow_offset(z))
            .collect();
        let widths: Vec<f32> = half_widths(r)
            .iter()
            .zip(h)
            .map(|(w, z)| w * (1.0 + 0.6 * z))
            .collect();
        mesh.band(&points, &widths, shadow);
    }
    mesh.soft = soft;
    mesh.z = z;
}

fn draw_ribbons(world: &World, raised: &[Vec<f32>], palette: &Palette, time: f32, mesh: &mut Mesh) {
    for (ribbon, (r, h)) in world.ribbons.iter().zip(raised).enumerate() {
        let points: Vec<Vec3> = r.pos.iter().zip(h).map(|(p, z)| p.extend(*z)).collect();
        let across = sides(r, h);
        let n = points.len();
        let lit: Vec<Rgba> = ribbon_colors(r, palette)
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let tangent = (points[(i + 1).min(n - 1)] - points[i.saturating_sub(1)])
                    .normalize_or(Vec3::X);
                let s = shade(tangent.cross(across[i]));
                Rgba::new(c.r * s, c.g * s, c.b * s, c.a)
            })
            .collect();
        mesh.strip(&points, &across, &half_widths(r), &lit);
        for side in [Side::Head, Side::Tail] {
            if world.partner(EndRef { ribbon, side }).is_none() {
                draw_end(r, side, (&points, &across), palette, time, mesh);
            }
        }
    }
    for weld in &world.welds {
        let r = &world.ribbons[weld.a.ribbon];
        let i = r.index(weld.a.side);
        let at = r.pos[i].extend(raised[weld.a.ribbon][i] + 0.004);
        let color = end_color(palette, r, weld.a.side);
        flat(
            mesh,
            at,
            (Vec3::X, Vec3::Y),
            (Kind::Disc, r.half_width * 1.1, 0.0),
            color,
        );
    }
}

fn draw_end(
    r: &Ribbon,
    side: Side,
    frame: (&[Vec3], &[Vec3]),
    palette: &Palette,
    time: f32,
    mesh: &mut Mesh,
) {
    let (points, across) = frame;
    let i = r.index(side);
    let inner = if side == Side::Head { 1 } else { r.len() - 2 };
    let at = points[i] + Vec3::Z * 0.003;
    let out = (points[i] - points[inner]).normalize_or(Vec3::X);
    let width = across[i];
    let hw = r.half_width;
    let plane = (Vec3::X, Vec3::Y);
    match r.end(side) {
        End::Source => flat(
            mesh,
            at,
            plane,
            (Kind::Disc, hw * 2.2, 0.0),
            palette.neutral,
        ),
        End::Goal => {
            flat(mesh, at, plane, (Kind::Disc, hw * 3.2, 0.0), palette.blue);
            let ring = (Kind::Ring, hw * 2.1, hw * 0.75);
            flat(mesh, at + Vec3::Z * 0.001, plane, ring, palette.neutral);
            let eye = (Kind::Disc, hw * 0.85, 0.0);
            flat(mesh, at + Vec3::Z * 0.002, plane, eye, palette.red);
        }
        End::Cap => {
            flat(
                mesh,
                at,
                plane,
                (Kind::Disc, hw * 1.5, 0.0),
                end_color(palette, r, side),
            );
            let hole = (Kind::Disc, hw * 0.55, 0.0);
            flat(mesh, at + Vec3::Z * 0.001, plane, hole, palette.ground);
        }
        End::Open(class) => {
            let size = hw * 1.7 * (1.0 + 0.06 * (time * 2.4).sin());
            let color = class_color(palette, class);
            let center = at + out * size * 0.6;
            match class {
                Class::Ink => {
                    let c = at + out * hw * 0.3;
                    let bar = [c - width * size, c + width * size];
                    mesh.strip(&bar, &[out, out], &[hw * 0.5; 2], &[color; 2]);
                }
                Class::Yellow => flat(
                    mesh,
                    center,
                    (width, out),
                    (Kind::Triangle, size * 1.15, 0.0),
                    color,
                ),
                Class::Red => flat(
                    mesh,
                    center,
                    (out, width),
                    (Kind::Square, size * 0.75, 0.0),
                    color,
                ),
                Class::Blue => flat(
                    mesh,
                    center,
                    (out, width),
                    (Kind::Disc, size * 0.85, 0.0),
                    color,
                ),
            }
        }
    }
}

fn span_us(span: &Span) -> Vec<f32> {
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
    us
}

fn sample<T: Copy + std::ops::Add<Output = T> + std::ops::Mul<f32, Output = T>>(
    values: &[T],
    u: f32,
) -> T {
    let last = values.len() - 1;
    let u = u.clamp(0.0, last as f32);
    let k = (u.floor() as usize).min(last.saturating_sub(1));
    let t = u - k as f32;
    values[k] * (1.0 - t) + values[(k + 1).min(last)] * t
}

fn draw_tether(
    world: &World,
    raised: &[Vec<f32>],
    traveler: &Traveler,
    palette: &Palette,
    mesh: &mut Mesh,
) {
    let h = palette.halo;
    let color = Rgba::new(h.r * 1.4, h.g * 1.4, h.b * 1.4, 1.0);
    for span in &traveler.tether {
        if (span.to - span.from).abs() < 1e-4 {
            continue;
        }
        let heights = &raised[span.ribbon];
        let across = sides(&world.ribbons[span.ribbon], heights);
        let us = span_us(span);
        let points: Vec<Vec3> = us
            .iter()
            .map(|&u| point_at(world, span.ribbon, u).extend(sample(heights, u) + 0.006))
            .collect();
        let widths: Vec<Vec3> = us
            .iter()
            .map(|&u| sample(&across, u).normalize_or(Vec3::Y))
            .collect();
        let n = points.len();
        mesh.strip(
            &points,
            &widths,
            &vec![TETHER_HALF_WIDTH; n],
            &vec![color; n],
        );
    }
}

fn draw_pulse(
    world: &World,
    raised: &[Vec<f32>],
    traveler: &Traveler,
    view: &View,
    palette: &Palette,
    time: f32,
    mesh: &mut Mesh,
) {
    let z = sample(&raised[traveler.ribbon], traveler.u) + PULSE_HOVER;
    let at = traveler.position(world).extend(z);
    let (right, up) = view.basis();
    let ready = match traveler.phase {
        Phase::Waiting { left } => (1.0 - left / PAUSE).clamp(0.0, 1.0),
        _ => 1.0,
    };
    let breath = 1.0 + 0.05 * (time * 6.0).sin();
    let glow = HALO_RADIUS * (0.6 + 0.4 * ready) * breath;
    let h = palette.halo;
    let light = Rgba::new(h.r * 2.2, h.g * 2.2, h.b * 2.2, 1.0);
    mesh.quad3(at, right, up, glow, light, (Kind::Glow, glow, 0.0));
    let core = PULSE_RADIUS * (1.0 + 0.08 * (time * 6.0).sin());
    flat(
        mesh,
        at,
        (right, up),
        (Kind::Disc, core, 0.0),
        palette.pulse,
    );
}

fn draw_veil(view: &View, palette: &Palette, veil: f32, mesh: &mut Mesh) {
    if veil <= 0.0 {
        return;
    }
    let eye = view.eye();
    let ahead = (view.target.extend(0.0) - eye).normalize();
    let (right, up) = view.basis();
    let center = eye + ahead;
    let corners = [
        center - right * 4.0 - up * 4.0,
        center + right * 4.0 - up * 4.0,
        center + right * 4.0 + up * 4.0,
        center - right * 4.0 + up * 4.0,
    ];
    mesh.solid(&corners, palette.ground.with_alpha(veil));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Level;
    use crate::world::Weld;

    fn ribbon(head: End, tail: End) -> Ribbon {
        Ribbon::new(
            &[Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0)],
            0.1,
            0.1,
            (head, tail),
            0.5,
        )
    }

    fn game(text: &str) -> Game {
        Game::with_world(Level::parse(text).unwrap().build(), 0)
    }

    fn render(game: &Game, look: &Look) -> Mesh {
        let mut mesh = Mesh::new(0.01);
        let view = View::new(Vec2::new(1280.0, 720.0));
        draw(game, look, &view, &Palette::video(), &mut mesh);
        mesh
    }

    fn quads(mesh: &Mesh, kind: Kind, color: Rgba) -> Vec<Vec3> {
        let want = color.premultiplied();
        let hits: Vec<Vec3> = mesh
            .vertices
            .iter()
            .filter(|v| v.shape[0] == kind as u32 as f32 && v.color == want)
            .map(|v| Vec3::from(v.pos))
            .collect();
        hits.chunks(4)
            .map(|c| c.iter().sum::<Vec3>() / 4.0)
            .collect()
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    const LINE: &str = "(ribbons: [(points: [(-2.0, 0.0), (2.0, 0.0)], head: Source, tail: Goal)])";

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
    fn ribbons_take_the_color_of_their_classes() {
        let p = Palette::video();
        let red = ribbon(End::Source, End::Open(Class::Red));
        assert_eq!(end_color(&p, &red, Side::Head), p.red);
        assert!(ribbon_colors(&red, &p).iter().all(|c| *c == p.red));
        let plain = ribbon(End::Source, End::Goal);
        assert!(ribbon_colors(&plain, &p).iter().all(|c| *c == p.neutral));
        let both = ribbon(End::Open(Class::Yellow), End::Open(Class::Blue));
        let colors = ribbon_colors(&both, &p);
        assert_eq!(colors[0], p.yellow);
        assert_eq!(colors[20], p.blue);
        assert!(colors[10] != p.yellow && colors[10] != p.blue);
    }

    #[test]
    fn heights_rise_toward_the_lifted_end() {
        let r = ribbon(End::Source, End::Open(Class::Ink));
        assert!(heights(&r, [0.0, 0.0]).iter().all(|&h| near(h, BASE_Z)));
        let h = heights(&r, [0.0, 1.0]);
        assert!(near(h[0], BASE_Z));
        assert!(near(h[20], BASE_Z + LIFT_HEIGHT));
        assert!(h.windows(2).all(|w| w[1] >= w[0] - 1e-6));
        let head = heights(&ribbon(End::Open(Class::Ink), End::Goal), [1.0, 0.0]);
        assert!(head[0] > head[20]);
    }

    #[test]
    fn sides_lie_flat_and_tilt_when_lifted() {
        let r = ribbon(End::Source, End::Open(Class::Ink));
        let flat = sides(&r, &heights(&r, [0.0, 0.0]));
        for s in &flat {
            assert!(near(s.length(), 1.0) && near(s.z, 0.0) && near(s.x, 0.0));
        }
        let up = sides(&r, &heights(&r, [0.0, 1.0]));
        assert!(near(up[20].length(), 1.0));
        assert!(up[20].z > 0.5);
        assert!(near(up[0].z, 0.0));
    }

    #[test]
    fn shade_is_brightest_toward_the_light() {
        assert!(near(shade(light()), 1.0));
        assert!(near(shade(-light()), 1.0));
        let across = light().cross(Vec3::X).normalize();
        assert!(near(shade(across), 0.62));
    }

    #[test]
    fn shadows_fall_to_the_lower_right_and_grow_with_height() {
        assert_eq!(shadow_offset(0.0), Vec2::ZERO);
        let one = shadow_offset(1.0);
        assert!(one.x > 0.0 && one.y < 0.0);
        assert!((shadow_offset(2.0) - one * 2.0).length() < 1e-5);
    }

    #[test]
    fn each_ribbon_draws_a_shadow_and_a_lit_strip() {
        let g = game(LINE);
        let mesh = render(&g, &Look::default());
        let bands = mesh
            .vertices
            .iter()
            .filter(|v| v.shape[0] == Kind::Band as u32 as f32)
            .count();
        assert!(bands >= 2 * 2 * g.world.ribbons[0].len());
    }

    #[test]
    fn goal_is_a_target() {
        let p = Palette::video();
        let g = game(LINE);
        let mesh = render(&g, &Look::default());
        let goal = Vec2::new(2.0, 0.0);
        let base = quads(&mesh, Kind::Disc, p.blue);
        let ring = quads(&mesh, Kind::Ring, p.neutral);
        let eye = quads(&mesh, Kind::Disc, p.red);
        for found in [base, ring, eye] {
            assert_eq!(found.len(), 1);
            assert!(found[0].truncate().distance(goal) < 1e-3);
        }
    }

    #[test]
    fn welded_ends_show_a_bead_and_no_caps() {
        let p = Palette::video();
        let mut g = game(
            "(ribbons: [
                (points: [(-1.0, 0.0), (0.0, 0.0)], head: Source, tail: Open(Red)),
                (points: [(0.0, 0.0), (1.0, 0.0)], head: Open(Red), tail: Goal),
            ])",
        );
        g.world.welds.push(Weld {
            a: EndRef {
                ribbon: 0,
                side: Side::Tail,
            },
            b: EndRef {
                ribbon: 1,
                side: Side::Head,
            },
        });
        let mesh = render(&g, &Look::default());
        assert!(quads(&mesh, Kind::Square, p.red).is_empty());
        let beads = quads(&mesh, Kind::Disc, p.red);
        assert!(beads.iter().any(|b| b.truncate().length() < 1e-3));
    }

    #[test]
    fn open_ends_wear_their_class_cap() {
        let p = Palette::video();
        let cases = [
            ("Red", Kind::Square, p.red),
            ("Yellow", Kind::Triangle, p.yellow),
            ("Blue", Kind::Disc, p.blue),
        ];
        for (class, kind, color) in cases {
            let text = format!(
                "(ribbons: [(points: [(-2.0, 0.0), (2.0, 0.0)], head: Source, tail: Open({class}))])"
            );
            let caps = quads(&render(&game(&text), &Look::default()), kind, color);
            assert!(
                caps.iter().any(|c| c.x > 2.0 && c.y.abs() < 1e-3),
                "{class}"
            );
        }
    }

    #[test]
    fn pulse_glows_with_a_bright_core_above_the_ribbon() {
        let p = Palette::video();
        let g = game(LINE);
        let mesh = render(&g, &Look::default());
        let core = quads(&mesh, Kind::Disc, p.pulse);
        assert_eq!(core.len(), 1);
        assert!(core[0].z > BASE_Z);
        assert!(core[0].truncate().distance(Vec2::new(-2.0, 0.0)) < 1e-3);
        let glows = mesh
            .vertices
            .iter()
            .filter(|v| v.shape[0] == Kind::Glow as u32 as f32)
            .count();
        assert!(glows >= 4);
    }

    #[test]
    fn tether_follows_each_traveled_span() {
        let mut g = game(LINE);
        let t = g.traveler.as_mut().unwrap();
        t.tether = vec![Span {
            ribbon: 0,
            from: 0.0,
            to: 10.5,
        }];
        t.u = 10.5;
        let mesh = render(&g, &Look::default());
        let tether = mesh
            .vertices
            .iter()
            .filter(|v| {
                v.shape[0] == Kind::Band as u32 as f32 && near(v.shape[1], TETHER_HALF_WIDTH)
            })
            .count();
        assert_eq!(tether, 2 * 12);
    }

    #[test]
    fn veil_covers_the_view_last() {
        let g = game(LINE);
        let mesh = render(&g, &Look::default());
        let last = mesh.vertices.last().unwrap();
        assert_eq!(last.shape[0], Kind::Solid as u32 as f32);
        let ground = Palette::video().ground.with_alpha(1.0).premultiplied();
        assert_eq!(last.color, ground);
    }

    #[test]
    fn lifted_ends_rise_in_the_drawing() {
        let g =
            game("(ribbons: [(points: [(-2.0, 0.0), (2.0, 0.0)], head: Source, tail: Open(Red))])");
        let mut look = Look::default();
        look.lifts = vec![[0.0, 1.0]];
        let mesh = render(&g, &look);
        let top = mesh
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MIN, f32::max);
        assert!(top > BASE_Z + 0.8 * LIFT_HEIGHT);
    }
}
