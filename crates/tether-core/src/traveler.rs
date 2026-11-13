use crate::math::Vec2;
use crate::ribbon::{End, Side};
use crate::world::{EndRef, World};

pub const SPEED: f32 = 1.6;
pub const REEL_SPEED: f32 = 7.0;
pub const PAUSE: f32 = 0.9;
pub const FALL_TIME: f32 = 0.6;
pub const STOP_TIME: f32 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub ribbon: usize,
    pub from: f32,
    pub to: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Waiting { left: f32 },
    Moving,
    Falling { out: Vec2, age: f32 },
    Stopped { age: f32 },
    Reeling,
    Arrived,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Moment {
    Departed,
    Crossed,
    Fell,
    Stopped,
    Reeled,
    Arrived,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Traveler {
    pub ribbon: usize,
    pub u: f32,
    pub dir: f32,
    pub phase: Phase,
    pub tether: Vec<Span>,
    source: EndRef,
}

pub const FALL_DISTANCE: f32 = 0.9;
const GUARD: usize = 10_000;

fn start(n: usize, side: Side) -> (f32, f32) {
    match side {
        Side::Head => (0.0, 1.0),
        Side::Tail => ((n - 1) as f32, -1.0),
    }
}

impl Traveler {
    pub fn new(world: &World) -> Option<Self> {
        for (ribbon, r) in world.ribbons.iter().enumerate() {
            for side in [Side::Head, Side::Tail] {
                if r.end(side) == End::Source {
                    let (u, dir) = start(r.len(), side);
                    return Some(Self {
                        ribbon,
                        u,
                        dir,
                        phase: Phase::Waiting { left: PAUSE },
                        tether: Vec::new(),
                        source: EndRef { ribbon, side },
                    });
                }
            }
        }
        None
    }

    pub fn position(&self, world: &World) -> Vec2 {
        let on = point_at(world, self.ribbon, self.u);
        match self.phase {
            Phase::Falling { out, age } => {
                let s = (age / FALL_TIME).clamp(0.0, 1.0);
                on + out * (FALL_DISTANCE * (std::f32::consts::PI * s).sin())
            }
            _ => on,
        }
    }

    pub fn step(&mut self, world: &World, dt: f32) -> Vec<Moment> {
        let mut moments = Vec::new();
        match self.phase {
            Phase::Waiting { left } if left - dt > 0.0 => {
                self.phase = Phase::Waiting { left: left - dt };
            }
            Phase::Waiting { .. } => {
                self.phase = Phase::Moving;
                self.tether.push(Span {
                    ribbon: self.ribbon,
                    from: self.u,
                    to: self.u,
                });
                moments.push(Moment::Departed);
            }
            Phase::Moving => self.advance(world, SPEED * dt, &mut moments),
            Phase::Falling { out, age } => {
                self.phase = if age + dt >= FALL_TIME {
                    Phase::Reeling
                } else {
                    Phase::Falling { out, age: age + dt }
                };
            }
            Phase::Stopped { age } => {
                self.phase = if age + dt >= STOP_TIME {
                    Phase::Reeling
                } else {
                    Phase::Stopped { age: age + dt }
                };
            }
            Phase::Reeling => self.reel(world, REEL_SPEED * dt, &mut moments),
            Phase::Arrived => {}
        }
        moments
    }

    fn advance(&mut self, world: &World, mut remaining: f32, moments: &mut Vec<Moment>) {
        for _ in 0..GUARD {
            let last = (world.ribbons[self.ribbon].len() - 1) as f32;
            let limit = if self.dir > 0.0 { last } else { 0.0 };
            let (u, left) = slide(world, self.ribbon, self.u, self.dir, limit, remaining);
            self.u = u;
            if let Some(span) = self.tether.last_mut() {
                span.to = u;
            }
            if u != limit || !self.cross(world, moments) {
                return;
            }
            remaining = left;
        }
    }

    fn cross(&mut self, world: &World, moments: &mut Vec<Moment>) -> bool {
        let side = if self.dir > 0.0 {
            Side::Tail
        } else {
            Side::Head
        };
        let end = EndRef {
            ribbon: self.ribbon,
            side,
        };
        if let Some(other) = world.partner(end) {
            let (u, dir) = start(world.ribbons[other.ribbon].len(), other.side);
            self.ribbon = other.ribbon;
            self.u = u;
            self.dir = dir;
            self.tether.push(Span {
                ribbon: other.ribbon,
                from: u,
                to: u,
            });
            moments.push(Moment::Crossed);
            return true;
        }
        let r = &world.ribbons[self.ribbon];
        match r.end(side) {
            End::Goal => {
                self.phase = Phase::Arrived;
                moments.push(Moment::Arrived);
            }
            End::Open(_) => {
                self.phase = Phase::Falling {
                    out: r.outward(side),
                    age: 0.0,
                };
                moments.push(Moment::Fell);
            }
            End::Cap | End::Source => {
                self.phase = Phase::Stopped { age: 0.0 };
                moments.push(Moment::Stopped);
            }
        }
        false
    }

    fn reel(&mut self, world: &World, mut remaining: f32, moments: &mut Vec<Moment>) {
        for _ in 0..GUARD {
            let Some(span) = self.tether.last().copied() else {
                let (u, dir) = start(world.ribbons[self.source.ribbon].len(), self.source.side);
                self.ribbon = self.source.ribbon;
                self.u = u;
                self.dir = dir;
                self.phase = Phase::Waiting { left: PAUSE };
                moments.push(Moment::Reeled);
                return;
            };
            self.ribbon = span.ribbon;
            let dir = if span.from > self.u { 1.0 } else { -1.0 };
            let (u, left) = slide(world, span.ribbon, self.u, dir, span.from, remaining);
            self.u = u;
            if let Some(last) = self.tether.last_mut() {
                last.to = u;
            }
            if u != span.from {
                return;
            }
            self.tether.pop();
            remaining = left;
            if let Some(previous) = self.tether.last() {
                self.ribbon = previous.ribbon;
                self.u = previous.to;
            }
        }
    }
}

fn slide(
    world: &World,
    ribbon: usize,
    mut u: f32,
    dir: f32,
    limit: f32,
    mut remaining: f32,
) -> (f32, f32) {
    let r = &world.ribbons[ribbon];
    let n = r.len();
    for _ in 0..=n {
        if (limit - u) * dir <= 0.0 {
            return (limit, remaining);
        }
        let k = if dir > 0.0 { u.floor() } else { u.ceil() - 1.0 };
        let k = k.clamp(0.0, (n - 2) as f32) as usize;
        let boundary = if dir > 0.0 { (k + 1) as f32 } else { k as f32 };
        let stop = if dir > 0.0 {
            boundary.min(limit)
        } else {
            boundary.max(limit)
        };
        let full = r.pos[k].distance(r.pos[k + 1]).max(1e-6);
        let gap = (stop - u).abs() * full;
        if remaining < gap {
            return (u + dir * remaining / full, 0.0);
        }
        remaining -= gap;
        u = stop;
    }
    (u, remaining)
}

pub fn point_at(world: &World, ribbon: usize, u: f32) -> Vec2 {
    let r = &world.ribbons[ribbon];
    let n = r.len();
    let u = u.clamp(0.0, (n - 1) as f32);
    let k = (u.floor() as usize).min(n - 2);
    r.pos[k].lerp(r.pos[k + 1], u - k as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Level;
    use crate::world::Weld;

    const DT: f32 = 1.0 / 120.0;

    fn world(text: &str) -> World {
        Level::parse(text).expect("level").build()
    }

    fn run(t: &mut Traveler, w: &World, seconds: f32) -> Vec<Moment> {
        let mut all = Vec::new();
        for _ in 0..(seconds / DT).round() as usize {
            all.extend(t.step(w, DT));
        }
        all
    }

    const LINE: &str =
        "(ribbons: [(points: [(-4.0, 0.0), (4.0, 0.0)], head: Source, tail: Open(Ink))])";
    const GOAL: &str = "(ribbons: [(points: [(-1.0, 0.0), (1.0, 0.0)], head: Source, tail: Goal)])";
    const CAP: &str = "(ribbons: [(points: [(1.0, 0.0), (-1.0, 0.0)], head: Cap, tail: Source)])";

    #[test]
    fn traveler_starts_waiting_at_the_source() {
        let w = world(CAP);
        let t = Traveler::new(&w).expect("source");
        assert_eq!((t.ribbon, t.u, t.dir), (0, 20.0, -1.0));
        assert!(matches!(t.phase, Phase::Waiting { .. }));
        assert!(t.position(&w).distance(Vec2::new(-1.0, 0.0)) < 1e-5);
        assert!(t.tether.is_empty());
    }

    #[test]
    fn world_without_a_source_has_no_traveler() {
        let w = world("(ribbons: [(points: [(0.0, 0.0), (1.0, 0.0)], head: Cap, tail: Goal)])");
        assert!(Traveler::new(&w).is_none());
    }

    #[test]
    fn traveler_departs_after_the_pause_and_keeps_its_speed() {
        let w = world(LINE);
        let mut t = Traveler::new(&w).unwrap();
        let moments = run(&mut t, &w, PAUSE + 0.01);
        assert_eq!(moments, vec![Moment::Departed]);
        assert_eq!(t.phase, Phase::Moving);
        let start = t.position(&w);
        run(&mut t, &w, 1.0);
        assert!((t.position(&w).distance(start) - SPEED).abs() < 0.02);
    }

    #[test]
    fn point_at_interpolates_the_material_coordinate() {
        let w = world(LINE);
        assert!(point_at(&w, 0, 0.0).distance(Vec2::new(-4.0, 0.0)) < 1e-5);
        assert!(point_at(&w, 0, 10.5).distance(Vec2::new(-2.95, 0.0)) < 1e-5);
        assert!(point_at(&w, 0, 80.0).distance(Vec2::new(4.0, 0.0)) < 1e-5);
    }

    #[test]
    fn traveler_rides_the_material_when_the_ribbon_moves() {
        let mut w = world(LINE);
        let mut t = Traveler::new(&w).unwrap();
        run(&mut t, &w, PAUSE + 1.0);
        let u = t.u;
        for p in &mut w.ribbons[0].pos {
            p.y += 1.0;
        }
        assert!(t.position(&w).distance(point_at(&w, 0, u)) < 1e-6);
        assert!((t.position(&w).y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn traveler_falls_off_an_open_edge_and_reels_back() {
        let w = world(LINE);
        let mut t = Traveler::new(&w).unwrap();
        let moments = run(&mut t, &w, PAUSE + 8.0 / SPEED + 0.05);
        assert!(moments.contains(&Moment::Fell));
        assert!(matches!(t.phase, Phase::Falling { .. }));
        assert!(t.position(&w).x > 4.0);
        let moments = run(&mut t, &w, FALL_TIME + 8.0 / REEL_SPEED + 0.1);
        assert!(moments.contains(&Moment::Reeled));
        assert!(matches!(t.phase, Phase::Waiting { .. }));
        assert!(t.position(&w).distance(Vec2::new(-4.0, 0.0)) < 1e-4);
        assert!(t.tether.is_empty());
    }

    #[test]
    fn traveler_stops_at_a_cap_and_reels_back() {
        let w = world(CAP);
        let mut t = Traveler::new(&w).unwrap();
        let moments = run(&mut t, &w, PAUSE + 2.0 / SPEED + 0.05);
        assert!(moments.contains(&Moment::Stopped));
        let moments = run(&mut t, &w, STOP_TIME + 2.0 / REEL_SPEED + 0.1);
        assert!(moments.contains(&Moment::Reeled));
    }

    #[test]
    fn traveler_arrives_at_the_goal() {
        let w = world(GOAL);
        let mut t = Traveler::new(&w).unwrap();
        let moments = run(&mut t, &w, PAUSE + 2.0 / SPEED + 0.05);
        assert!(moments.contains(&Moment::Arrived));
        assert_eq!(t.phase, Phase::Arrived);
        assert!(t.position(&w).distance(Vec2::new(1.0, 0.0)) < 1e-4);
    }

    #[test]
    fn traveler_crosses_a_weld_onto_the_partner() {
        let mut w = world(
            "(ribbons: [
                (points: [(-2.0, 0.0), (0.0, 0.0)], head: Source, tail: Open(Ink)),
                (points: [(0.0, 2.0), (0.0, 0.0)], head: Goal, tail: Open(Ink)),
            ])",
        );
        w.welds.push(Weld {
            a: EndRef {
                ribbon: 0,
                side: Side::Tail,
            },
            b: EndRef {
                ribbon: 1,
                side: Side::Tail,
            },
        });
        let mut t = Traveler::new(&w).unwrap();
        let moments = run(&mut t, &w, PAUSE + 2.5 / SPEED);
        assert!(moments.contains(&Moment::Crossed));
        assert_eq!((t.ribbon, t.dir), (1, -1.0));
        assert_eq!(t.tether.len(), 2);
        assert!(t.position(&w).x.abs() < 1e-4 && t.position(&w).y > 0.4);
        let moments = run(&mut t, &w, 2.0 / SPEED);
        assert!(moments.contains(&Moment::Arrived));
    }

    #[test]
    fn tether_records_the_path_in_order() {
        let w = world(LINE);
        let mut t = Traveler::new(&w).unwrap();
        run(&mut t, &w, PAUSE + 1.0);
        assert_eq!(t.tether.len(), 1);
        let span = t.tether[0];
        assert_eq!((span.ribbon, span.from), (0, 0.0));
        assert!((span.to - t.u).abs() < 1e-6);
    }
}
