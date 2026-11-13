use crate::audio::score::Score;
use crate::audio::synth::{Command, Note, Sound, Synth, Timbre};
use crate::game::Game;
use crate::math::{Vec2, smoothstep};

pub const CATALOG_SECONDS: f32 = 29.0;
pub const FRAME: f32 = 1.0 / 60.0;
pub const DRAG_SECONDS: f32 = 1.0;
pub const RELEASE_SECONDS: f32 = 1.4;

fn play(at: f32, note: Note) -> (f32, Command) {
    (at, Command::Play(note))
}

fn sweep(from: f32, to: f32, steps: usize, make: impl Fn(f32) -> Command) -> Vec<(f32, Command)> {
    (0..=steps)
        .map(|k| {
            let s = k as f32 / steps as f32;
            (from + (to - from) * s, make(s))
        })
        .collect()
}

pub fn catalog() -> Vec<(f32, Command)> {
    let root = 110.0;
    let tone = |timbre, ratio: f32| Note::new(Sound::Tone(timbre), root * ratio);
    let pair = |level, d: f32| Command::Pair {
        pitch: root * 3.0,
        detune: 0.03 * d,
        level,
        pan: 0.0,
    };
    let mut c = vec![
        (0.0, Command::Drone { root, tension: 0.0 }),
        play(0.5, tone(Timbre::Wood, 3.0)),
        play(1.2, tone(Timbre::Horn, 5.0)),
        play(2.2, tone(Timbre::Trumpet, 9.0)),
        play(3.2, tone(Timbre::Organ, 2.0)),
    ];
    c.extend(sweep(4.4, 7.4, 60, |s| {
        let d = 2.0 + (0.08 - 2.0) * s;
        pair(smoothstep(2.0, 0.0, d), d)
    }));
    c.extend([
        (7.4, pair(0.0, 0.0)),
        play(7.4, Note::new(Sound::Boom, root * 0.5)),
        play(7.4, Note::new(Sound::Shimmer, root * 12.0)),
        play(7.4, tone(Timbre::Wood, 3.0).gain(0.8)),
    ]);
    c.extend(sweep(9.0, 11.0, 40, |s| Command::Strain {
        level: s,
        pan: 0.0,
    }));
    let fall = Sound::Glide {
        ratio: 0.5,
        seconds: 0.6,
    };
    let reel = Sound::Glide {
        ratio: 2.0,
        seconds: 0.5,
    };
    c.extend([
        (
            11.0,
            Command::Strain {
                level: 0.0,
                pan: 0.0,
            },
        ),
        play(11.0, Note::new(Sound::Pluck, root * 2.0)),
        play(12.0, Note::new(Sound::Rub, root * 5.0)),
        play(13.0, Note::new(Sound::Rip, root * 4.0)),
        play(13.0, Note::new(Sound::Knock, root * 2.0).gain(0.6)),
        play(14.0, tone(Timbre::Wood, 4.0).gain(0.4)),
        play(14.8, Note::new(Sound::Sparkle, root * 16.0).gain(0.7)),
        play(14.8, tone(Timbre::Organ, 2.0).gain(0.35)),
        play(15.8, Note::new(fall, root * 4.0)),
        play(16.8, Note::new(reel, root * 2.0).gain(0.6)),
        play(17.4, tone(Timbre::Wood, 3.0).gain(0.3)),
        play(18.2, Note::new(Sound::Knock, root * 2.0)),
    ]);
    c.extend(sweep(19.0, 22.0, 30, |s| Command::Drone {
        root,
        tension: s,
    }));
    c.push((22.5, Command::Drone { root, tension: 0.0 }));
    for (k, ratio) in [1.0, 1.25, 1.5, 2.0].into_iter().enumerate() {
        c.push(play(
            22.5,
            tone(Timbre::Horn, 4.0 * ratio).delay(0.09 * k as f32),
        ));
    }
    c.extend([
        play(22.5, Note::new(Sound::Shimmer, root * 12.0).delay(0.36)),
        play(22.5, Note::new(Sound::Boom, root * 0.5).gain(0.6)),
        play(25.5, Note::new(Sound::Whoosh, 165.0).gain(0.8)),
        (
            25.5,
            Command::Drone {
                root: 165.0,
                tension: 0.0,
            },
        ),
    ]);
    c.sort_by(|a, b| a.0.total_cmp(&b.0));
    c
}

