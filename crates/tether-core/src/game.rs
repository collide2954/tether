use crate::color::Palette;
use crate::level::{LEVELS, Level};
use crate::math::{Vec2, smoothstep};
use crate::mesh::Mesh;
use crate::ribbon::Side;
use crate::scene::{draw_halo, draw_ribbons, draw_shadows, draw_traveler};
use crate::traveler::{Moment, Traveler};
use crate::world::{EndRef, Grip, World};

pub const STEP: f32 = 1.0 / 120.0;
pub const MAX_FRAME: f32 = 0.25;
pub const GRAB_RADIUS: f32 = 0.45;
pub const HOLD: f32 = 1.8;
pub const FADE: f32 = 0.6;

#[derive(Clone, Debug)]
pub struct Game {
    pub world: World,
    pub traveler: Option<Traveler>,
    pub level: usize,
    pub time: f32,
    pub age: f32,
    pub complete: Option<f32>,
    pub moments: Vec<Moment>,
    accumulator: f32,
}

impl Game {
    pub fn new(level: usize) -> Self {
        let level = level % LEVELS.len();
        let world = Level::parse(LEVELS[level])
            .map(|l| l.build())
            .unwrap_or_default();
        Self::with_world(world, level)
    }

    pub fn with_world(world: World, level: usize) -> Self {
        Self {
            traveler: Traveler::new(&world),
            world,
            level,
            time: 0.0,
            age: 0.0,
            complete: None,
            moments: Vec::new(),
            accumulator: 0.0,
        }
    }

    fn step(&mut self) {
        self.world.step(STEP);
        self.time += STEP;
        self.age += STEP;
        if let Some(traveler) = &mut self.traveler {
            let moments = traveler.step(&self.world, STEP);
            if moments.contains(&Moment::Arrived) {
                self.complete = Some(0.0);
            }
            self.moments.extend(moments);
        }
        if let Some(held) = self.complete {
            if held + STEP >= HOLD {
                let next = Self::new(self.level + 1);
                let (time, moments) = (self.time, std::mem::take(&mut self.moments));
                *self = Self {
                    time,
                    moments,
                    ..next
                };
            } else {
                self.complete = Some(held + STEP);
            }
        }
    }

    pub fn advance(&mut self, dt: f32) -> usize {
        self.accumulator += dt.clamp(0.0, MAX_FRAME);
        let mut steps = 0;
        while self.accumulator + 1e-6 >= STEP {
            self.accumulator = (self.accumulator - STEP).max(0.0);
            self.step();
            steps += 1;
        }
        steps
    }

    pub fn pointer_down(&mut self, at: Vec2) {
        if self.complete.is_some() {
            return;
        }
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

    pub fn veil(&self) -> f32 {
        let entry = 1.0 - smoothstep(0.0, FADE, self.age);
        let exit = self
            .complete
            .map_or(0.0, |held| smoothstep(HOLD - FADE, HOLD, held));
        entry.max(exit)
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
        draw_shadows(&self.world, palette, mesh);
        if let Some(traveler) = &self.traveler {
            draw_halo(&self.world, traveler, palette, self.time, mesh);
        }
        draw_ribbons(&self.world, palette, self.time, mesh);
        if let Some(traveler) = &self.traveler {
            draw_traveler(&self.world, traveler, palette, self.time, mesh);
        }
        let veil = self.veil();
        if veil > 0.0 {
            mesh.square(Vec2::ZERO, VEIL_REACH, 0.0, palette.ground.with_alpha(veil));
        }
    }
}

const VEIL_REACH: f32 = 20.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traveler::{PAUSE, Phase};

    fn open_tail(game: &Game) -> Vec2 {
        game.world.ribbons[0].end_pos(Side::Tail)
    }

    fn run(game: &mut Game, seconds: f32) {
        for _ in 0..(seconds / STEP).round() as usize {
            game.advance(STEP);
        }
    }

