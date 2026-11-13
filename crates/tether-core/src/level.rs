use crate::math::Vec2;
use crate::ribbon::{End, Ribbon};
use crate::world::World;
use serde::Deserialize;

pub const SPACING: f32 = 0.1;

pub const LEVELS: &[&str] = &[
    include_str!("../levels/01.ron"),
    include_str!("../levels/02.ron"),
    include_str!("../levels/03.ron"),
    include_str!("../levels/04.ron"),
    include_str!("../levels/05.ron"),
];

fn default_free() -> f32 {
    2.0
}

fn default_width() -> f32 {
    0.24
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RibbonSpec {
    pub points: Vec<(f32, f32)>,
    pub head: End,
    pub tail: End,
    #[serde(default = "default_free")]
    pub free: f32,
    #[serde(default = "default_width")]
    pub width: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Level {
    pub ribbons: Vec<RibbonSpec>,
}

impl Level {
    pub fn parse(text: &str) -> Result<Self, String> {
        let level: Self = ron::from_str(text).map_err(|e| e.to_string())?;
        if level.ribbons.iter().any(|r| r.points.len() < 2) {
            return Err("each ribbon needs two or more points".into());
        }
        Ok(level)
    }

    pub fn build(&self) -> World {
        let ribbons = self
            .ribbons
            .iter()
            .map(|spec| {
                let points: Vec<Vec2> = spec.points.iter().map(|&(x, y)| Vec2::new(x, y)).collect();
                Ribbon::new(
                    &points,
                    SPACING,
                    spec.width * 0.5,
                    (spec.head, spec.tail),
                    spec.free,
                )
            })
            .collect();
        World::new(ribbons)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ribbon::{Class, Side};

    const TWO: &str = "(
        ribbons: [
            (points: [(-4.0, 0.0), (-1.0, 0.0)], head: Source, tail: Open(Ink), free: 1.5, width: 0.3),
            (points: [(1.0, 1.0), (4.0, 1.0)], head: Open(Red), tail: Goal),
        ],
    )";

    #[test]
    fn parse_reads_ribbons_and_ends() {
        let level = Level::parse(TWO).expect("parse");
        assert_eq!(level.ribbons.len(), 2);
        let a = &level.ribbons[0];
        assert_eq!(a.points, vec![(-4.0, 0.0), (-1.0, 0.0)]);
        assert_eq!((a.head, a.tail), (End::Source, End::Open(Class::Ink)));
        assert_eq!((a.free, a.width), (1.5, 0.3));
    }

    #[test]
    fn parse_fills_defaults() {
        let level = Level::parse(TWO).expect("parse");
        let b = &level.ribbons[1];
        assert_eq!((b.head, b.tail), (End::Open(Class::Red), End::Goal));
        assert_eq!((b.free, b.width), (default_free(), default_width()));
    }

    #[test]
    fn parse_reports_errors() {
        assert!(Level::parse("(ribbons: [(points: [], head: Nowhere)])").is_err());
    }

    #[test]
    fn build_makes_a_world_with_sampled_ribbons() {
        let world = Level::parse(TWO).expect("parse").build();
        assert_eq!(world.ribbons.len(), 2);
        let r = &world.ribbons[0];
        assert_eq!(r.len(), 31);
        assert!((r.half_width - 0.15).abs() < 1e-6);
        assert_eq!(r.end(Side::Tail), End::Open(Class::Ink));
        assert!(r.end_pos(Side::Head).distance(Vec2::new(-4.0, 0.0)) < 1e-5);
        assert!(world.grip.is_none());
    }

    #[test]
    fn bundled_levels_are_valid() {
        for (i, text) in LEVELS.iter().enumerate() {
            let level = Level::parse(text).unwrap_or_else(|e| panic!("level {i}: {e}"));
            let ends: Vec<End> = level
                .ribbons
                .iter()
                .flat_map(|r| [r.head, r.tail])
                .collect();
            assert_eq!(
                ends.iter().filter(|e| **e == End::Source).count(),
                1,
                "level {i}"
            );
            assert!(ends.contains(&End::Goal), "level {i}");
            assert!(
                level.ribbons.iter().all(|r| r.points.len() >= 2),
                "level {i}"
            );
        }
    }
}