pub fn render_timeline(timeline: &[(f32, Command)], seconds: f32, rate: f32) -> Vec<f32> {
    let total = (seconds * rate) as usize;
    let mut out = vec![0.0; total * 2];
    let mut synth = Synth::new(rate);
    let mut next = 0;
    let mut frame = 0;
    while frame < total {
        let now = frame as f32 / rate;
        while next < timeline.len() && timeline[next].0 <= now {
            synth.apply(timeline[next].1);
            next += 1;
        }
        let end = (frame + 64).min(total);
        synth.render(&mut out[frame * 2..end * 2]);
        frame = end;
    }
    out
}

pub fn listen(level: usize, drag: Option<(Vec2, Vec2)>, seconds: f32, rate: f32) -> Vec<f32> {
    let mut game = Game::new(level);
    let mut score = Score::new();
    let mut synth = Synth::new(rate);
    let total = (seconds * rate) as usize;
    let per_frame = (FRAME * rate).round() as usize;
    let mut out = vec![0.0; total * 2];
    if let Some((from, _)) = drag {
        game.pointer_down(from);
    }
    let (mut frame, mut k, mut held) = (0, 0, drag.is_some());
    while frame < total {
        let t = k as f32 * FRAME;
        if let Some((from, to)) = drag.filter(|_| held) {
            if t < RELEASE_SECONDS {
                game.pointer_move(from.lerp(to, ((t + FRAME) / DRAG_SECONDS).min(1.0)));
            } else {
                game.pointer_up();
                held = false;
            }
        }
        game.advance(FRAME);
        let cues = game.take_cues();
        for command in score.listen(&game, &cues) {
            synth.apply(command);
        }
        let end = (frame + per_frame).min(total);
        synth.render(&mut out[frame * 2..end * 2]);
        frame = end;
        k += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn bounded(samples: &[f32]) -> bool {
        samples.iter().all(|x| x.is_finite() && x.abs() <= 1.0)
    }

    fn peak(samples: &[f32], from: f32, to: f32) -> f32 {
        let (a, b) = ((from * RATE) as usize * 2, (to * RATE) as usize * 2);
        samples[a..b.min(samples.len())]
            .iter()
            .fold(0.0, |m, x| m.max(x.abs()))
    }

    #[test]
    fn catalog_is_in_time_order_and_uses_every_sound() {
        let timeline = catalog();
        assert!(timeline.windows(2).all(|w| w[0].0 <= w[1].0));
        assert!(timeline.last().unwrap().0 < CATALOG_SECONDS);
        let used: Vec<Sound> = timeline
            .iter()
            .filter_map(|(_, c)| match c {
                Command::Play(n) => Some(n.sound),
                _ => None,
            })
            .collect();
        for wanted in [
            Sound::Tone(Timbre::Wood),
            Sound::Tone(Timbre::Horn),
            Sound::Tone(Timbre::Trumpet),
            Sound::Tone(Timbre::Organ),
            Sound::Boom,
            Sound::Shimmer,
            Sound::Sparkle,
            Sound::Rip,
            Sound::Pluck,
            Sound::Knock,
            Sound::Rub,
            Sound::Whoosh,
        ] {
            assert!(used.contains(&wanted), "{wanted:?}");
        }
        assert!(
            used.iter()
                .any(|s| matches!(s, Sound::Glide { ratio, .. } if *ratio < 1.0))
        );
        assert!(
            used.iter()
                .any(|s| matches!(s, Sound::Glide { ratio, .. } if *ratio > 1.0))
        );
        for kind in ["Pair", "Strain", "Drone"] {
            assert!(
                timeline
                    .iter()
                    .any(|(_, c)| format!("{c:?}").starts_with(kind)),
                "{kind}"
            );
        }
    }

    #[test]
    fn render_timeline_makes_stereo_audio_of_the_right_length() {
        let timeline = vec![(0.1, Command::Play(Note::new(Sound::Knock, 220.0)))];
        let out = render_timeline(&timeline, 0.5, RATE);
        assert_eq!(out.len(), (0.5 * RATE) as usize * 2);
        assert!(peak(&out, 0.0, 0.09) == 0.0);
        assert!(peak(&out, 0.1, 0.2) > 0.05);
        assert!(bounded(&out));
    }

    #[test]
    fn catalog_renders_bounded_audio_with_a_weld_hit() {
        let out = render_timeline(&catalog(), CATALOG_SECONDS, RATE);
        assert!(bounded(&out));
        assert!(peak(&out, 7.4, 7.7) > 2.0 * peak(&out, 7.0, 7.3));
    }

    #[test]
    fn listen_hears_the_game_weld() {
        let drag = Some((Vec2::new(-1.2, -0.6), Vec2::new(1.2, 0.6)));
        let out = listen(0, drag, 3.0, RATE);
        assert_eq!(out.len(), (3.0 * RATE) as usize * 2);
        assert!(bounded(&out));
        assert!(peak(&out, 0.5, 2.0) > 0.2);
    }
}
