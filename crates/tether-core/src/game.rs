use crate::color::Palette;
use crate::level::{LEVELS, Level};
use crate::math::Vec2;
use crate::mesh::Mesh;
use crate::ribbon::Side;
use crate::scene::draw_world;
use crate::world::{EndRef, Grip, World};

pub const STEP: f32 = 1.0 / 120.0;
pub const MAX_FRAME: f32 = 0.25;
pub const GRAB_RADIUS: f32 = 0.45;

#[derive(Clone, Debug)]
pub struct Game {
    pub world: World,
    pub level: usize,
    pub time: f32,
    accumulator: f32,
}

impl Game {
    pub fn new(level: usize) -> Self {
        let level = level % LEVELS.len();
        let world = Level::parse(LEVELS[level])
            .map(|l| l.build())
            .unwrap_or_default();
        Self {
            world,
            level,
            time: 0.0,
            accumulator: 0.0,
        }
    }

    pub fn advance(&mut self, dt: f32) -> usize {
        self.accumulator += dt.clamp(0.0, MAX_FRAME);
        let mut steps = 0;
        while self.accumulator + 1e-6 >= STEP {
            self.accumulator = (self.accumulator - STEP).max(0.0);
            self.world.step(STEP);
            self.time += STEP;
            steps += 1;
        }
        steps
    }

    pub fn pointer_down(&mut self, at: Vec2) {
        let mut best: Option<(f32, EndRef)> = None;
        for (ribbon, r) in self.world.ribbons.iter().enumerate() {
            for side in [Side::Head, Side::Tail] {
                if r.end(side).is_fixed() {
                    continue;
                }
                let d = r.end_pos(side).distance(at);
                if d <= GRAB_RADIUS && best.is_none_or(|(nearest, _)| d < nearest) {
                    best = Some((d, EndRef { ribbon, side }));
                }
            }
        }
        self.world.grip = best.map(|(_, end)| Grip { end, target: at });
    }

    pub fn pointer_move(&mut self, at: Vec2) {
        if let Some(grip) = &mut self.world.grip {
            grip.target = at;
        }
    }

    pub fn pointer_up(&mut self) {
        self.world.grip = None;
    }

    pub fn draw(&self, palette: &Palette, mesh: &mut Mesh) {
        draw_world(&self.world, palette, self.time, mesh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_tail(game: &Game) -> Vec2 {
        game.world.ribbons[0].end_pos(Side::Tail)
    }

    #[test]
    fn new_game_loads_the_level() {
        let game = Game::new(0);
        assert_eq!(game.level, 0);
        assert_eq!(game.world.ribbons.len(), 2);
        assert_eq!(game.time, 0.0);
    }

    #[test]
    fn new_game_wraps_the_level_index() {
        assert_eq!(Game::new(LEVELS.len()).level, 0);
    }

    #[test]
    fn advance_runs_whole_fixed_steps_and_keeps_the_rest() {
        let mut game = Game::new(0);
        assert_eq!(game.advance(STEP * 2.5), 2);
        assert!((game.time - STEP * 2.0).abs() < 1e-6);
        assert_eq!(game.advance(STEP * 0.6), 1);
        assert!((game.time - STEP * 3.0).abs() < 1e-6);
    }

    #[test]
    fn advance_limits_long_frames() {
        let mut game = Game::new(0);
        let steps = game.advance(10.0);
        assert_eq!(steps, (MAX_FRAME / STEP).round() as usize);
    }

    #[test]
    fn pointer_down_grips_the_nearest_open_end() {
        let mut game = Game::new(0);
        let tail = open_tail(&game);
        game.pointer_down(tail + Vec2::new(0.2, 0.1));
        let grip = game.world.grip.expect("grip");
        assert_eq!(
            grip.end,
            EndRef {
                ribbon: 0,
                side: Side::Tail
            }
        );
        assert_eq!(grip.target, tail + Vec2::new(0.2, 0.1));
    }

    #[test]
    fn pointer_down_far_away_or_on_a_fixed_end_grips_nothing() {
        let mut game = Game::new(0);
        game.pointer_down(Vec2::new(0.0, 4.0));
        assert!(game.world.grip.is_none());
        let source = game.world.ribbons[0].end_pos(Side::Head);
        game.pointer_down(source);
        assert!(game.world.grip.is_none());
    }

    #[test]
    fn pointer_move_drags_and_pointer_up_releases() {
        let mut game = Game::new(0);
        let tail = open_tail(&game);
        game.pointer_down(tail);
        game.pointer_move(tail + Vec2::new(0.5, 1.0));
        assert_eq!(game.world.grip.unwrap().target, tail + Vec2::new(0.5, 1.0));
        game.pointer_up();
        assert!(game.world.grip.is_none());
        game.pointer_move(Vec2::ZERO);
        assert!(game.world.grip.is_none());
    }
}
