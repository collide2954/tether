pub fn encode_wav(rate: u32, channels: u16, samples: &[f32]) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for &s in samples {
        let v = if s.is_nan() { 0.0 } else { s.clamp(-1.0, 1.0) };
        out.extend_from_slice(&((v * 32767.0).round() as i16).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    fn u16_at(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
    }

    fn sample_at(bytes: &[u8], index: usize) -> i16 {
        i16::from_le_bytes(bytes[44 + 2 * index..46 + 2 * index].try_into().unwrap())
    }

    #[test]
    fn header_describes_sixteen_bit_pcm() {
        let wav = encode_wav(48_000, 2, &[0.0; 6]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32_at(&wav, 4), 36 + 12);
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u32_at(&wav, 16), 16);
        assert_eq!(u16_at(&wav, 20), 1);
        assert_eq!(u16_at(&wav, 22), 2);
        assert_eq!(u32_at(&wav, 24), 48_000);
        assert_eq!(u32_at(&wav, 28), 48_000 * 2 * 2);
        assert_eq!(u16_at(&wav, 32), 4);
        assert_eq!(u16_at(&wav, 34), 16);
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32_at(&wav, 40), 12);
        assert_eq!(wav.len(), 44 + 12);
    }

    #[test]
    fn samples_are_scaled_rounded_and_clamped() {
        let wav = encode_wav(44_100, 1, &[0.5, -0.5, 2.0, -2.0, f32::NAN, 0.0]);
        assert_eq!(sample_at(&wav, 0), 16384);
        assert_eq!(sample_at(&wav, 1), -16384);
        assert_eq!(sample_at(&wav, 2), 32767);
        assert_eq!(sample_at(&wav, 3), -32767);
        assert_eq!(sample_at(&wav, 4), 0);
        assert_eq!(sample_at(&wav, 5), 0);
    }
}
