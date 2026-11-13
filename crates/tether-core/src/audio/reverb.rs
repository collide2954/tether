const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;
const TUNED_RATE: f32 = 44_100.0;

#[derive(Clone, Debug)]
struct Comb {
    buffer: Vec<f32>,
    index: usize,
    store: f32,
}

#[derive(Clone, Debug)]
struct Allpass {
    buffer: Vec<f32>,
    index: usize,
}

#[derive(Clone, Debug)]
pub struct Reverb {
    left: (Vec<Comb>, Vec<Allpass>),
    right: (Vec<Comb>, Vec<Allpass>),
    feedback: f32,
    damp: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)],
            index: 0,
            store: 0.0,
        }
    }

    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let output = self.buffer[self.index];
        self.store = output * (1.0 - damp) + self.store * damp;
        self.buffer[self.index] = input + self.store * feedback;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)],
            index: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buffered = self.buffer[self.index];
        self.buffer[self.index] = input + buffered * 0.5;
        self.index = (self.index + 1) % self.buffer.len();
        buffered - input
    }
}

fn bank(rate: f32, spread: usize) -> (Vec<Comb>, Vec<Allpass>) {
    let scale = rate / TUNED_RATE;
    let size = |n: usize| ((n + spread) as f32 * scale) as usize;
    (
        COMBS.iter().map(|&n| Comb::new(size(n))).collect(),
        ALLPASSES.iter().map(|&n| Allpass::new(size(n))).collect(),
    )
}

fn run(bank: &mut (Vec<Comb>, Vec<Allpass>), input: f32, feedback: f32, damp: f32) -> f32 {
    let mut sum = 0.0;
    for comb in &mut bank.0 {
        sum += comb.process(input, feedback, damp);
    }
    for allpass in &mut bank.1 {
        sum = allpass.process(sum);
    }
    sum
}

impl Reverb {
    pub fn new(rate: f32, room: f32, damp: f32) -> Self {
        Self {
            left: bank(rate, 0),
            right: bank(rate, SPREAD),
            feedback: room * 0.28 + 0.7,
            damp: damp * 0.4,
        }
    }

    pub fn process(&mut self, input: f32) -> (f32, f32) {
        let x = input * 0.015;
        (
            run(&mut self.left, x, self.feedback, self.damp),
            run(&mut self.right, x, self.feedback, self.damp),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::dsp::Noise;

    const RATE: f32 = 48_000.0;

    fn energy(xs: &[(f32, f32)]) -> f32 {
        xs.iter().map(|(l, r)| l * l + r * r).sum()
    }

    fn impulse(seconds: f32) -> Vec<(f32, f32)> {
        let mut verb = Reverb::new(RATE, 0.84, 0.3);
        (0..(seconds * RATE) as usize)
            .map(|i| verb.process(if i == 0 { 1.0 } else { 0.0 }))
            .collect()
    }

    #[test]
    fn reverb_starts_after_the_shortest_comb() {
        let ir = impulse(0.1);
        let first = (COMBS[0] as f32 * RATE / TUNED_RATE) as usize;
        assert!(energy(&ir[..first - 10]) < 1e-12);
        assert!(energy(&ir[first..first + 2000]) > 1e-6);
    }

    #[test]
    fn reverb_tail_decays() {
        let ir = impulse(4.0);
        let window = (0.5 * RATE) as usize;
        let start = energy(&ir[..window]);
        let late = energy(&ir[ir.len() - window..]);
        assert!(late < start * 1e-3, "{late} vs {start}");
    }

    #[test]
    fn reverb_is_stereo_and_stable() {
        let mut verb = Reverb::new(RATE, 0.84, 0.3);
        let mut noise = Noise::new(3);
        let out: Vec<(f32, f32)> = (0..(5.0 * RATE) as usize)
            .map(|_| verb.process(noise.sample() * 0.5))
            .collect();
        assert!(out.iter().all(|(l, r)| l.is_finite() && r.is_finite()));
        assert!(out.iter().all(|(l, r)| l.abs() < 4.0 && r.abs() < 4.0));
        let differ = out.iter().filter(|(l, r)| (l - r).abs() > 1e-4).count();
        assert!(differ > out.len() / 2);
    }

    #[test]
    fn reverb_scales_its_delays_with_the_rate() {
        let slow = Reverb::new(22_050.0, 0.84, 0.3);
        let fast = Reverb::new(96_000.0, 0.84, 0.3);
        assert!(slow.left.0[0].buffer.len() < fast.left.0[0].buffer.len());
        assert_eq!(
            fast.right.0[0].buffer.len(),
            ((COMBS[0] + SPREAD) as f32 * 96_000.0 / TUNED_RATE) as usize
        );
    }
}
