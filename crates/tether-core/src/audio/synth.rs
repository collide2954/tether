use crate::audio::dsp::{Env, Noise, Smooth, Svf, advance, pan, sine, soft_clip};
use crate::audio::reverb::Reverb;
use std::f32::consts::{FRAC_1_SQRT_2, TAU};

pub const VOICES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timbre {
    Wood,
    Horn,
    Trumpet,
    Organ,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sound {
    Tone(Timbre),
    Boom,
    Shimmer,
    Sparkle,
    Rip,
    Pluck,
    Knock,
    Glide { ratio: f32, seconds: f32 },
    Rub,
    Whoosh,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    pub sound: Sound,
    pub pitch: f32,
    pub gain: f32,
    pub pan: f32,
    pub delay: f32,
}

impl Note {
    pub fn new(sound: Sound, pitch: f32) -> Self {
        Self {
            sound,
            pitch,
            gain: 1.0,
            pan: 0.0,
            delay: 0.0,
        }
    }

    pub fn gain(self, gain: f32) -> Self {
        Self { gain, ..self }
    }

    pub fn pan(self, pan: f32) -> Self {
        Self { pan, ..self }
    }

    pub fn delay(self, delay: f32) -> Self {
        Self { delay, ..self }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Play(Note),
    Pair {
        pitch: f32,
        detune: f32,
        level: f32,
        pan: f32,
    },
    Strain {
        level: f32,
        pan: f32,
    },
    Drone {
        root: f32,
        tension: f32,
    },
    Master(f32),
}

const WET: f32 = 3.0;
const MAX_LIFE: f32 = 6.0;
const LOWEST_PLUCK: f32 = 30.0;

fn shape(sound: Sound) -> (f32, f32, f32) {
    match sound {
        Sound::Tone(Timbre::Wood) => (0.002, 0.12, 0.25),
        Sound::Tone(Timbre::Horn) => (0.025, 1.2, 0.35),
        Sound::Tone(Timbre::Trumpet) => (0.012, 0.9, 0.35),
        Sound::Tone(Timbre::Organ) => (0.08, 2.4, 0.4),
        Sound::Boom => (0.002, 1.6, 0.08),
        Sound::Shimmer => (0.004, 2.2, 0.6),
        Sound::Sparkle => (0.002, 0.4, 0.5),
        Sound::Rip => (0.004, 0.35, 0.3),
        Sound::Pluck => (0.001, 1.5, 0.3),
        Sound::Knock => (0.001, 0.15, 0.2),
        Sound::Glide { seconds, .. } => (0.015, seconds.max(0.05), 0.35),
        Sound::Rub => (0.03, 0.6, 0.3),
        Sound::Whoosh => (0.5, 0.9, 0.5),
    }
}

#[derive(Clone, Debug)]
struct Voice {
    active: bool,
    note: Note,
    wait: u32,
    age: u32,
    life: u32,
    env: Env,
    phase: [f32; 4],
    svf: Svf,
    noise: Noise,
    gains: (f32, f32),
    send: f32,
    line: Vec<f32>,
    line_len: usize,
    line_pos: usize,
}

impl Voice {
    fn new(line: usize, rate: f32) -> Self {
        Self {
            active: false,
            note: Note::new(Sound::Rub, 0.0),
            wait: 0,
            age: 0,
            life: 0,
            env: Env::new(0.01, 0.01, rate),
            phase: [0.0; 4],
            svf: Svf::default(),
            noise: Noise::new(0),
            gains: (0.0, 0.0),
            send: 0.0,
            line: vec![0.0; line],
            line_len: 2,
            line_pos: 0,
        }
    }

    fn start(&mut self, note: Note, rate: f32, seed: u32) {
        let (attack, decay, send) = shape(note.sound);
        let (l, r) = pan(note.pan);
        self.active = true;
        self.note = note;
        self.wait = (note.delay.max(0.0) * rate) as u32;
        self.age = 0;
        self.life = (MAX_LIFE * rate) as u32;
        self.env = Env::new(attack, decay, rate);
        self.phase = [0.0; 4];
        self.svf = Svf::default();
        self.noise = Noise::new(seed);
        self.gains = (l * note.gain, r * note.gain);
        self.send = send * note.gain;
        if note.sound == Sound::Pluck {
            let len = (rate / note.pitch.max(LOWEST_PLUCK)).round() as usize;
            self.line_len = len.clamp(2, self.line.len());
            self.line_pos = 0;
            for i in 0..self.line_len {
                self.line[i] = self.noise.sample();
            }
        }
    }

    fn fm(&mut self, ratio: f32, index: f32, rate: f32) -> f32 {
        let y = (TAU * self.phase[0] + index * sine(self.phase[1])).sin();
        self.phase[0] = advance(self.phase[0], self.note.pitch, rate);
        self.phase[1] = advance(self.phase[1], self.note.pitch * ratio, rate);
        y
    }

    fn partials(&mut self, freqs: [f32; 3], amps: [f32; 3], rate: f32) -> f32 {
        let mut y = 0.0;
        for k in 0..3 {
            y += sine(self.phase[k]) * amps[k];
            self.phase[k] = advance(self.phase[k], freqs[k], rate);
        }
        y
    }

    fn sample(&mut self, rate: f32) -> f32 {
        if self.wait > 0 {
            self.wait -= 1;
            return 0.0;
        }
        let t = self.age as f32 / rate;
        self.age += 1;
        let e = self.env.tick();
        let f = self.note.pitch;
        let x = match self.note.sound {
            Sound::Tone(Timbre::Wood) => {
                let click = self.noise.sample() * (-t / 0.003).exp() * 0.5;
                (self.fm(3.53, 2.2 * e, rate) + click) * e * 0.6
            }
            Sound::Tone(Timbre::Horn) => self.fm(1.0, 0.4 + 1.4 * e, rate) * e * 0.35,
            Sound::Tone(Timbre::Trumpet) => self.fm(1.0, 0.8 + 2.6 * e, rate) * e * 0.3,
            Sound::Tone(Timbre::Organ) => {
                self.partials([f, 2.0 * f, 4.0 * f], [1.0, 0.45, 0.2], rate) * e * 0.35
            }
            Sound::Boom => {
                let bend = f * (1.0 + 1.6 * (-t / 0.035).exp());
                let y = sine(self.phase[0]);
                self.phase[0] = advance(self.phase[0], bend, rate);
                let thump =
                    self.svf.low(self.noise.sample(), 300.0, 0.7, rate) * (-t / 0.006).exp() * 0.6;
                (y + thump) * e * 0.9
            }
            Sound::Shimmer => {
                let bell = self.partials([f, 1.43 * f, 2.29 * f], [1.0, 0.6, 0.4], rate);
                let air = self.svf.band(self.noise.sample(), 8000.0, 1.5, rate) * (-t / 0.25).exp();
                (bell * 0.5 + air * 0.4) * e * 0.18
            }
            Sound::Sparkle => {
                let bell = self.partials([f, 1.5 * f, 0.0], [1.0, 0.6, 0.0], rate);
                let air = self.svf.band(self.noise.sample(), 9000.0, 1.2, rate);
                (bell * 0.5 + air * 0.3) * e * 0.15
            }
            Sound::Rip => {
                let center = 3500.0 * (500.0f32 / 3500.0).powf((t / 0.3).min(1.0));
                let flutter = 0.6 + 0.4 * sine(self.phase[0]);
                self.phase[0] = advance(self.phase[0], 60.0, rate);
                self.svf.band(self.noise.sample(), center, 1.8, rate) * flutter * e * 0.7
            }
            Sound::Pluck => {
                let out = self.line[self.line_pos];
                let next = self.line[(self.line_pos + 1) % self.line_len];
                self.line[self.line_pos] = 0.5 * (out + next) * 0.996;
                self.line_pos = (self.line_pos + 1) % self.line_len;
                out * e * 0.5
            }
            Sound::Knock => {
                let bend = f * (1.0 + 0.6 * (-t / 0.01).exp());
                let y = sine(self.phase[0]);
                self.phase[0] = advance(self.phase[0], bend, rate);
                (y + self.noise.sample() * (-t / 0.002).exp() * 0.4) * e * 0.6
            }
            Sound::Glide { ratio, seconds } => {
                let bend = f * ratio.powf((t / seconds.max(0.01)).min(1.0));
                self.partials([bend, 2.0 * bend, 0.0], [1.0, 0.3, 0.0], rate) * e * 0.3
            }
            Sound::Rub => {
                self.partials([f, f * 16.0 / 15.0, 0.0], [1.0, 1.0, 0.0], rate) * e * 0.15
            }
            Sound::Whoosh => {
                self.svf
                    .low(self.noise.sample(), 250.0 + 2500.0 * e, 0.8, rate)
                    * e
                    * 0.35
            }
        };
        if self.env.done() || self.age >= self.life {
            self.active = false;
        }
        x
    }
}

#[derive(Clone, Debug)]
struct Pair {
    level: Smooth,
    pitch: Smooth,
    detune: Smooth,
    pan: Smooth,
    phase: [f32; 4],
}

impl Pair {
    fn sample(&mut self, rate: f32) -> (f32, f32) {
        let (level, f, d, p) = (
            self.level.tick(),
            self.pitch.tick(),
            self.detune.tick(),
            self.pan.tick(),
        );
        if level < 1e-7 {
            return (0.0, p);
        }
        let g = f * (1.0 + d);
        let y = 0.5 * (sine(self.phase[0]) + sine(self.phase[1]))
            + 0.15 * (sine(self.phase[2]) + sine(self.phase[3]));
        for (k, freq) in [f, g, 2.0 * f, 2.0 * g].into_iter().enumerate() {
            self.phase[k] = advance(self.phase[k], freq, rate);
        }
        (y * level * 0.25, p)
    }
}

#[derive(Clone, Debug)]
struct Strain {
    level: Smooth,
    pan: Smooth,
    burst: f32,
    fall: f32,
    svf: Svf,
    noise: Noise,
}

impl Strain {
    fn sample(&mut self, rate: f32) -> (f32, f32) {
        let (level, p) = (self.level.tick(), self.pan.tick());
        let density = 3.0 + 90.0 * level * level;
        if self.noise.sample() * 0.5 + 0.5 < density / rate {
            self.burst = 0.5 + 0.5 * self.noise.sample().abs();
        }
        let y = self.svf.band(self.noise.sample(), 3000.0, 0.8, rate) * self.burst * level * 0.6;
        self.burst *= self.fall;
        (y, p)
    }
}

const PARTIALS: [(f32, f32); 4] = [(1.0, 1.0), (2.0, 0.45), (3.0, 0.25), (4.0, 0.12)];
const TENSE: [(f32, f32); 2] = [(2.25, 0.35), (2.8125, 0.25)];

#[derive(Clone, Debug)]
struct Drone {
    root: Smooth,
    level: Smooth,
    tension: Smooth,
    breath: f32,
    phase: [f32; 6],
    svf: Svf,
}

impl Drone {
    fn sample(&mut self, rate: f32) -> f32 {
        let (level, root, t) = (self.level.tick(), self.root.tick(), self.tension.tick());
        if level < 1e-7 {
            return 0.0;
        }
        let mut y = 0.0;
        for (k, (ratio, amp)) in PARTIALS.iter().chain(TENSE.iter()).enumerate() {
            let weight = if k < PARTIALS.len() { *amp } else { amp * t };
            y += sine(self.phase[k]) * weight;
            self.phase[k] = advance(self.phase[k], root * ratio, rate);
        }
        let lfo = 0.75 + 0.25 * sine(self.breath);
        self.breath = advance(self.breath, 1.0 / 7.0 + t * (1.0 / 3.0 - 1.0 / 7.0), rate);
        self.svf.low(y, 700.0 + 1800.0 * t, 0.7, rate) * lfo * level * 0.05
    }
}

pub struct Synth {
    rate: f32,
    voices: Vec<Voice>,
    pair: Pair,
    strain: Strain,
    drone: Drone,
    reverb: Reverb,
    master: Smooth,
    seed: u32,
}

impl Synth {
    pub fn new(rate: f32) -> Self {
        let line = (rate / LOWEST_PLUCK) as usize + 2;
        let smooth = |value, time| Smooth::new(value, time, rate);
        Self {
            rate,
            voices: (0..VOICES).map(|_| Voice::new(line, rate)).collect(),
            pair: Pair {
                level: smooth(0.0, 0.08),
                pitch: smooth(0.0, 0.05),
                detune: smooth(0.0, 0.05),
                pan: smooth(0.0, 0.1),
                phase: [0.0; 4],
            },
            strain: Strain {
                level: smooth(0.0, 0.1),
                pan: smooth(0.0, 0.1),
                burst: 0.0,
                fall: (-1.0 / (0.0015 * rate)).exp(),
                svf: Svf::default(),
                noise: Noise::new(99),
            },
            drone: Drone {
                root: smooth(0.0, 1.5),
                level: smooth(0.0, 2.0),
                tension: smooth(0.0, 1.0),
                breath: 0.0,
                phase: [0.0; 6],
                svf: Svf::default(),
            },
            reverb: Reverb::new(rate, 0.84, 0.3),
            master: smooth(1.0, 0.15),
            seed: 1,
        }
    }

    pub fn apply(&mut self, command: Command) {
        match command {
            Command::Play(note) => {
                let index = self
                    .voices
                    .iter()
                    .position(|v| !v.active)
                    .or_else(|| {
                        self.voices
                            .iter()
                            .enumerate()
                            .max_by_key(|(_, v)| v.age)
                            .map(|(i, _)| i)
                    })
                    .unwrap_or(0);
                self.seed = self.seed.wrapping_add(0x9E37);
                self.voices[index].start(note, self.rate, self.seed);
            }
            Command::Pair {
                pitch,
                detune,
                level,
                pan,
            } => {
                let p = &mut self.pair;
                if p.level.value < 1e-4 {
                    p.pitch.value = pitch;
                    p.detune.value = detune;
                    p.pan.value = pan;
                }
                p.pitch.target = pitch;
                p.detune.target = detune;
                p.level.target = level;
                p.pan.target = pan;
            }
            Command::Strain { level, pan } => {
                self.strain.level.target = level;
                self.strain.pan.target = pan;
            }
            Command::Drone { root, tension } => {
                let d = &mut self.drone;
                if root > 0.0 {
                    if d.level.value < 1e-4 {
                        d.root.value = root;
                    }
                    d.root.target = root;
                    d.level.target = 1.0;
                } else {
                    d.level.target = 0.0;
                }
                d.tension.target = tension;
            }
            Command::Master(level) => self.master.target = level,
        }
    }

    pub fn render(&mut self, out: &mut [f32]) {
        let rate = self.rate;
        for frame in out.as_chunks_mut::<2>().0 {
            let (mut l, mut r, mut send) = (0.0, 0.0, 0.0);
            for v in self.voices.iter_mut().filter(|v| v.active) {
                let x = v.sample(rate);
                l += x * v.gains.0;
                r += x * v.gains.1;
                send += x * v.send;
            }
            let (x, p) = self.pair.sample(rate);
            let (pl, pr) = pan(p);
            l += x * pl;
            r += x * pr;
            send += x * 0.3;
            let (x, p) = self.strain.sample(rate);
            let (sl, sr) = pan(p);
            l += x * sl;
            r += x * sr;
            send += x * 0.15;
            let x = self.drone.sample(rate);
            l += x * FRAC_1_SQRT_2;
            r += x * FRAC_1_SQRT_2;
            send += x * 0.4;
            let (wl, wr) = self.reverb.process(send);
            let m = self.master.tick();
            frame[0] = soft_clip((l + wl * WET) * m);
            frame[1] = soft_clip((r + wr * WET) * m);
        }
    }

    pub fn active(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn render(synth: &mut Synth, seconds: f32) -> Vec<(f32, f32)> {
        let mut buffer = vec![0.0; (seconds * RATE) as usize * 2];
        for chunk in buffer.chunks_mut(512) {
            synth.render(chunk);
        }
        buffer
            .as_chunks::<2>()
            .0
            .iter()
            .map(|f| (f[0], f[1]))
            .collect()
    }

    fn left(frames: &[(f32, f32)]) -> Vec<f32> {
        frames.iter().map(|f| f.0).collect()
    }

    fn rms(xs: &[f32]) -> f32 {
        (xs.iter().map(|x| x * x).sum::<f32>() / xs.len().max(1) as f32).sqrt()
    }

    fn at(seconds: f32) -> usize {
        (seconds * RATE) as usize
    }

    fn brightness(xs: &[f32]) -> f32 {
        let diff: Vec<f32> = xs.windows(2).map(|w| w[1] - w[0]).collect();
        rms(&diff) / rms(xs)
    }

    fn windows(xs: &[f32], size: usize) -> Vec<f32> {
        xs.chunks_exact(size).map(rms).collect()
    }

    fn play(synth: &mut Synth, sound: Sound, pitch: f32) {
        synth.apply(Command::Play(Note::new(sound, pitch)));
    }

    #[test]
    fn synth_is_silent_until_told() {
        let mut s = Synth::new(RATE);
        assert!(render(&mut s, 0.5).iter().all(|f| *f == (0.0, 0.0)));
        assert_eq!(s.active(), 0);
    }

    #[test]
    fn tone_sounds_then_fades_and_frees_its_voice() {
        let mut s = Synth::new(RATE);
        play(&mut s, Sound::Tone(Timbre::Organ), 220.0);
        assert_eq!(s.active(), 1);
        let out = left(&render(&mut s, 4.0));
        let peak = out[..at(0.5)].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.02 && peak <= 1.0, "{peak}");
        assert!(rms(&out[at(3.8)..]) < 0.01 * rms(&out[..at(0.5)]));
        assert_eq!(s.active(), 0);
    }

    #[test]
    fn glide_without_change_holds_its_pitch() {
        let mut s = Synth::new(RATE);
        play(
            &mut s,
            Sound::Glide {
                ratio: 1.0,
                seconds: 1.0,
            },
            220.0,
        );
        let out = left(&render(&mut s, 0.4));
        let part = &out[at(0.1)..at(0.3)];
        let crossings = part
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        assert!((42..=46).contains(&crossings), "{crossings}");
    }

    #[test]
    fn trumpet_is_brighter_than_organ() {
        let tone = |timbre| {
            let mut s = Synth::new(RATE);
            play(&mut s, Sound::Tone(timbre), 330.0);
            let out = left(&render(&mut s, 0.3));
            brightness(&out[at(0.1)..at(0.3)])
        };
        assert!(tone(Timbre::Trumpet) > 1.3 * tone(Timbre::Organ));
    }

    #[test]
    fn voices_are_limited_and_the_oldest_is_stolen() {
        let mut s = Synth::new(RATE);
        for i in 0..40 {
            play(&mut s, Sound::Tone(Timbre::Organ), 110.0 + i as f32);
        }
        assert_eq!(s.active(), VOICES);
        render(&mut s, 0.01);
        assert_eq!(s.active(), VOICES);
    }

    #[test]
    fn delayed_note_waits() {
        let mut s = Synth::new(RATE);
        s.apply(Command::Play(
            Note::new(Sound::Tone(Timbre::Trumpet), 440.0).delay(0.25),
        ));
        let out = left(&render(&mut s, 0.5));
        assert!(out[..at(0.24)].iter().all(|x| x.abs() < 1e-9));
        assert!(rms(&out[at(0.26)..at(0.4)]) > 0.01);
    }

    #[test]
    fn pan_moves_the_dry_sound() {
        let mut s = Synth::new(RATE);
        s.apply(Command::Play(
            Note::new(Sound::Tone(Timbre::Horn), 330.0).pan(-1.0),
        ));
        let out = render(&mut s, 0.3);
        let l: Vec<f32> = out.iter().map(|f| f.0).collect();
        let r: Vec<f32> = out.iter().map(|f| f.1).collect();
        assert!(rms(&l) > 3.0 * rms(&r));
    }

    #[test]
    fn pair_beats_when_detuned_and_holds_when_tuned() {
        let wobble = |detune: f32| {
            let mut s = Synth::new(RATE);
            s.apply(Command::Pair {
                pitch: 300.0,
                detune,
                level: 1.0,
                pan: 0.0,
            });
            let out = left(&render(&mut s, 2.0));
            let w = windows(&out[at(0.6)..], at(0.05));
            let max = w.iter().copied().fold(0.0, f32::max);
            let min = w.iter().copied().fold(f32::INFINITY, f32::min);
            max / min
        };
        assert!(wobble(0.01) > 2.5);
        assert!(wobble(0.0) < 1.3);
    }

    #[test]
    fn pair_fades_out_at_level_zero() {
        let mut s = Synth::new(RATE);
        let pair = |level| Command::Pair {
            pitch: 300.0,
            detune: 0.0,
            level,
            pan: 0.0,
        };
        s.apply(pair(1.0));
        let loud = rms(&left(&render(&mut s, 0.5))[at(0.3)..]);
        s.apply(pair(0.0));
        let out = left(&render(&mut s, 4.0));
        assert!(rms(&out[at(3.5)..]) < 0.01 * loud);
    }

    #[test]
    fn strain_crackles_more_with_more_strain() {
        let crackle = |level| {
            let mut s = Synth::new(RATE);
            s.apply(Command::Strain { level, pan: 0.0 });
            rms(&left(&render(&mut s, 1.5))[at(0.5)..])
        };
        assert_eq!(crackle(0.0), 0.0);
        assert!(crackle(1.0) > 3.0 * crackle(0.25));
        assert!(crackle(0.25) > 0.0);
    }

    #[test]
    fn drone_is_soft_and_tightens_with_tension() {
        let drone = |root, tension| {
            let mut s = Synth::new(RATE);
            s.apply(Command::Drone { root, tension });
            left(&render(&mut s, 6.0))[at(4.0)..].to_vec()
        };
        let calm = drone(110.0, 0.0);
        let tense = drone(110.0, 1.0);
        assert!(rms(&calm) > 0.003 && rms(&calm) < 0.1, "{}", rms(&calm));
        assert!(rms(&tense) > rms(&calm));
        assert!(brightness(&tense) > brightness(&calm));
        assert_eq!(rms(&drone(0.0, 0.0)), 0.0);
    }

    #[test]
    fn master_fades_everything_out() {
        let mut s = Synth::new(RATE);
        s.apply(Command::Drone {
            root: 110.0,
            tension: 0.5,
        });
        render(&mut s, 3.0);
        s.apply(Command::Master(0.0));
        play(&mut s, Sound::Tone(Timbre::Trumpet), 440.0);
        let out = left(&render(&mut s, 2.0));
        assert!(rms(&out[at(1.5)..]) < 1e-4);
        s.apply(Command::Master(1.0));
        let back = left(&render(&mut s, 2.0));
        assert!(rms(&back[at(1.5)..]) > 1e-3);
    }

    #[test]
    fn every_sound_plays_and_ends() {
        let sounds = [
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
            Sound::Glide {
                ratio: 0.5,
                seconds: 0.6,
            },
            Sound::Rub,
            Sound::Whoosh,
        ];
        for sound in sounds {
            let mut s = Synth::new(RATE);
            play(&mut s, sound, 220.0);
            let out = left(&render(&mut s, 0.5));
            assert!(rms(&out) > 1e-3, "{sound:?} is silent");
            render(&mut s, 6.0);
            assert_eq!(s.active(), 0, "{sound:?} never ends");
        }
    }

    #[test]
    fn output_stays_finite_and_bounded_under_load() {
        let mut s = Synth::new(RATE);
        s.apply(Command::Drone {
            root: 110.0,
            tension: 1.0,
        });
        s.apply(Command::Pair {
            pitch: 400.0,
            detune: 0.02,
            level: 1.0,
            pan: 0.5,
        });
        s.apply(Command::Strain {
            level: 1.0,
            pan: -0.5,
        });
        for i in 0..64 {
            s.apply(Command::Play(
                Note::new(Sound::Boom, 50.0 + i as f32).gain(4.0),
            ));
            s.apply(Command::Play(Note::new(Sound::Rip, 300.0).gain(4.0)));
        }
        let out = render(&mut s, 3.0);
        assert!(out.iter().all(|(l, r)| l.is_finite() && r.is_finite()));
        assert!(out.iter().all(|(l, r)| l.abs() <= 1.0 && r.abs() <= 1.0));
    }

    #[test]
    fn notes_build_with_defaults() {
        let n = Note::new(Sound::Rub, 200.0).gain(0.5).pan(0.25).delay(0.1);
        assert_eq!((n.gain, n.pan, n.delay), (0.5, 0.25, 0.1));
        let plain = Note::new(Sound::Rub, 200.0);
        assert_eq!((plain.gain, plain.pan, plain.delay), (1.0, 0.0, 0.0));
    }
}
