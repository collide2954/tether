use crate::audio::synth::{Command, Note, Sound, Timbre};
use crate::game::Game;
use crate::math::smoothstep;
use crate::ribbon::{Class, Side};
use crate::traveler::{Moment, Phase};
use crate::world::{EndRef, Event, GRIP_STRAIN};

pub const BASE: f32 = 110.0;
pub const PAIR_RANGE: f32 = 2.0;
pub const DETUNE_PER_UNIT: f32 = 0.03;
pub const RUB_RANGE: f32 = 0.9;
pub const RUB_REARM: f32 = 1.3;
pub const STRAIN_START: f32 = 1.15;

pub fn root(level: usize) -> f32 {
    let mut r = BASE;
    for _ in 0..level % 12 {
        r *= 1.5;
        if r >= 2.0 * BASE {
            r *= 0.5;
        }
    }
    r
}

pub fn timbre(class: Class) -> Timbre {
    match class {
        Class::Ink => Timbre::Wood,
        Class::Red => Timbre::Horn,
        Class::Yellow => Timbre::Trumpet,
        Class::Blue => Timbre::Organ,
    }
}

pub fn pitch(root: f32, class: Class) -> f32 {
    root * match class {
        Class::Blue => 2.0,
        Class::Ink => 3.0,
        Class::Red => 5.0,
        Class::Yellow => 9.0,
    }
}

pub fn pan_of(x: f32) -> f32 {
    (x / 8.0).clamp(-1.0, 1.0) * 0.8
}

fn play(out: &mut Vec<Command>, note: Note) {
    out.push(Command::Play(note));
}

#[derive(Clone, Debug)]
pub struct Score {
    level: Option<usize>,
    grip: Option<EndRef>,
    rub_armed: bool,
    reeling: bool,
    pair_pitch: f32,
}

impl Default for Score {
    fn default() -> Self {
        Self::new()
    }
}

impl Score {
    pub fn new() -> Self {
        Self {
            level: None,
            grip: None,
            rub_armed: true,
            reeling: false,
            pair_pitch: BASE,
        }
    }

