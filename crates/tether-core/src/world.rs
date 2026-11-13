use crate::math::{Vec2, smoothstep};
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

pub const MAGNET_RADIUS: f32 = 0.7;
pub const WELD_RADIUS: f32 = 0.08;
pub const GRIP_STRAIN: f32 = 3.0;
pub const TEAR_STRAIN: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weld {
    pub a: EndRef,
    pub b: EndRef,
}

impl Weld {
    pub fn other(&self, end: EndRef) -> Option<EndRef> {
        if end == self.a {
            Some(self.b)
        } else if end == self.b {
            Some(self.a)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Welded(Weld),
    Tore(Weld),
    Slipped(EndRef),
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
    pub welds: Vec<Weld>,
    pub events: Vec<Event>,
    pub params: Params,
}

impl World {
    pub fn new(ribbons: Vec<Ribbon>) -> Self {
        Self {
            ribbons,
            ..Self::default()
        }
    }

    pub fn end_pos(&self, end: EndRef) -> Vec2 {
        self.ribbons[end.ribbon].end_pos(end.side)
    }

    pub fn partner(&self, end: EndRef) -> Option<EndRef> {
        self.welds.iter().find_map(|w| w.other(end))
    }

    pub fn compatible(&self, a: EndRef, b: EndRef) -> bool {
        let class = |e: EndRef| self.ribbons[e.ribbon].end(e.side).class();
        match (class(a), class(b)) {
            (Some(x), Some(y)) => {
                a != b && x == y && self.partner(a).is_none() && self.partner(b).is_none()
            }
            _ => false,
        }
    }

    pub fn magnet(&self, end: EndRef) -> Option<(EndRef, f32)> {
        let at = self.end_pos(end);
        let mut best: Option<(EndRef, f32)> = None;
        for (ribbon, r) in self.ribbons.iter().enumerate() {
            for side in [Side::Head, Side::Tail] {
                let other = EndRef { ribbon, side };
                if !self.compatible(end, other) {
                    continue;
                }
                let d = r.end_pos(side).distance(at);
                if d < MAGNET_RADIUS && best.is_none_or(|(_, nearest)| d < nearest) {
                    best = Some((other, d));
                }
            }
        }
        best
    }

    fn settle(&mut self) {
        if let Some(grip) = self.grip
            && let Some((partner, d)) = self.magnet(grip.end)
            && d < WELD_RADIUS
        {
            let weld = Weld {
                a: grip.end,
                b: partner,
            };
            let meet = (self.end_pos(weld.a) + self.end_pos(weld.b)) * 0.5;
            for end in [weld.a, weld.b] {
                let r = &mut self.ribbons[end.ribbon];
                let i = r.index(end.side);
                r.pos[i] = meet;
            }
            self.welds.push(weld);
            self.events.push(Event::Welded(weld));
            self.grip = None;
        }
        let ribbons = &self.ribbons;
        let mut torn = Vec::new();
        self.welds.retain(|w| {
            let keep = ribbons[w.a.ribbon].free_strain() <= TEAR_STRAIN
                && ribbons[w.b.ribbon].free_strain() <= TEAR_STRAIN;
            if !keep {
                torn.push(*w);
            }
            keep
        });
        for weld in torn {
            self.events.push(Event::Tore(weld));
            if self.grip.is_some_and(|g| weld.other(g.end).is_some()) {
                self.grip = None;
            }
        }
        if let Some(grip) = self.grip
            && self.ribbons[grip.end.ribbon].free_strain() > GRIP_STRAIN
        {
            self.grip = None;
            self.events.push(Event::Slipped(grip.end));
        }
    }

    fn solve_seams(&mut self, bend: f32) {
        for weld in &self.welds {
            let ends = [weld.a, weld.b];
            let [ia, ib] = ends.map(|e| self.ribbons[e.ribbon].index(e.side));
            let [na, nb] = ends.map(|e| {
                let r = &self.ribbons[e.ribbon];
                match e.side {
                    Side::Head => 1,
                    Side::Tail => r.len() - 2,
                }
            });
            let (ra, rb) = (weld.a.ribbon, weld.b.ribbon);
            let (wa, wb) = (self.ribbons[ra].inv_mass[ia], self.ribbons[rb].inv_mass[ib]);
            if wa + wb == 0.0 {
                continue;
            }
            let gap = self.ribbons[ra].pos[ia] - self.ribbons[rb].pos[ib];
            self.ribbons[ra].pos[ia] -= gap * (wa / (wa + wb));
            self.ribbons[rb].pos[ib] += gap * (wb / (wa + wb));
            let seam = self.ribbons[ra].pos[ia];
            let (pa, pb) = (self.ribbons[ra].pos[na], self.ribbons[rb].pos[nb]);
            let (wpa, wpb) = (self.ribbons[ra].inv_mass[na], self.ribbons[rb].inv_mass[nb]);
            let ws = 0.5 * (wa + wb);
            let c = seam - (pa + pb) * 0.5;
            let dl = -c / (0.25 * wpa + ws + 0.25 * wpb + bend);
            let moved = seam + dl * ws;
            self.ribbons[ra].pos[ia] = moved;
            self.ribbons[rb].pos[ib] = moved;
            self.ribbons[ra].pos[na] -= dl * (0.5 * wpa);
            self.ribbons[rb].pos[nb] -= dl * (0.5 * wpb);
        }
    }

    fn solve_grip(&mut self) {
        let Some(grip) = self.grip else {
            return;
        };
        let mut target = grip.target;
        if let Some((partner, d)) = self.magnet(grip.end) {
            let p = self.end_pos(partner);
            let pull = smoothstep(MAGNET_RADIUS, WELD_RADIUS, d);
            target = target.lerp(p, pull);
            let held = self.end_pos(grip.end);
            let r = &mut self.ribbons[partner.ribbon];
            let i = r.index(partner.side);
            let lean = (held - r.pos[i]) * (LEAN * pull * r.inv_mass[i]);
            r.pos[i] += lean;
        }
        let r = &mut self.ribbons[grip.end.ribbon];
        let i = r.index(grip.end.side);
        if r.inv_mass[i] > 0.0 {
            r.pos[i] = target;
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
            self.solve_seams(bend);
            self.solve_grip();
        }
        self.settle();
    }
}

const LEAN: f32 = 0.02;

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

    const A_TAIL: EndRef = EndRef {
        ribbon: 0,
        side: Side::Tail,
    };
    const B_HEAD: EndRef = EndRef {
        ribbon: 1,
        side: Side::Head,
    };

    fn pair(b_head: End) -> World {
        let a = Ribbon::new(
            &[Vec2::new(-4.0, 0.0), Vec2::new(-0.5, 0.0)],
            0.1,
            0.12,
            (End::Source, End::Open(Class::Ink)),
            2.0,
        );
        let b = Ribbon::new(
            &[Vec2::new(0.5, 0.0), Vec2::new(4.0, 0.0)],
            0.1,
            0.12,
            (b_head, End::Goal),
            2.0,
        );
        World::new(vec![a, b])
    }

    fn hold(w: &mut World, end: EndRef, target: Vec2) {
        w.grip = Some(Grip { end, target });
    }

    fn welded_pair() -> World {
        let mut w = pair(End::Open(Class::Ink));
        hold(&mut w, A_TAIL, Vec2::new(0.5, 0.0));
        run(&mut w, 1.0);
        assert_eq!(w.welds.len(), 1, "fixture weld");
        w.events.clear();
        w
    }

    #[test]
    fn weld_knows_both_ends() {
        let weld = Weld {
            a: A_TAIL,
            b: B_HEAD,
        };
        assert_eq!(weld.other(A_TAIL), Some(B_HEAD));
        assert_eq!(weld.other(B_HEAD), Some(A_TAIL));
        let stranger = EndRef {
            ribbon: 0,
            side: Side::Head,
        };
        assert_eq!(weld.other(stranger), None);
    }

    #[test]
    fn compatible_ends_share_a_class_and_are_free() {
        let w = pair(End::Open(Class::Ink));
        assert!(w.compatible(A_TAIL, B_HEAD));
        assert!(!w.compatible(A_TAIL, A_TAIL));
        let source = EndRef {
            ribbon: 0,
            side: Side::Head,
        };
        assert!(!w.compatible(A_TAIL, source));
        let red = pair(End::Open(Class::Red));
        assert!(!red.compatible(A_TAIL, B_HEAD));
        let welded = welded_pair();
        assert!(!welded.compatible(A_TAIL, B_HEAD));
    }

    #[test]
    fn magnet_finds_the_nearest_compatible_end_in_range() {
        let mut w = pair(End::Open(Class::Ink));
        assert_eq!(w.magnet(A_TAIL), None);
        let tail = w.ribbons[0].index(Side::Tail);
        w.ribbons[0].pos[tail] = Vec2::new(0.0, 0.1);
        let (partner, distance) = w.magnet(A_TAIL).expect("magnet");
        assert_eq!(partner, B_HEAD);
        assert!((distance - Vec2::new(0.5, -0.1).length()).abs() < 1e-5);
    }

    #[test]
    fn contact_welds_the_ends_and_releases_the_grip() {
        let mut w = pair(End::Open(Class::Ink));
        hold(&mut w, A_TAIL, Vec2::new(0.5, 0.0));
        run(&mut w, 1.0);
        let weld = Weld {
            a: A_TAIL,
            b: B_HEAD,
        };
        assert_eq!(w.welds, vec![weld]);
        assert_eq!(w.partner(A_TAIL), Some(B_HEAD));
        assert_eq!(w.partner(B_HEAD), Some(A_TAIL));
        assert!(w.grip.is_none());
        assert!(w.events.contains(&Event::Welded(weld)));
    }

    #[test]
    fn magnet_snaps_a_near_end_into_a_weld() {
        let mut w = pair(End::Open(Class::Ink));
        hold(&mut w, A_TAIL, Vec2::new(0.1, 0.0));
        run(&mut w, 1.0);
        assert_eq!(w.welds.len(), 1);
    }

    #[test]
    fn incompatible_ends_never_weld() {
        let mut w = pair(End::Open(Class::Red));
        hold(&mut w, A_TAIL, Vec2::new(0.5, 0.0));
        run(&mut w, 1.0);
        assert!(w.welds.is_empty());
        assert!(w.grip.is_some());
    }

    #[test]
    fn weld_holds_the_ends_together_with_a_smooth_seam() {
        let mut w = welded_pair();
        run(&mut w, 2.0);
        let (a, b) = (&w.ribbons[0], &w.ribbons[1]);
        assert!(w.end_pos(A_TAIL).distance(w.end_pos(B_HEAD)) < 1e-3);
        let into = a.pos[a.len() - 1] - a.pos[a.len() - 2];
        let out = b.pos[1] - b.pos[0];
        assert!(crate::math::signed_angle(into, out).abs() < 0.3);
    }

    #[test]
    fn pulling_a_seam_hard_tears_it() {
        let mut w = welded_pair();
        hold(&mut w, A_TAIL, Vec2::new(0.0, 7.0));
        run(&mut w, 1.5);
        assert!(w.welds.is_empty());
        assert!(w.grip.is_none());
        assert!(w.events.iter().any(|e| matches!(e, Event::Tore(_))));
    }

    #[test]
    fn overstretched_grip_slips() {
        let mut w = pair(End::Open(Class::Ink));
        hold(&mut w, A_TAIL, Vec2::new(-1.0, 8.0));
        run(&mut w, 1.0);
        assert!(w.grip.is_none());
        assert!(w.events.contains(&Event::Slipped(A_TAIL)));
    }
}
