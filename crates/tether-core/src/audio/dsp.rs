use std::f32::consts::{FRAC_PI_4, PI, TAU};

#[derive(Clone, Copy, Debug)]
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u32) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9).wrapping_add(0x6D2B_79F5) | 1)
    }

    pub fn sample(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OnePole {
    z: f32,
    a: f32,
}

impl OnePole {
    pub fn lowpass(cutoff: f32, rate: f32) -> Self {
        Self {
            z: 0.0,
            a: 1.0 - (-TAU * cutoff / rate).exp(),
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        self.z += self.a * (x - self.z);
        self.z
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
}

impl Svf {
    pub fn band(&mut self, x: f32, cutoff: f32, q: f32, rate: f32) -> f32 {
        self.tick(x, cutoff, q, rate).0
    }

    pub fn low(&mut self, x: f32, cutoff: f32, q: f32, rate: f32) -> f32 {
        self.tick(x, cutoff, q, rate).1
    }

    fn tick(&mut self, x: f32, cutoff: f32, q: f32, rate: f32) -> (f32, f32) {
        let g = (PI * (cutoff / rate).clamp(1e-5, 0.49)).tan();
        let k = 1.0 / q.max(0.05);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v1, v2)
    }
}

const SIXTY_DECIBELS: f32 = 6.907_755;

#[derive(Clone, Copy, Debug)]
pub struct Env {
    level: f32,
    rise: f32,
    fall: f32,
    rising: bool,
}

impl Env {
    pub fn new(attack: f32, decay: f32, rate: f32) -> Self {
        Self {
            level: 0.0,
            rise: 1.0 / (attack * rate).max(1.0),
            fall: (-SIXTY_DECIBELS / (decay * rate).max(1.0)).exp(),
            rising: true,
        }
    }

    pub fn tick(&mut self) -> f32 {
        if self.rising {
            self.level += self.rise;
            if self.level >= 1.0 {
                self.level = 1.0;
                self.rising = false;
            }
        } else {
            self.level *= self.fall;
        }
        self.level
    }

    pub fn done(&self) -> bool {
        !self.rising && self.level < 1e-4
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Smooth {
    pub value: f32,
    pub target: f32,
    coeff: f32,
}

impl Smooth {
    pub fn new(value: f32, time: f32, rate: f32) -> Self {
        Self {
            value,
            target: value,
            coeff: 1.0 - (-1.0 / (time * rate).max(1.0)).exp(),
        }
    }

    pub fn tick(&mut self) -> f32 {
        self.value += (self.target - self.value) * self.coeff;
        self.value
    }
}

pub fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    (x * (27.0 + x * x) / (27.0 + 9.0 * x * x)).clamp(-1.0, 1.0)
}

pub fn pan(position: f32) -> (f32, f32) {
    let angle = (position.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
    (angle.cos(), angle.sin())
}

pub fn sine(phase: f32) -> f32 {
    (TAU * phase).sin()
}

pub fn advance(phase: f32, freq: f32, rate: f32) -> f32 {
    let p = phase + freq / rate;
    p - p.floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn rms(xs: &[f32]) -> f32 {
        (xs.iter().map(|x| x * x).sum::<f32>() / xs.len() as f32).sqrt()
    }

    fn tone(freq: f32, n: usize) -> Vec<f32> {
        let mut phase = 0.0;
        (0..n)
            .map(|_| {
                let s = sine(phase);
                phase = advance(phase, freq, RATE);
                s
            })
            .collect()
    }

    #[test]
    fn noise_is_bounded_centered_and_seeded() {
        let mut a = Noise::new(7);
        let xs: Vec<f32> = (0..20_000).map(|_| a.sample()).collect();
        assert!(xs.iter().all(|x| (-1.0..1.0).contains(x)));
        let mean = xs.iter().sum::<f32>() / xs.len() as f32;
        assert!(mean.abs() < 0.03);
        let mut b = Noise::new(7);
        let mut c = Noise::new(8);
        assert_eq!(b.sample(), xs[0]);
        assert_ne!(c.sample(), xs[0]);
        assert!(Noise::new(0).sample().is_finite());
    }

    #[test]
    fn one_pole_lowpass_follows_a_step_with_its_time_constant() {
        let cutoff = 100.0;
        let mut f = OnePole::lowpass(cutoff, RATE);
        let tau = (RATE / (TAU * cutoff)).round() as usize;
        let mut y = 0.0;
        for _ in 0..tau {
            y = f.process(1.0);
        }
        assert!((y - 0.632).abs() < 0.02, "{y}");
        for _ in 0..tau * 6 {
            y = f.process(1.0);
        }
        assert!((y - 1.0).abs() < 0.01);
    }

    #[test]
    fn band_pass_prefers_its_center() {
        let run = |freq: f32| {
            let mut f = Svf::default();
            let out: Vec<f32> = tone(freq, 9600)
                .into_iter()
                .map(|x| f.band(x, 1000.0, 4.0, RATE))
                .collect();
            rms(&out[4800..])
        };
        assert!(run(1000.0) > 3.0 * run(8000.0));
        assert!(run(1000.0) > 3.0 * run(125.0));
    }

    #[test]
    fn low_pass_keeps_lows_and_cuts_highs() {
        let run = |freq: f32| {
            let mut f = Svf::default();
            let out: Vec<f32> = tone(freq, 9600)
                .into_iter()
                .map(|x| f.low(x, 500.0, 0.707, RATE))
                .collect();
            rms(&out[4800..])
        };
        assert!(run(100.0) > 0.6);
        assert!(run(8000.0) < 0.02);
    }

    #[test]
    fn envelope_rises_then_decays_to_done() {
        let mut env = Env::new(0.01, 0.1, RATE);
        let first = env.tick();
        assert!(first > 0.0 && first <= 1.0 / (0.01 * RATE) + 1e-6);
        let mut peak: f32 = 0.0;
        for _ in 0..480 {
            peak = peak.max(env.tick());
        }
        assert!(peak > 0.99);
        assert!(!env.done());
        for _ in 0..(0.1 * 7.0 * RATE) as usize {
            env.tick();
        }
        assert!(env.tick() < 1e-3);
        for _ in 0..(0.1 * 3.0 * RATE) as usize {
            env.tick();
        }
        assert!(env.done());
    }

    #[test]
    fn envelope_decay_is_a_sixty_decibel_time() {
        let mut env = Env::new(0.01, 0.5, RATE);
        for _ in 0..((0.01 + 0.5) * RATE) as usize {
            env.tick();
        }
        let level = env.tick();
        assert!((level - 1e-3).abs() < 2e-4, "{level}");
    }

    #[test]
    fn smooth_moves_toward_its_target() {
        let mut s = Smooth::new(0.0, 0.05, RATE);
        s.target = 1.0;
        for _ in 0..(0.05 * RATE) as usize {
            s.tick();
        }
        assert!((s.value - 0.632).abs() < 0.02);
        for _ in 0..(0.5 * RATE) as usize {
            s.tick();
        }
        assert!((s.value - 1.0).abs() < 1e-3);
    }

    #[test]
    fn soft_clip_is_bounded_odd_and_gentle_near_zero() {
        for x in [-10.0, -2.0, -0.5, 0.0, 0.5, 2.0, 10.0] {
            assert!(soft_clip(x).abs() <= 1.0);
            assert!((soft_clip(-x) + soft_clip(x)).abs() < 1e-6);
        }
        assert!((soft_clip(0.05) - 0.05).abs() < 1e-3);
    }

    #[test]
    fn soft_clip_never_passes_one_on_a_fine_sweep() {
        for i in -60_000..=60_000 {
            let x = i as f32 * 1e-4;
            assert!(soft_clip(x).abs() <= 1.0, "{x} -> {}", soft_clip(x));
        }
    }

    #[test]
    fn pan_keeps_equal_power() {
        let (l, r) = pan(0.0);
        assert!((l - r).abs() < 1e-6 && (l * l + r * r - 1.0).abs() < 1e-5);
        let (l, r) = pan(-1.0);
        assert!((l - 1.0).abs() < 1e-5 && r.abs() < 1e-5);
        let (l, r) = pan(1.0);
        assert!(l.abs() < 1e-5 && (r - 1.0).abs() < 1e-5);
        let (l, r) = pan(5.0);
        assert!(l.abs() < 1e-5 && (r - 1.0).abs() < 1e-5);
    }

    #[test]
    fn sine_and_advance_make_a_tone() {
        let xs = tone(1000.0, 4800);
        let crossings = xs.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((99..=101).contains(&crossings));
        assert!((rms(&xs) - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01);
        assert!((advance(0.9, 4800.0, RATE) - 0.0).abs() < 1e-5);
    }
}
