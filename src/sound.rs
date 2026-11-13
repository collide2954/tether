use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use tether_core::audio::synth::{Command, Synth};

const QUEUE: usize = 1024;
const RESERVE: usize = 16_384;

pub fn spread<T: FromSample<f32>>(stereo: &[f32], out: &mut [T], channels: usize) {
    let channels = channels.max(1);
    for (frame, pair) in out
        .chunks_exact_mut(channels)
        .zip(stereo.as_chunks::<2>().0)
    {
        if channels == 1 {
            frame[0] = T::from_sample_(0.5 * (pair[0] + pair[1]));
            continue;
        }
        frame[0] = T::from_sample_(pair[0]);
        frame[1] = T::from_sample_(pair[1]);
        for rest in &mut frame[2..] {
            *rest = T::from_sample_(0.0);
        }
    }
}

pub struct Sound {
    sender: SyncSender<Command>,
    _stream: cpal::Stream,
}

impl Sound {
    pub fn start() -> Option<Self> {
        let device = cpal::default_host().default_output_device()?;
        let supported = device.default_output_config().ok()?;
        let config = supported.config();
        let (sender, receiver) = sync_channel(QUEUE);
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, config, receiver),
            SampleFormat::F64 => build::<f64>(&device, config, receiver),
            SampleFormat::I16 => build::<i16>(&device, config, receiver),
            SampleFormat::I32 => build::<i32>(&device, config, receiver),
            SampleFormat::U16 => build::<u16>(&device, config, receiver),
            _ => None,
        }?;
        stream.play().ok()?;
        Some(Self {
            sender,
            _stream: stream,
        })
    }

    pub fn send(&self, commands: impl IntoIterator<Item = Command>) {
        for command in commands {
            let _ = self.sender.try_send(command);
        }
    }
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    receiver: Receiver<Command>,
) -> Option<cpal::Stream>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
{
    let channels = usize::from(config.channels.max(1));
    let mut synth = Synth::new(config.sample_rate as f32);
    let mut stereo: Vec<f32> = Vec::with_capacity(RESERVE);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                while let Ok(command) = receiver.try_recv() {
                    synth.apply(command);
                }
                stereo.resize(data.len() / channels * 2, 0.0);
                synth.render(&mut stereo);
                spread(&stereo, data, channels);
            },
            |error| eprintln!("tether: audio: {error}"),
            None,
        )
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_passes_through_to_two_channels() {
        let mut out = [0.0f32; 4];
        spread(&[0.1, 0.2, 0.3, 0.4], &mut out, 2);
        assert_eq!(out, [0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn mono_hears_the_average() {
        let mut out = [0.0f32; 2];
        spread(&[0.2, 0.4, -0.5, 0.5], &mut out, 1);
        assert!((out[0] - 0.3).abs() < 1e-6 && out[1].abs() < 1e-6);
    }

    #[test]
    fn extra_channels_stay_silent() {
        let mut out = [9.0f32; 12];
        spread(&[0.1, 0.2, 0.3, 0.4], &mut out, 6);
        assert_eq!(
            out,
            [0.1, 0.2, 0.0, 0.0, 0.0, 0.0, 0.3, 0.4, 0.0, 0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn integer_devices_get_scaled_samples() {
        let mut out = [0i16; 2];
        spread(&[0.5, -0.5], &mut out, 2);
        assert!((16_000..=16_400).contains(&out[0]));
        assert!((-16_400..=-16_000).contains(&out[1]));
    }
}
