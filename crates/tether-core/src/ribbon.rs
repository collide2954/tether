use crate::math::{Vec2, smoothstep};
use serde::Deserialize;

pub const RAMP: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum Class {
    Ink,
    Yellow,
    Red,
    Blue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum End {
    Source,
    Goal,
    Cap,
    Open(Class),
}

impl End {
    pub fn is_fixed(self) -> bool {
        !matches!(self, End::Open(_))
    }

    pub fn class(self) -> Option<Class> {
        match self {
            End::Open(class) => Some(class),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Head,
    Tail,
}

impl Side {
    pub fn other(self) -> Self {
        match self {
            Side::Head => Side::Tail,
            Side::Tail => Side::Head,
        }
    }
}

fn polyline_length(points: &[Vec2]) -> f32 {
    points.windows(2).map(|w| w[0].distance(w[1])).sum()
}

pub fn resample(control: &[Vec2], spacing: f32) -> Vec<Vec2> {
    let Some(&first) = control.first() else {
        return Vec::new();
    };
    let total = polyline_length(control);
    if total <= f32::EPSILON || spacing <= 0.0 {
        return vec![first, first];
    }
    let n = ((total / spacing).round() as usize).max(1) + 1;
    let step = total / (n - 1) as f32;
    let last = control.len() - 1;
    let mut out = Vec::with_capacity(n);
    let mut seg = 0;
    let mut seg_start = 0.0;
    for k in 0..n {
        let s = (k as f32 * step).min(total);
        while seg + 1 < last && seg_start + control[seg].distance(control[seg + 1]) < s {
            seg_start += control[seg].distance(control[seg + 1]);
            seg += 1;
        }
        let len = control[seg].distance(control[seg + 1]);
        let t = if len > 0.0 {
            ((s - seg_start) / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(control[seg].lerp(control[seg + 1], t));
    }
    out[n - 1] = control[last];
    out
}

#[derive(Clone, Debug)]
pub struct Ribbon {
    pub pos: Vec<Vec2>,
    pub prev: Vec<Vec2>,
    pub rest: Vec<Vec2>,
    pub inv_mass: Vec<f32>,
    pub stiffness: Vec<f32>,
    pub rest_len: Vec<f32>,
    pub spacing: f32,
    pub half_width: f32,
    pub head: End,
    pub tail: End,
}

impl Ribbon {
    pub fn new(
        control: &[Vec2],
        spacing: f32,
        half_width: f32,
        ends: (End, End),
        free_length: f32,
    ) -> Self {
        let (head, tail) = ends;
        let rest = resample(control, spacing);
        let n = rest.len();
        let total = polyline_length(control);
        let spacing = if n > 1 && total > 0.0 {
            total / (n - 1) as f32
        } else {
            spacing
        };
        let mut inv_mass = vec![1.0; n];
        if head.is_fixed() {
            inv_mass[0] = 0.0;
        }
        if tail.is_fixed() {
            inv_mass[n - 1] = 0.0;
        }
        let stiffness = (0..n)
            .map(|i| {
                let s = i as f32 * spacing;
                let mut d = f32::INFINITY;
                if !head.is_fixed() {
                    d = d.min(s);
                }
                if !tail.is_fixed() {
                    d = d.min(total - s);
                }
                smoothstep(free_length, free_length + RAMP, d)
            })
            .collect();
        let rest_len = rest.windows(2).map(|w| w[0].distance(w[1])).collect();
        Self {
            pos: rest.clone(),
            prev: rest.clone(),
            rest,
            inv_mass,
            stiffness,
            rest_len,
            spacing,
            half_width,
            head,
            tail,
        }
    }

    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    pub fn index(&self, side: Side) -> usize {
        match side {
            Side::Head => 0,
            Side::Tail => self.len() - 1,
        }
    }

    pub fn end(&self, side: Side) -> End {
        match side {
            Side::Head => self.head,
            Side::Tail => self.tail,
        }
    }

    pub fn end_pos(&self, side: Side) -> Vec2 {
        self.pos[self.index(side)]
    }

    pub fn outward(&self, side: Side) -> Vec2 {
        let n = self.len();
        match side {
            Side::Head => (self.pos[0] - self.pos[1]).normalize_or_zero(),
            Side::Tail => (self.pos[n - 1] - self.pos[n - 2]).normalize_or_zero(),
        }
    }

    pub fn segment_strain(&self, i: usize) -> f32 {
        let rest = self.rest_len[i];
        if rest > 0.0 {
            self.pos[i].distance(self.pos[i + 1]) / rest
        } else {
            1.0
        }
    }

    pub fn free_strain(&self) -> f32 {
        let (mut now, mut rest) = (0.0, 0.0);
        for i in 0..self.rest_len.len() {
            if self.stiffness[i].min(self.stiffness[i + 1]) < 1.0 {
                now += self.pos[i].distance(self.pos[i + 1]);
                rest += self.rest_len[i];
            }
        }
        if rest > 0.0 { now / rest } else { 1.0 }
    }

    pub fn strain(&self) -> f32 {
        (0..self.rest_len.len())
            .map(|i| self.segment_strain(i))
            .fold(1.0, f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn near_v(a: Vec2, b: Vec2) -> bool {
        (a - b).length() < 1e-4
    }

    fn line(head: End, tail: End, free: f32) -> Ribbon {
        Ribbon::new(
            &[Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)],
            0.1,
            0.12,
            (head, tail),
            free,
        )
    }

    #[test]
    fn open_ends_are_free_and_carry_a_class() {
        assert!(End::Source.is_fixed() && End::Goal.is_fixed() && End::Cap.is_fixed());
        assert!(!End::Open(Class::Red).is_fixed());
        assert_eq!(End::Open(Class::Blue).class(), Some(Class::Blue));
        assert_eq!(End::Goal.class(), None);
        assert_eq!(Side::Head.other(), Side::Tail);
        assert_eq!(Side::Tail.other(), Side::Head);
    }

    #[test]
    fn resample_spaces_points_evenly_on_a_line() {
        let pts = resample(&[Vec2::ZERO, Vec2::new(1.0, 0.0)], 0.1);
        assert_eq!(pts.len(), 11);
        for (k, p) in pts.iter().enumerate() {
            assert!(near_v(*p, Vec2::new(0.1 * k as f32, 0.0)));
        }
    }

    #[test]
    fn resample_follows_corners() {
        let pts = resample(
            &[Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0)],
            0.25,
        );
        assert_eq!(pts.len(), 9);
        assert!(near_v(pts[4], Vec2::new(1.0, 0.0)));
        assert!(near_v(pts[6], Vec2::new(1.0, 0.5)));
        assert!(near_v(pts[8], Vec2::new(1.0, 1.0)));
    }

    #[test]
    fn resample_rounds_to_an_even_spacing_and_keeps_endpoints() {
        let pts = resample(&[Vec2::ZERO, Vec2::new(1.0, 0.0)], 0.3);
        assert_eq!(pts.len(), 4);
        assert!(near_v(pts[1], Vec2::new(1.0 / 3.0, 0.0)));
        assert!(near_v(pts[3], Vec2::new(1.0, 0.0)));
    }

    #[test]
    fn resample_of_a_point_gives_two_points() {
        let p = Vec2::new(2.0, 1.0);
        assert_eq!(resample(&[p], 0.1), vec![p, p]);
        assert_eq!(resample(&[p, p], 0.1), vec![p, p]);
    }

    #[test]
    fn new_ribbon_starts_at_rest() {
        let r = line(End::Source, End::Open(Class::Ink), 1.0);
        assert_eq!(r.len(), 41);
        assert!(near(r.spacing, 0.1));
        assert_eq!(r.pos, r.rest);
        assert_eq!(r.prev, r.rest);
        assert!(near(r.strain(), 1.0));
    }

    #[test]
    fn fixed_ends_have_no_inverse_mass() {
        let r = line(End::Source, End::Open(Class::Ink), 1.0);
        assert_eq!(r.inv_mass[0], 0.0);
        assert_eq!(r.inv_mass[40], 1.0);
        assert_eq!(r.inv_mass[20], 1.0);
        let capped = line(End::Cap, End::Goal, 1.0);
        assert_eq!((capped.inv_mass[0], capped.inv_mass[40]), (0.0, 0.0));
    }

    #[test]
    fn stiffness_is_zero_near_open_ends_and_full_far_away() {
        let r = line(End::Source, End::Open(Class::Ink), 1.0);
        assert_eq!(r.stiffness[40], 0.0);
        assert_eq!(r.stiffness[31], 0.0);
        assert_eq!(r.stiffness[0], 1.0);
        let firm = 40 - ((1.0 + RAMP) / 0.1).ceil() as usize - 1;
        assert_eq!(r.stiffness[firm], 1.0);
        assert!(r.stiffness[27] > 0.0 && r.stiffness[27] < 1.0);
        let fixed = line(End::Source, End::Goal, 1.0);
        assert!(fixed.stiffness.iter().all(|&s| s == 1.0));
    }

    #[test]
    fn ends_report_kind_position_and_outward_direction() {
        let r = line(End::Source, End::Open(Class::Red), 1.0);
        assert_eq!(r.index(Side::Head), 0);
        assert_eq!(r.index(Side::Tail), 40);
        assert_eq!(r.end(Side::Tail), End::Open(Class::Red));
        assert!(near_v(r.end_pos(Side::Tail), Vec2::new(4.0, 0.0)));
        assert!(near_v(r.outward(Side::Head), Vec2::new(-1.0, 0.0)));
        assert!(near_v(r.outward(Side::Tail), Vec2::new(1.0, 0.0)));
    }

    #[test]
    fn free_strain_measures_the_stretch_of_the_soft_part() {
        let mut r = line(End::Source, End::Open(Class::Ink), 1.0);
        assert!(near(r.free_strain(), 1.0));
        let soft_rest: f32 = (0..40)
            .filter(|&i| r.stiffness[i].min(r.stiffness[i + 1]) < 1.0)
            .map(|i| r.rest_len[i])
            .sum();
        r.pos[40].x += 0.5;
        assert!(near(r.free_strain(), (soft_rest + 0.5) / soft_rest));
        let fixed = line(End::Source, End::Goal, 1.0);
        assert!(near(fixed.free_strain(), 1.0));
    }

    #[test]
    fn strain_is_the_largest_segment_stretch() {
        let mut r = line(End::Source, End::Open(Class::Ink), 1.0);
        r.pos[40].x += 0.1;
        assert!(near(r.segment_strain(39), 2.0));
        assert!(near(r.segment_strain(0), 1.0));
        assert!(near(r.strain(), 2.0));
    }
}
