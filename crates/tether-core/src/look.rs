use crate::game::{Cues, Game};
use crate::ribbon::Side;

pub const LIFT_TIME: f32 = 0.18;

#[derive(Clone, Debug, Default)]
pub struct Look {
    pub lifts: Vec<[f32; 2]>,
    level: Option<usize>,
}

pub fn side_index(side: Side) -> usize {
    match side {
        Side::Head => 0,
        Side::Tail => 1,
    }
}

impl Look {
    pub fn update(&mut self, game: &Game, _cues: &Cues, dt: f32) {
        let count = game.world.ribbons.len();
        if self.level != Some(game.level) || self.lifts.len() != count {
            self.lifts = vec![[0.0; 2]; count];
            self.level = Some(game.level);
        }
        let k = 1.0 - (-dt / LIFT_TIME).exp();
        let held = game
            .world
            .grip
            .map(|g| g.end)
            .filter(|end| game.world.partner(*end).is_none());
        for (ribbon, lift) in self.lifts.iter_mut().enumerate() {
            for side in [Side::Head, Side::Tail] {
                let up = held.is_some_and(|end| end.ribbon == ribbon && end.side == side);
                let value = &mut lift[side_index(side)];
                *value += (f32::from(u8::from(up)) - *value) * k;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec2;
    use crate::world::{EndRef, Grip};

    fn hold(game: &mut Game, ribbon: usize, side: Side) {
        let end = EndRef { ribbon, side };
        let at = game.world.end_pos(end);
        game.world.grip = Some(Grip { end, target: at });
    }

    fn run(look: &mut Look, game: &Game, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            look.update(game, &Cues::default(), 1.0 / 60.0);
        }
    }

    #[test]
    fn lifts_match_the_ribbon_count() {
        let game = Game::new(0);
        let mut look = Look::default();
        look.update(&game, &Cues::default(), 0.0);
        assert_eq!(look.lifts, vec![[0.0, 0.0]; game.world.ribbons.len()]);
    }

    #[test]
    fn the_gripped_end_lifts_and_falls_back_after_release() {
        let mut game = Game::new(0);
        let mut look = Look::default();
        hold(&mut game, 0, Side::Tail);
        run(&mut look, &game, 0.5);
        assert!(look.lifts[0][1] > 0.9);
        assert_eq!(look.lifts[0][0], 0.0);
        assert_eq!(look.lifts[1], [0.0, 0.0]);
        game.world.grip = None;
        run(&mut look, &game, 0.5);
        assert!(look.lifts[0][1] < 0.1);
    }

    #[test]
    fn a_new_level_resets_the_lifts() {
        let mut game = Game::new(0);
        let mut look = Look::default();
        hold(&mut game, 0, Side::Tail);
        run(&mut look, &game, 0.5);
        let mut next = Game::new(2);
        next.world.grip = None;
        look.update(&next, &Cues::default(), 1.0 / 60.0);
        assert_eq!(look.lifts.len(), next.world.ribbons.len());
        assert!(look.lifts.iter().all(|l| l[0] < 0.2 && l[1] < 0.2));
        let _ = Vec2::ZERO;
    }

    #[test]
    fn side_index_orders_head_then_tail() {
        assert_eq!(side_index(Side::Head), 0);
        assert_eq!(side_index(Side::Tail), 1);
    }
}