    fn short_line() -> World {
        Level::parse("(ribbons: [(points: [(-0.5, 0.0), (0.5, 0.0)], head: Source, tail: Goal)])")
            .unwrap()
            .build()
    }

    #[test]
    fn new_game_places_the_traveler_at_the_source() {
        let game = Game::new(0);
        let t = game.traveler.as_ref().expect("traveler");
        assert!(matches!(t.phase, Phase::Waiting { .. }));
        assert!(game.complete.is_none());
    }

    #[test]
    fn advance_moves_the_traveler() {
        let mut game = Game::new(0);
        for _ in 0..((PAUSE + 0.3) / 0.1) as usize {
            game.advance(0.1);
        }
        assert_eq!(game.traveler.as_ref().unwrap().phase, Phase::Moving);
        assert!(game.moments.contains(&Moment::Departed));
    }

    #[test]
    fn arrival_completes_the_level() {
        let mut game = Game::with_world(short_line(), 0);
        for _ in 0..12 {
            game.advance(0.15);
        }
        assert!(game.complete.is_some());
        assert!(game.moments.contains(&Moment::Arrived));
    }

    #[test]
    fn completed_level_folds_into_the_next_after_a_hold() {
        let mut game = Game::with_world(short_line(), 0);
        for _ in 0..12 {
            game.advance(0.15);
        }
        for _ in 0..((HOLD / 0.2) as usize + 2) {
            game.advance(0.2);
        }
        assert_eq!(game.level, 1 % LEVELS.len());
        assert!(game.complete.is_none());
        assert!(matches!(
            game.traveler.as_ref().unwrap().phase,
            Phase::Waiting { .. }
        ));
    }

    #[test]
    fn draw_puts_the_halo_under_the_ribbons_and_the_core_on_top() {
        use crate::mesh::Kind;
        let game = Game::new(0);
        let palette = Palette::paper();
        let mut mesh = Mesh::new(0.01);
        game.draw(&palette, &mut mesh);
        let kind = |k: Kind| k as u32 as f32;
        let first = |test: &dyn Fn(&crate::mesh::Vertex) -> bool| {
            mesh.vertices.iter().position(test).expect("vertex")
        };
        let glow = first(&|v| v.shape[0] == kind(Kind::Glow));
        let ink =
            first(&|v| v.shape[0] == kind(Kind::Band) && v.color == palette.ink.premultiplied());
        let core =
            first(&|v| v.shape[0] == kind(Kind::Disc) && v.color == palette.pulse.premultiplied());
        assert!(glow < ink && ink < core);
    }

    #[test]
    fn veil_fades_in_on_entry_and_out_after_completion() {
        let mut game = Game::with_world(short_line(), 0);
        assert_eq!(game.veil(), 1.0);
        run(&mut game, FADE / 2.0);
        assert!(game.veil() > 0.0 && game.veil() < 1.0);
        run(&mut game, FADE);
        assert_eq!(game.veil(), 0.0);
        game.complete = Some(HOLD - FADE / 2.0);
        assert!(game.veil() > 0.0 && game.veil() < 1.0);
        game.complete = Some(HOLD);
        assert_eq!(game.veil(), 1.0);
    }

    #[test]
    fn draw_covers_the_frame_with_the_veil() {
        use crate::mesh::Kind;
        let game = Game::new(0);
        let palette = Palette::paper();
        let mut mesh = Mesh::new(0.01);
        game.draw(&palette, &mut mesh);
        let last = mesh.vertices.last().expect("vertex");
        assert_eq!(last.shape[0], Kind::Square as u32 as f32);
        assert_eq!(last.color, palette.ground.with_alpha(1.0).premultiplied());
    }

    #[test]
    fn pointer_does_nothing_after_completion() {
        let mut game = Game::new(0);
        game.complete = Some(0.0);
        let tail = open_tail(&game);
        game.pointer_down(tail);
        assert!(game.world.grip.is_none());
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