    pub fn listen(&mut self, game: &mut Game) -> Vec<Command> {
        let events = std::mem::take(&mut game.world.events);
        let moments = std::mem::take(&mut game.moments);
        let mut out = Vec::new();
        let key = root(game.level);
        if self.level.is_some_and(|level| level != game.level) {
            play(&mut out, Note::new(Sound::Whoosh, key).gain(0.8));
        }
        self.level = Some(game.level);
        let world = &game.world;
        let class_of = |e: EndRef| {
            world.ribbons[e.ribbon]
                .end(e.side)
                .class()
                .unwrap_or(Class::Ink)
        };
        let grip = world.grip.map(|g| g.end);
        let held = grip.map(|end| world.ribbons[end.ribbon].free_strain());
        let tension = match (game.complete, held) {
            (None, Some(s)) => smoothstep(1.0, GRIP_STRAIN, s),
            _ => 0.0,
        };
        out.push(Command::Drone { root: key, tension });
        if let Some(end) = grip.filter(|end| self.grip != Some(*end)) {
            let class = class_of(end);
            let note = Note::new(Sound::Tone(timbre(class)), pitch(key, class));
            play(&mut out, note.gain(0.5).pan(pan_of(world.end_pos(end).x)));
        }
        self.grip = grip;
        match grip {
            Some(end) => {
                let pan = pan_of(world.end_pos(end).x);
                let class = class_of(end);
                self.pair_pitch = pitch(key, class);
                let (detune, level) = world
                    .nearest_compatible(end, PAIR_RANGE)
                    .map_or((0.0, 0.0), |(_, d)| {
                        (DETUNE_PER_UNIT * d, smoothstep(PAIR_RANGE, 0.0, d))
                    });
                out.push(Command::Pair {
                    pitch: self.pair_pitch,
                    detune,
                    level,
                    pan,
                });
                match world.nearest_clash(end, RUB_REARM) {
                    Some((_, d)) if d < RUB_RANGE && self.rub_armed => {
                        play(&mut out, Note::new(Sound::Rub, pitch(key, class)).pan(pan));
                        self.rub_armed = false;
                    }
                    Some(_) => {}
                    None => self.rub_armed = true,
                }
                let level = match (world.partner(end), held) {
                    (None, Some(s)) => smoothstep(STRAIN_START, GRIP_STRAIN, s),
                    _ => 0.0,
                };
                out.push(Command::Strain { level, pan });
            }
            None => {
                out.push(Command::Pair {
                    pitch: self.pair_pitch,
                    detune: 0.0,
                    level: 0.0,
                    pan: 0.0,
                });
                out.push(Command::Strain {
                    level: 0.0,
                    pan: 0.0,
                });
                self.rub_armed = true;
            }
        }
        for event in events {
            match event {
                Event::Welded(weld) => {
                    let pan = pan_of(world.end_pos(weld.a).x);
                    let class = class_of(weld.a);
                    play(&mut out, Note::new(Sound::Boom, key * 0.5).pan(pan));
                    play(&mut out, Note::new(Sound::Shimmer, key * 12.0).pan(pan));
                    let tone = Note::new(Sound::Tone(timbre(class)), pitch(key, class));
                    play(&mut out, tone.gain(0.8).pan(pan));
                }
                Event::Tore(weld) => {
                    let pan = pan_of(world.end_pos(weld.a).x);
                    play(&mut out, Note::new(Sound::Rip, key * 4.0).pan(pan));
                    play(
                        &mut out,
                        Note::new(Sound::Knock, key * 2.0).gain(0.6).pan(pan),
                    );
                }
                Event::Slipped(end) => {
                    let pan = pan_of(world.end_pos(end).x);
                    play(&mut out, Note::new(Sound::Pluck, key * 2.0).pan(pan));
                }
            }
        }
        let traveler = game.traveler.as_ref();
        let pan = traveler.map_or(0.0, |t| pan_of(t.position(world).x));
        for moment in moments {
            match moment {
                Moment::Departed => {
                    let note = Note::new(Sound::Tone(Timbre::Wood), key * 4.0);
                    play(&mut out, note.gain(0.4).pan(pan));
                }
                Moment::Crossed => {
                    play(
                        &mut out,
                        Note::new(Sound::Sparkle, key * 16.0).gain(0.7).pan(pan),
                    );
                    if let Some(t) = traveler {
                        let side = if t.dir > 0.0 { Side::Head } else { Side::Tail };
                        let class = class_of(EndRef {
                            ribbon: t.ribbon,
                            side,
                        });
                        let tone = Note::new(Sound::Tone(timbre(class)), pitch(key, class));
                        play(&mut out, tone.gain(0.35).pan(pan));
                    }
                }
                Moment::Fell => {
                    let glide = Sound::Glide {
                        ratio: 0.5,
                        seconds: 0.6,
                    };
                    play(&mut out, Note::new(glide, key * 4.0).pan(pan));
                }
                Moment::Stopped => play(&mut out, Note::new(Sound::Knock, key * 2.0).pan(pan)),
                Moment::Reeled => {
                    let note = Note::new(Sound::Tone(Timbre::Wood), key * 3.0);
                    play(&mut out, note.gain(0.3).pan(pan));
                }
                Moment::Arrived => {
                    for (k, ratio) in [1.0, 1.25, 1.5, 2.0].into_iter().enumerate() {
                        let note = Note::new(Sound::Tone(Timbre::Horn), key * 4.0 * ratio);
                        play(&mut out, note.delay(0.09 * k as f32).pan(pan));
                    }
                    play(
                        &mut out,
                        Note::new(Sound::Shimmer, key * 12.0).delay(0.36).pan(pan),
                    );
                    play(
                        &mut out,
                        Note::new(Sound::Boom, key * 0.5).gain(0.6).pan(pan),
                    );
                }
            }
        }
        let reeling = traveler.is_some_and(|t| t.phase == Phase::Reeling);
        if reeling && !self.reeling {
            let glide = Sound::Glide {
                ratio: 2.0,
                seconds: 0.5,
            };
            play(&mut out, Note::new(glide, key * 2.0).gain(0.6).pan(pan));
        }
        self.reeling = reeling;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Level;
    use crate::math::Vec2;
    use crate::world::{Grip, Weld};

    const PAIR: &str = "(ribbons: [
        (points: [(-5.0, 0.0), (-1.0, 0.0)], head: Source, tail: Open(Ink)),
        (points: [(1.0, 0.0), (5.0, 0.0)], head: Open(Ink), tail: Goal),
    ])";
    const CLASH: &str = "(ribbons: [
        (points: [(-5.0, 0.0), (-1.0, 0.0)], head: Source, tail: Open(Red)),
        (points: [(1.0, 0.0), (5.0, 0.0)], head: Open(Blue), tail: Goal),
    ])";
    const A_TAIL: EndRef = EndRef {
        ribbon: 0,
        side: Side::Tail,
    };
    const B_HEAD: EndRef = EndRef {
        ribbon: 1,
        side: Side::Head,
    };

    fn game(text: &str) -> Game {
        Game::with_world(Level::parse(text).unwrap().build(), 0)
    }

    fn hold_at(game: &mut Game, at: Vec2) {
        let r = &mut game.world.ribbons[0];
        let i = r.index(Side::Tail);
        r.pos[i] = at;
        game.world.grip = Some(Grip {
            end: A_TAIL,
            target: at,
        });
    }

    fn notes(commands: &[Command]) -> Vec<Note> {
        commands
            .iter()
            .filter_map(|c| match c {
                Command::Play(n) => Some(*n),
                _ => None,
            })
            .collect()
    }

    fn sounds(commands: &[Command]) -> Vec<Sound> {
        notes(commands).iter().map(|n| n.sound).collect()
    }

    fn pair(commands: &[Command]) -> (f32, f32) {
        commands
            .iter()
            .find_map(|c| match c {
                Command::Pair { detune, level, .. } => Some((*detune, *level)),
                _ => None,
            })
            .expect("pair")
    }

    fn strain(commands: &[Command]) -> f32 {
        commands
            .iter()
            .find_map(|c| match c {
                Command::Strain { level, .. } => Some(*level),
                _ => None,
            })
            .expect("strain")
    }

    fn drone(commands: &[Command]) -> (f32, f32) {
        commands
            .iter()
            .find_map(|c| match c {
                Command::Drone { root, tension } => Some((*root, *tension)),
                _ => None,
            })
            .expect("drone")
    }

    fn rubs(commands: &[Command]) -> usize {
        sounds(commands)
            .iter()
            .filter(|s| **s == Sound::Rub)
            .count()
    }

    #[test]
    fn root_walks_the_circle_of_fifths_in_one_octave() {
        assert_eq!(root(0), BASE);
        assert!((root(1) - 165.0).abs() < 1e-3);
        assert!((root(2) - 123.75).abs() < 1e-3);
        for level in 0..30 {
            assert!((BASE..2.0 * BASE).contains(&root(level)));
        }
    }

    #[test]
    fn classes_have_their_voice_and_register() {
        assert_eq!(timbre(Class::Ink), Timbre::Wood);
        assert_eq!(timbre(Class::Red), Timbre::Horn);
        assert_eq!(timbre(Class::Yellow), Timbre::Trumpet);
        assert_eq!(timbre(Class::Blue), Timbre::Organ);
        let r = 110.0;
        assert!(pitch(r, Class::Blue) < pitch(r, Class::Ink));
        assert!(pitch(r, Class::Ink) < pitch(r, Class::Red));
        assert!(pitch(r, Class::Red) < pitch(r, Class::Yellow));
        assert_eq!(pitch(r, Class::Blue), 220.0);
    }

    #[test]
    fn pan_follows_the_frame() {
        assert_eq!(pan_of(0.0), 0.0);
        assert!((pan_of(-8.0) + 0.8).abs() < 1e-6);
        assert!((pan_of(30.0) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn first_listen_starts_the_drone_without_a_whoosh() {
        let mut g = game(PAIR);
        let commands = Score::new().listen(&mut g);
        assert_eq!(drone(&commands), (root(0), 0.0));
        assert!(!sounds(&commands).contains(&Sound::Whoosh));
    }

    #[test]
    fn a_new_level_whooshes_and_moves_the_drone() {
        let mut g = game(PAIR);
        let mut score = Score::new();
        score.listen(&mut g);
        g.level = 1;
        let commands = score.listen(&mut g);
        assert!(sounds(&commands).contains(&Sound::Whoosh));
        assert_eq!(drone(&commands).0, root(1));
    }

    #[test]
    fn a_grip_plays_one_pickup_tone() {
        let mut g = game(PAIR);
        let mut score = Score::new();
        hold_at(&mut g, Vec2::new(-1.0, 0.0));
        let first = notes(&score.listen(&mut g));
        assert!(first.iter().any(|n| n.sound == Sound::Tone(Timbre::Wood)));
        let second = notes(&score.listen(&mut g));
        assert!(!second.iter().any(|n| n.sound == Sound::Tone(Timbre::Wood)));
    }

    #[test]
    fn a_near_compatible_end_tunes_toward_unison() {
        let mut g = game(PAIR);
        let mut score = Score::new();
        hold_at(&mut g, Vec2::new(-0.5, 0.0));
        let (far_detune, far_level) = pair(&score.listen(&mut g));
        hold_at(&mut g, Vec2::new(0.7, 0.0));
        let (near_detune, near_level) = pair(&score.listen(&mut g));
        assert!(far_level > 0.0 && near_level > far_level);
        assert!(near_detune < far_detune);
        assert!((far_detune - DETUNE_PER_UNIT * 1.5).abs() < 1e-4);
        g.world.grip = None;
        assert_eq!(pair(&score.listen(&mut g)).1, 0.0);
    }

    #[test]
    fn a_clashing_end_rubs_once_until_rearmed() {
        let mut g = game(CLASH);
        let mut score = Score::new();
        hold_at(&mut g, Vec2::new(0.5, 0.0));
        assert_eq!(rubs(&score.listen(&mut g)), 1);
        assert_eq!(rubs(&score.listen(&mut g)), 0);
        hold_at(&mut g, Vec2::new(-1.0, 0.0));
        assert_eq!(rubs(&score.listen(&mut g)), 0);
        hold_at(&mut g, Vec2::new(0.5, 0.0));
        assert_eq!(rubs(&score.listen(&mut g)), 1);
    }

    #[test]
    fn strain_crackle_follows_the_free_strain() {
        let mut g = game(PAIR);
        let mut score = Score::new();
        assert_eq!(strain(&score.listen(&mut g)), 0.0);
        hold_at(&mut g, Vec2::new(-1.0, 3.0));
        assert!(strain(&score.listen(&mut g)) > 0.3);
    }

    #[test]
    fn drone_tightens_with_strain_and_resolves_on_completion() {
        let mut g = game(PAIR);
        let mut score = Score::new();
        hold_at(&mut g, Vec2::new(-1.0, 3.0));
        assert!(drone(&score.listen(&mut g)).1 > 0.3);
        g.complete = Some(0.0);
        assert_eq!(drone(&score.listen(&mut g)).1, 0.0);
    }

    #[test]
    fn world_events_map_to_sounds_and_drain() {
        let mut g = game(PAIR);
        let mut score = Score::new();
        let weld = Weld {
            a: A_TAIL,
            b: B_HEAD,
        };
        g.world.events.push(Event::Welded(weld));
        let welded = sounds(&score.listen(&mut g));
        assert!(welded.contains(&Sound::Boom));
        assert!(welded.contains(&Sound::Shimmer));
        assert!(welded.contains(&Sound::Tone(Timbre::Wood)));
        assert!(g.world.events.is_empty());
        g.world.events.push(Event::Tore(weld));
        assert!(sounds(&score.listen(&mut g)).contains(&Sound::Rip));
        g.world.events.push(Event::Slipped(A_TAIL));
        assert!(sounds(&score.listen(&mut g)).contains(&Sound::Pluck));
    }

    #[test]
    fn traveler_moments_map_to_sounds_and_drain() {
        let mut g = game(PAIR);
        let mut score = Score::new();
        score.listen(&mut g);
        let mut hear = |m: Moment| {
            g.moments.push(m);
            let commands = score.listen(&mut g);
            assert!(g.moments.is_empty());
            notes(&commands)
        };
        assert!(
            hear(Moment::Departed)
                .iter()
                .any(|n| n.sound == Sound::Tone(Timbre::Wood))
        );
        assert!(
            hear(Moment::Crossed)
                .iter()
                .any(|n| n.sound == Sound::Sparkle)
        );
        assert!(
            hear(Moment::Fell)
                .iter()
                .any(|n| matches!(n.sound, Sound::Glide { ratio, .. } if ratio < 1.0))
        );
        assert!(
            hear(Moment::Stopped)
                .iter()
                .any(|n| n.sound == Sound::Knock)
        );
        let arrival = hear(Moment::Arrived);
        let mut delays: Vec<f32> = arrival.iter().map(|n| n.delay).collect();
        delays.sort_by(f32::total_cmp);
        delays.dedup();
        assert!(delays.len() >= 4);
    }

    #[test]
    fn reeling_plays_one_rising_glide() {
        let mut g = game(PAIR);
        let mut score = Score::new();
        score.listen(&mut g);
        g.traveler.as_mut().unwrap().phase = Phase::Reeling;
        let rising = |c: &[Command]| {
            notes(c)
                .iter()
                .filter(|n| matches!(n.sound, Sound::Glide { ratio, .. } if ratio > 1.0))
                .count()
        };
        assert_eq!(rising(&score.listen(&mut g)), 1);
        assert_eq!(rising(&score.listen(&mut g)), 0);
    }
}
