use crate::math::Vec2;
use crate::ribbon::{Ribbon, Side};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EndRef {
    pub ribbon: usize,
    pub side: Side,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grip {
    pub end: EndRef,
    pub target: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub substeps: u32,
    pub damping: f32,
    pub stretch: f32,
    pub bend: f32,
    pub shape: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            substeps: 8,
            damping: 3.0,
            stretch: 1e-5,
            bend: 1e-8,
            shape: 1e-8,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct World {
    pub ribbons: Vec<Ribbon>,
    pub grip: Option<Grip>,
    pub params: Params,
}

impl World {
    pub fn new(ribbons: Vec<Ribbon>) -> Self {
        Self {
            ribbons,
            ..Self::default()
        }
    }

    pub fn step(&mut self, dt: f32) {
        let substeps = self.params.substeps.max(1);
        let h = dt / substeps as f32;
        let damp = (-self.params.damping * h).exp();
        let scale = 1.0 / (h * h);
        let (stretch, bend, shape) = (
            self.params.stretch * scale,
            self.params.bend * scale,
            self.params.shape * scale,
        );
        for _ in 0..substeps {
            for r in &mut self.ribbons {
                integrate(r, damp);
                solve_stretch(r, stretch);
                solve_bend(r, bend);
                solve_shape(r, shape);
            }
            if let Some(grip) = self.grip {
                let r = &mut self.ribbons[grip.end.ribbon];
                let i = r.index(grip.end.side);
                if r.inv_mass[i] > 0.0 {
                    r.pos[i] = grip.target;
                }
            }
        }
    }
}

const INTERNAL_DAMPING: f32 = 0.3;

fn integrate(r: &mut Ribbon, damp: f32) {
    let n = r.len();
    let velocity: Vec<Vec2> = (0..n).map(|i| r.pos[i] - r.prev[i]).collect();
    for i in 0..n {
        let pos = r.pos[i];
        r.pos[i] = if r.inv_mass[i] > 0.0 {
            let mut v = velocity[i];
            if i > 0 && i + 1 < n {
                let mean = (velocity[i - 1] + velocity[i + 1]) * 0.5;
                v += (mean - v) * INTERNAL_DAMPING;
            }
            pos + v * damp
        } else {
            r.rest[i]
        };
        r.prev[i] = pos;
    }
}

fn solve_stretch(r: &mut Ribbon, alpha: f32) {
    for i in 0..r.len().saturating_sub(1) {
        let (wa, wb) = (r.inv_mass[i], r.inv_mass[i + 1]);
        let d = r.pos[i + 1] - r.pos[i];
        let len = d.length();
        if wa + wb == 0.0 || len < 1e-9 {
            continue;
        }
        let n = d / len;
        let dl = -(len - r.rest_len[i]) / (wa + wb + alpha);
        r.pos[i] -= n * (wa * dl);
        r.pos[i + 1] += n * (wb * dl);
    }
}

const SHAPE_DECADES: f32 = 6.0;

const BEND_STRIDES: [usize; 3] = [12, 4, 1];

fn solve_bend(r: &mut Ribbon, alpha: f32) {
    for stride in BEND_STRIDES {
        let alpha = alpha * (stride as f32).powi(4);
        for i in stride..r.len().saturating_sub(stride) {
            let (a, b) = (i - stride, i + stride);
            let (w0, w1, w2) = (r.inv_mass[a], r.inv_mass[i], r.inv_mass[b]);
            let weight = 0.25 * w0 + w1 + 0.25 * w2;
            if weight == 0.0 {
                continue;
            }
            let rest_offset = r.rest[i] - (r.rest[a] + r.rest[b]) * 0.5;
            let offset = follow(rest_offset, r.rest[b] - r.rest[a], r.pos[b] - r.pos[a]);
            let c = r.pos[i] - (r.pos[a] + r.pos[b]) * 0.5 - offset;
            let dl = -c / (weight + alpha);
            r.pos[a] -= dl * (0.5 * w0);
            r.pos[i] += dl * w1;
            r.pos[b] -= dl * (0.5 * w2);
        }
    }
}

fn follow(offset: Vec2, rest_chord: Vec2, chord: Vec2) -> Vec2 {
    let turn = Vec2::new(rest_chord.dot(chord), rest_chord.perp_dot(chord));
    let size = turn.length();
    if offset == Vec2::ZERO || size < 1e-12 {
        return offset;
    }
    let turn = turn / size;
    Vec2::new(
        turn.x * offset.x - turn.y * offset.y,
        turn.y * offset.x + turn.x * offset.y,
    )
}

fn solve_shape(r: &mut Ribbon, alpha: f32) {
    for i in 0..r.len() {
        let (s, w) = (r.stiffness[i], r.inv_mass[i]);
        let d = r.pos[i] - r.rest[i];
        let len = d.length();
        if s <= 0.0 || w == 0.0 || len < 1e-9 {
            continue;
        }
        let soft = 10f32.powf(SHAPE_DECADES * (1.0 - s));
        let dl = -len / (w + alpha * soft);
        r.pos[i] += d / len * (w * dl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ribbon::{Class, End};

    const DT: f32 = 1.0 / 120.0;

    fn world() -> World {
        let ribbon = Ribbon::new(
            &[Vec2::new(-2.0, 0.0), Vec2::new(2.0, 0.0)],
            0.1,
            0.12,
            (End::Source, End::Open(Class::Ink)),
            2.0,
        );
        World::new(vec![ribbon])
    }

    fn run(w: &mut World, seconds: f32) {
        for _ in 0..(seconds / DT).round() as usize {
            w.step(DT);
        }
    }

    fn deviation(r: &Ribbon, range: std::ops::Range<usize>) -> f32 {
        range
            .map(|i| r.pos[i].distance(r.rest[i]))
            .fold(0.0, f32::max)
    }

    fn grip_tail(w: &mut World, offset: Vec2) {
        let r = &w.ribbons[0];
        w.grip = Some(Grip {
            end: EndRef {
                ribbon: 0,
                side: Side::Tail,
            },
            target: r.rest[r.len() - 1] + offset,
        });
    }

    #[test]
    fn ribbon_at_rest_stays_at_rest() {
        let mut w = world();
        run(&mut w, 2.0);
        assert!(deviation(&w.ribbons[0], 0..41) < 1e-4);
    }

    #[test]
    fn gripped_end_follows_target_while_root_holds() {
        let mut w = world();
        grip_tail(&mut w, Vec2::new(0.0, 1.5));
        run(&mut w, 1.0);
        let r = &w.ribbons[0];
        assert!(r.pos[40].distance(w.grip.unwrap().target) < 1e-3);
        assert!(deviation(r, 0..5) < 0.01);
        assert!(r.pos[30].y > 0.2);
    }

    #[test]
    fn pulled_free_part_stretches_evenly() {
        let mut w = world();
        grip_tail(&mut w, Vec2::new(2.0, 0.0));
        run(&mut w, 2.0);
        let r = &w.ribbons[0];
        let strains: Vec<f32> = (24..40).map(|i| r.segment_strain(i)).collect();
        let max = strains.iter().copied().fold(0.0, f32::max);
        let min = strains.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(max / min < 1.35, "uneven strain {min} {max}");
        assert!((1.5..2.5).contains(&r.strain()), "strain {}", r.strain());
    }

    #[test]
    fn released_ribbon_returns_to_rest() {
        let mut w = world();
        grip_tail(&mut w, Vec2::new(-0.5, 1.2));
        run(&mut w, 1.0);
        w.grip = None;
        run(&mut w, 3.0);
        assert!(deviation(&w.ribbons[0], 0..41) < 0.02);
    }

    #[test]
    fn ribbon_with_a_corner_keeps_its_rest_shape() {
        let corner = Ribbon::new(
            &[
                Vec2::new(-2.0, 0.0),
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, 2.0),
            ],
            0.1,
            0.12,
            (End::Source, End::Open(Class::Red)),
            1.5,
        );
        let mut w = World::new(vec![corner]);
        run(&mut w, 2.0);
        assert!(deviation(&w.ribbons[0], 0..41) < 1e-4);
        grip_tail(&mut w, Vec2::new(1.0, -0.5));
        run(&mut w, 1.0);
        w.grip = None;
        run(&mut w, 3.0);
        assert!(deviation(&w.ribbons[0], 0..41) < 0.02);
    }

    #[test]
    fn dragged_free_part_bends_smoothly() {
        let mut w = world();
        grip_tail(&mut w, Vec2::new(-1.0, 1.8));
        run(&mut w, 1.5);
        let r = &w.ribbons[0];
        let turns: Vec<f32> = (1..40)
            .map(|i| {
                crate::math::signed_angle(r.pos[i] - r.pos[i - 1], r.pos[i + 1] - r.pos[i]).abs()
            })
            .collect();
        let max = turns.iter().copied().fold(0.0, f32::max);
        let total: f32 = turns.iter().sum();
        assert!(total > 0.5, "total turn {total}");
        assert!(max < 0.25, "kink {max}");
    }

    #[test]
    fn fixed_ends_never_move() {
        let mut w = world();
        grip_tail(&mut w, Vec2::new(1.0, -2.0));
        run(&mut w, 1.0);
        assert_eq!(w.ribbons[0].pos[0], w.ribbons[0].rest[0]);
    }

    #[test]
    fn gripping_a_fixed_end_does_nothing() {
        let mut w = world();
        w.grip = Some(Grip {
            end: EndRef {
                ribbon: 0,
                side: Side::Head,
            },
            target: Vec2::new(0.0, 3.0),
        });
        run(&mut w, 0.5);
        assert!(deviation(&w.ribbons[0], 0..41) < 1e-4);
    }
}
