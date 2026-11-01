//! Decode retail IMA ADPCM WAV files into interleaved samples.
use std::{io::Cursor, sync::Arc};
use symphonia::core::{
    codecs::audio::{AudioDecoderOptions, CODEC_ID_NULL_AUDIO},
    formats::{FormatOptions, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
};

struct AudioBytes(Arc<Vec<u8>>);

impl AsRef<[u8]> for AudioBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

fn wave_chunk<'a>(bytes: &'a [u8], name: &[u8; 4]) -> Option<&'a [u8]> {
    if bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return None;
    }
    let mut pos = 12usize;
    while let Some(header) = bytes.get(pos..pos.checked_add(8)?) {
        let len = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
        let start = pos.checked_add(8)?;
        let end = start.checked_add(len)?;
        let chunk = bytes.get(start..end)?;
        if &header[..4] == name {
            return Some(chunk);
        }
        pos = end.checked_add(len & 1)?;
    }
    None
}

/// Whether a WAV contains IMA ADPCM samples.
pub fn is_ima_wave(bytes: &[u8]) -> bool {
    wave_chunk(bytes, b"fmt ").and_then(|chunk| chunk.get(..2)) == Some(&[0x11, 0])
}

/// Device-independent PCM data for a single playback request.
#[derive(Debug)]
pub struct DecodedImaWave {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

/// Decode the complete WAV and discard padding beyond its declared frame count.
pub fn decode(bytes: Arc<Vec<u8>>) -> Result<DecodedImaWave, String> {
    let declared_frames = wave_chunk(&bytes, b"fact")
        .and_then(|chunk| chunk.get(..4))
        .map(|raw| u32::from_le_bytes(raw.try_into().unwrap()) as u64);
    let stream =
        MediaSourceStream::new(Box::new(Cursor::new(AudioBytes(bytes))), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("wav");
    let mut reader = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| e.to_string())?;
    let track = reader
        .tracks()
        .iter()
        .find(|track| {
            track
                .codec_params
                .as_ref()
                .and_then(|p| p.audio())
                .is_some_and(|p| p.codec != CODEC_ID_NULL_AUDIO)
        })
        .ok_or("no audio track")?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or("missing audio parameters")?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|e| e.to_string())?;
    let mut samples = Vec::new();
    let mut packet_samples = Vec::new();
    let mut format = None;
    loop {
        let packet = match reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet).map_err(|e| e.to_string())?;
        let spec = decoded.spec();
        let current = (spec.channels().count() as u16, spec.rate());
        if format.is_some_and(|existing| existing != current) {
            return Err("audio format changes within stream".into());
        }
        format = Some(current);
        decoded.copy_to_vec_interleaved::<f32>(&mut packet_samples);
        samples.extend_from_slice(&packet_samples);
    }
    let (channels, rate) = format.ok_or("no audio samples")?;
    if let Some(frames) = declared_frames.filter(|frames| *frames > 0) {
        let count = usize::try_from(frames)
            .ok()
            .and_then(|n| n.checked_mul(channels as usize))
            .ok_or("sample count overflow")?;
        if count > samples.len() {
            return Err("audio ends before declared frame count".into());
        }
        samples.truncate(count);
    }
    if samples.is_empty() {
        return Err("no audio samples".into());
    }
    if channels == 0 || rate == 0 {
        return Err("invalid audio channel count or sample rate".into());
    }
    Ok(DecodedImaWave {
        channels,
        sample_rate: rate,
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_multiple_ima_blocks_with_expected_samples() {
        // Generated from an 8 kHz mono tone by ffmpeg's IMA WAV encoder.
        // The sample values were independently decoded by ffmpeg.
        let bytes = include_bytes!("fixtures/ima_mono.wav").to_vec();
        assert!(is_ima_wave(&bytes));
        let source = decode(Arc::new(bytes)).unwrap();
        assert_eq!(source.channels, 1);
        assert_eq!(source.sample_rate, 8000);
        let samples = source.samples;
        // The fact chunk excludes the padding in the final compressed block.
        assert_eq!(samples.len(), 160);
        let expected = [
            0, 11, 41, 104, 240, 533, 1164, 2521, 2327, 1094, -27, -1338, -2571, -3372, -3808,
        ];
        for (actual, expected) in samples.iter().zip(expected) {
            assert!(
                (*actual - expected as f32 / 32768.0).abs() < 0.002,
                "{actual} != {expected}"
            );
        }
    }

    #[test]
    fn decodes_interleaved_stereo_channels() {
        let bytes = include_bytes!("fixtures/ima_stereo.wav").to_vec();
        let source = decode(Arc::new(bytes)).unwrap();
        assert_eq!(source.channels, 2);
        assert_eq!(source.sample_rate, 8000);
        let samples = source.samples;
        assert_eq!(samples.len(), 320);
        let expected = [
            0, 0, 11, 11, 41, 41, 104, 104, 240, 240, 533, 533, 1164, -98, 2521, -1455,
        ];
        for (actual, expected) in samples.iter().zip(expected) {
            assert!((*actual - expected as f32 / 32768.0).abs() < 0.002);
        }
    }

    #[test]
    fn rejects_other_or_truncated_formats() {
        assert!(!is_ima_wave(b"RIFF\0\0\0\0WAVEfmt \x14\0\0\0\x11"));
        assert!(!is_ima_wave(b"not a WAV"));
        let mut pcm = include_bytes!("fixtures/ima_mono.wav").to_vec();
        pcm[20] = 1;
        assert!(!is_ima_wave(&pcm));
    }
}
