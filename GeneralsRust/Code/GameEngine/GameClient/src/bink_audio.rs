//! Bink audio bitstream parser and DCT/RDFT decoder.
//!
use std::f32::consts::PI;

mod dct;
use dct::BinkDct;

const MAX_BANDS: usize = 25;
const CRITICAL_FREQS: [u16; 25] = [
    100, 200, 300, 400, 510, 630, 770, 920, 1080, 1270, 1480, 1720, 2000, 2320, 2700, 3150, 3700,
    4400, 5300, 6400, 7700, 9500, 12000, 15500, 24500,
];
const RUN_TAB: [usize; 16] = [2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16, 32, 64];

const BINK_AUD_16BITS: u16 = 0x4000;
const BINK_AUD_STEREO: u16 = 0x2000;
const BINK_AUD_USEDCT: u16 = 0x1000;

#[derive(Debug, Clone)]
pub struct BinkAudioTrack {
    pub sample_rate: u32,
    pub channels: u8,
    pub use_dct: bool,
    pub version_b: bool,
    pub id: u32,
}

#[derive(Debug, Clone)]
pub struct BinkAudioLayout {
    pub tracks: Vec<BinkAudioTrack>,
    pub frame_table_offset: usize,
}

pub fn has_bink_audio_bitstream_parser() -> bool {
    true
}

/// C++ `BinkVideoPlayer::createStream` volume mapping.
pub fn apply_speech_slider_volume(speech_volume: f32) -> f32 {
    let speech = speech_volume.clamp(0.0, 1.0);
    let percent = (speech * 0.8 * 100.0) as i32 + 1;
    let bink_units = 32768 * percent / 100;
    bink_units as f32 / 32768.0
}

pub fn parse_audio_layout(
    bytes: &[u8],
    audio_track_count: u32,
    version_b: bool,
) -> BinkAudioLayout {
    let mut offset = 44usize;
    let count = audio_track_count as usize;
    // Per-track max decoded size (u32 each).
    offset = offset
        .saturating_add(count.saturating_mul(4))
        .min(bytes.len());

    let mut tracks = Vec::with_capacity(count);
    for _ in 0..count {
        if offset + 8 > bytes.len() {
            break;
        }
        let sample_rate = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as u32;
        let flags = u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]);
        let id = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        offset += 8;
        let channels = if flags & BINK_AUD_STEREO != 0 { 2 } else { 1 };
        let use_dct = flags & BINK_AUD_USEDCT != 0 || flags & BINK_AUD_16BITS == 0;
        let _ = flags;
        tracks.push(BinkAudioTrack {
            sample_rate: sample_rate.max(1),
            channels,
            use_dct,
            version_b,
            id,
        });
    }

    BinkAudioLayout {
        tracks,
        frame_table_offset: offset,
    }
}

pub fn split_frame_audio_and_video<'a>(
    packet: &'a [u8],
    track_count: usize,
) -> (Vec<&'a [u8]>, &'a [u8]) {
    let mut cursor = 0usize;
    let mut audio = Vec::with_capacity(track_count);
    for _ in 0..track_count {
        if cursor + 4 > packet.len() {
            break;
        }
        let size = u32::from_le_bytes([
            packet[cursor],
            packet[cursor + 1],
            packet[cursor + 2],
            packet[cursor + 3],
        ]) as usize;
        cursor += 4;
        let end = (cursor + size).min(packet.len());
        audio.push(&packet[cursor..end]);
        cursor = end;
    }
    (audio, packet.get(cursor..).unwrap_or(&[]))
}

pub struct BinkAudioDecoder {
    dct: Option<BinkDct>,
    track: BinkAudioTrack,
    len: usize,
    duration: usize,
    quants: [f32; 96],
    bands: [usize; MAX_BANDS + 1],
    num_bands: usize,
    coeffs: [f32; 4096],
    delay: [[f32; 256]; 2],
    first_frm: bool,
    scale: f32,
}

impl BinkAudioDecoder {
    pub fn new(track: BinkAudioTrack) -> Self {
        let srate = track.sample_rate;
        let mut frame_bits = if srate < 22050 {
            9
        } else if srate < 44100 {
            10
        } else {
            11
        };
        if !track.use_dct && !track.version_b && track.channels > 1 {
            frame_bits += 1;
        }
        let len = 1usize << frame_bits;
        let mut duration = len - (len >> 4);
        let single = !track.use_dct && track.channels == 2;
        if single {
            duration >>= 1;
        }
        let dct = track.use_dct && !track.version_b;
        let scale = if dct {
            (len as f32).sqrt() / 32768.0
        } else if track.use_dct {
            (2.0 / (len as f32)).sqrt() / 1024.0
        } else {
            1.0 / (32768.0 * (len as f32).sqrt())
        };
        let mut num_bands = 0;
        let mut bands = [0usize; MAX_BANDS + 1];
        if dct {
            init_dct_bands(srate.div_ceil(2) as usize, len, &mut num_bands, &mut bands);
        } else {
            let s_srate = if single { srate } else { srate >> 1 } as usize;
            init_bands(s_srate, len, &mut num_bands, &mut bands);
        }
        Self {
            dct: dct.then(|| BinkDct::new(len)),
            track,
            len,
            duration,
            quants: quant_table(),
            bands,
            num_bands,
            coeffs: [0.0; 4096],
            delay: [[0.0; 256]; 2],
            first_frm: true,
            scale,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.track.sample_rate
    }

    pub fn channels(&self) -> u8 {
        self.track.channels
    }

    pub fn reset_prediction(&mut self) {
        self.delay.fill([0.0; 256]);
        self.first_frm = true;
    }

    pub fn decode_packet(&mut self, packet: &[u8]) -> Vec<f32> {
        if packet.len() < 4 {
            return Vec::new();
        }
        let mut br = BitReader::new(packet);
        // The packet prefix counts decoded 16-bit PCM bytes across all channels.
        let pcm_bytes = br.read(32).unwrap_or(0) as usize;
        let ch = usize::from(self.track.channels);
        let bytes_per_sample = std::mem::size_of::<i16>();
        if pcm_bytes == 0 || ch == 0 || !pcm_bytes.is_multiple_of(ch * bytes_per_sample) {
            return Vec::new();
        }
        let nsamples = pcm_bytes / bytes_per_sample;
        let samples_per_subframe = self.duration.max(1) * ch;
        // Even an otherwise empty block encodes two initial coefficients.
        // Reject impossible sizes before allocating from the packet header.
        let min_coefficient_bits = if self.track.version_b { 64 } else { 58 };
        if nsamples.div_ceil(samples_per_subframe)
            > (packet.len() - 4).saturating_mul(8) / min_coefficient_bits
        {
            return Vec::new();
        }
        let mut planar = vec![0.0f32; nsamples.max(self.duration * ch)];
        let mut off0 = 0usize;
        let mut off1 = if ch > 1 { nsamples / ch } else { 0 };
        let num_subframes = (nsamples / samples_per_subframe).max(1);
        for _ in 0..num_subframes {
            if self.track.use_dct {
                let _ = br.skip(2);
            }
            if self.decode_block(&mut br).is_err() {
                break;
            }
            self.output(&mut planar, off0, off1, 0);
            if ch > 1 && self.track.use_dct {
                if self.decode_block(&mut br).is_err() {
                    break;
                }
                self.output(&mut planar, off0, off1, 1);
            }
            self.first_frm = false;
            let left = br.left() & 31;
            if left != 0 {
                let _ = br.skip(left as u32);
            }
            off0 += self.duration;
            off1 += self.duration;
        }
        interleave_planar(&planar, nsamples, ch)
    }

    fn decode_block(&mut self, br: &mut BitReader) -> Result<(), ()> {
        self.coeffs = [0.0; 4096];
        if self.track.version_b {
            self.coeffs[0] = br.read_f32_bits()? * self.scale;
            self.coeffs[1] = br.read_f32_bits()? * self.scale;
        } else if self.dct.is_some() {
            self.coeffs[0] = read_dct_float(br)? * self.scale;
            self.coeffs[1] = read_dct_float(br)? * self.scale;
        } else {
            self.coeffs[0] = read_bink_float(br)? * self.scale;
            self.coeffs[1] = read_bink_float(br)? * self.scale;
        }
        let mut quants = [0.0f32; MAX_BANDS];
        for quant in quants[..self.num_bands].iter_mut() {
            let idx = br.read(8)? as usize;
            *quant = self.quants[idx.min(self.quants.len() - 1)] * self.scale;
        }
        let mut idx = 2;
        let mut band_idx = 0;
        while idx < self.len {
            let width = if self.track.version_b {
                16
            } else if br.read_bool()? {
                let run = br.read(4)? as usize;
                RUN_TAB[run.min(RUN_TAB.len() - 1)] * 8
            } else {
                8
            };
            let end = (idx + width).min(self.len);
            let bits = br.read(4)? as u8;
            if bits != 0 {
                for i in idx..end {
                    while band_idx < self.bands.len() && self.bands[band_idx] <= i {
                        band_idx += 1;
                    }
                    let q = quants[band_idx.saturating_sub(1).min(quants.len() - 1)];
                    let coeff = br.read(bits)?;
                    if coeff != 0 {
                        self.coeffs[i] = if br.read_bool()? {
                            -(coeff as f32) * q
                        } else {
                            (coeff as f32) * q
                        };
                    }
                }
            }
            idx = end;
        }
        Ok(())
    }

    fn output(&mut self, dst: &mut [f32], off0: usize, off1: usize, chno: usize) {
        if let Some(dct) = &mut self.dct {
            dct.run(&mut self.coeffs[..self.len]);
        } else if self.track.use_dct {
            dct_iii_inplace(&mut self.coeffs[..self.len]);
        } else {
            rdft_inplace(&mut self.coeffs[..self.len]);
            for i in (0..self.len.saturating_sub(1)).step_by(2) {
                self.coeffs.swap(i, i + 1);
            }
        }
        if self.track.use_dct || self.track.channels == 1 {
            let overlap_len = if self.first_frm { 0 } else { self.len >> 4 };
            let base = if chno == 0 { off0 } else { off1 };
            if base >= dst.len() {
                return;
            }
            let out = &mut dst[base..];
            if self.dct.is_some() {
                let channels = usize::from(self.track.channels);
                let weight = overlap_len * channels;
                for (i, sample) in out.iter_mut().take(overlap_len).enumerate() {
                    let incoming = i * channels + chno;
                    *sample = (self.delay[chno][i] * (weight - incoming) as f32
                        + self.coeffs[i] * incoming as f32)
                        / weight as f32;
                }
            } else {
                overlap(&self.delay[chno], &self.coeffs, out, overlap_len, 1);
            }
            let copy_end = self.duration.min(out.len());
            if overlap_len < copy_end {
                out[overlap_len..copy_end].copy_from_slice(&self.coeffs[overlap_len..copy_end]);
            }
            for i in 0..(self.len >> 4).min(256) {
                let src = self.duration + i;
                self.delay[chno][i] = if src < self.coeffs.len() {
                    self.coeffs[src]
                } else {
                    0.0
                };
            }
        } else {
            let overlap_len = if self.first_frm { 0 } else { self.len >> 8 };
            overlap(
                &self.delay[0],
                &self.coeffs,
                &mut dst[off0..],
                overlap_len,
                2,
            );
            if off1 < dst.len() {
                overlap(
                    &self.delay[1],
                    &self.coeffs[1..],
                    &mut dst[off1..],
                    overlap_len,
                    2,
                );
            }
            for i in overlap_len..self.duration {
                if off0 + i < dst.len() {
                    dst[off0 + i] = self.coeffs[i * 2];
                }
                if off1 + i < dst.len() {
                    dst[off1 + i] = self.coeffs[i * 2 + 1];
                }
            }
            for i in 0..(self.len >> 8).min(256) {
                let src = self.duration * 2 + i * 2;
                self.delay[0][i] = self.coeffs.get(src).copied().unwrap_or(0.0);
                self.delay[1][i] = self.coeffs.get(src + 1).copied().unwrap_or(0.0);
            }
        }
    }
}

fn interleave_planar(planar: &[f32], nsamples: usize, channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return planar[..nsamples.min(planar.len())].to_vec();
    }
    let per_ch = nsamples / channels;
    let mut out = vec![0.0f32; per_ch * channels];
    for i in 0..per_ch {
        for ch in 0..channels {
            let src = ch * per_ch + i;
            out[i * channels + ch] = planar.get(src).copied().unwrap_or(0.0);
        }
    }
    out
}

fn overlap(a: &[f32], b: &[f32], dst: &mut [f32], len: usize, step: usize) {
    let n = len.min(dst.len()).min(a.len());
    for i in 0..n {
        let b_i = i * step;
        let bv = if b_i < b.len() { b[b_i] } else { 0.0 };
        dst[i] = (a[i] * ((len - i) as f32) + bv * (i as f32)) / (len as f32).max(1.0);
    }
}

/// Bink's DCT stores an explicit mantissa and a separate sign.
fn read_dct_float(br: &mut BitReader) -> Result<f32, ()> {
    let exponent = br.read(5)? as i32;
    let mantissa = br.read(23)?;
    let negative = br.read_bool()?;
    let magnitude = mantissa as f32 * 2.0_f32.powi(exponent - 23);
    Ok(if negative { -magnitude } else { magnitude })
}

fn read_bink_float(br: &mut BitReader) -> Result<f32, ()> {
    let exp = br.read(5)? as u8;
    let mant = br.read(23)?;
    let sign = br.read(1)?;
    let nexp = exp.wrapping_add(0x7E) as u32;
    let nmant = (mant << 1) & ((1 << 23) - 1);
    Ok(f32::from_bits((sign << 31) | (nexp << 23) | nmant))
}

fn quant_table() -> [f32; 96] {
    let mut q = [0.0f32; 96];
    for (i, slot) in q.iter_mut().enumerate() {
        *slot = (i as f32 * 0.152_891_65).exp();
    }
    q
}

/// Bink DCT quantizer boundaries are even coefficient indices.
fn init_dct_bands(
    sample_rate_half: usize,
    frame_len: usize,
    num_bands: &mut usize,
    bands: &mut [usize],
) {
    *num_bands = 1 + CRITICAL_FREQS[..MAX_BANDS - 1]
        .iter()
        .take_while(|&&freq| sample_rate_half > usize::from(freq))
        .count();
    bands[0] = 2;
    for (band, &freq) in bands[1..*num_bands].iter_mut().zip(&CRITICAL_FREQS) {
        *band = (usize::from(freq) * frame_len / sample_rate_half.max(1)) & !1;
    }
    bands[*num_bands] = frame_len;
}

fn init_bands(sample_rate: usize, frame_len: usize, num_bands: &mut usize, bands: &mut [usize]) {
    *num_bands = 1;
    bands[0] = 2;
    for freq in CRITICAL_FREQS {
        let bin = ((freq as usize) * frame_len / sample_rate.max(1)).max(2);
        if bin >= frame_len {
            break;
        }
        if bin > bands[*num_bands - 1] {
            bands[*num_bands] = bin;
            *num_bands += 1;
        }
    }
    bands[*num_bands] = frame_len;
}

fn dct_iii_inplace(buf: &mut [f32]) {
    let n = buf.len();
    if n < 2 {
        return;
    }
    let mut tmp = vec![0.0f32; n];
    for i in 0..n {
        let mut sum = 0.0;
        for (k, &sample) in buf.iter().enumerate() {
            sum += sample * ((PI * k as f32 * (2.0 * i as f32 + 1.0)) / (2.0 * n as f32)).cos();
        }
        tmp[i] = sum;
    }
    buf.copy_from_slice(&tmp);
}

fn rdft_inplace(buf: &mut [f32]) {
    let n = buf.len();
    if n < 2 {
        return;
    }
    let mut real = buf.to_vec();
    let imag = vec![0.0f32; n];
    let mut out_r = vec![0.0f32; n];
    let mut out_i = vec![0.0f32; n];
    for i in 0..n {
        let mut sr = 0.0;
        let mut si = 0.0;
        for k in 0..n {
            let ang = 2.0 * PI * (i * k) as f32 / n as f32;
            sr += real[k] * ang.cos() + imag[k] * ang.sin();
            si += imag[k] * ang.cos() - real[k] * ang.sin();
        }
        out_r[i] = sr;
        out_i[i] = si;
    }
    for i in 0..n {
        buf[i] = if i % 2 == 0 {
            out_r[i / 2]
        } else {
            out_i[i / 2]
        };
    }
    let _ = real;
}

struct BitReader<'a> {
    data: &'a [u8],
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }

    fn left(&self) -> usize {
        self.data
            .len()
            .saturating_mul(8)
            .saturating_sub(self.bit_pos)
    }

    fn read(&mut self, bits: u8) -> Result<u32, ()> {
        if bits == 0 {
            return Ok(0);
        }
        let bits = bits as usize;
        if self.left() < bits {
            return Err(());
        }
        let mut value = 0u32;
        for i in 0..bits {
            let byte = self.data[self.bit_pos / 8];
            let bit = (byte >> (self.bit_pos % 8)) & 1;
            value |= (bit as u32) << i;
            self.bit_pos += 1;
        }
        Ok(value)
    }

    fn read_bool(&mut self) -> Result<bool, ()> {
        Ok(self.read(1)? != 0)
    }

    fn skip(&mut self, bits: u32) -> Result<(), ()> {
        let bits = bits as usize;
        if self.left() < bits {
            return Err(());
        }
        self.bit_pos += bits;
        Ok(())
    }

    fn read_f32_bits(&mut self) -> Result<f32, ()> {
        let bits = self.read(32)?;
        Ok(f32::from_bits(bits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_is_present() {
        crate::test_support::ensure_ambient_host();
        assert!(has_bink_audio_bitstream_parser());
    }

    #[test]
    fn speech_slider_never_hits_zero() {
        crate::test_support::ensure_ambient_host();
        assert_eq!(apply_speech_slider_volume(0.0), 327.0 / 32768.0);
        assert_eq!(apply_speech_slider_volume(0.552), 14745.0 / 32768.0);
        assert_eq!(apply_speech_slider_volume(1.0), 26542.0 / 32768.0);
    }

    #[test]
    fn layout_skips_audio_headers_before_frame_table() {
        crate::test_support::ensure_ambient_host();
        let mut bytes = vec![0u8; 44 + 4 + 8 + 8];
        bytes[44..48].copy_from_slice(&128u32.to_le_bytes());
        bytes[48..50].copy_from_slice(&22050u16.to_le_bytes());
        bytes[50..52].copy_from_slice(&BINK_AUD_USEDCT.to_le_bytes());
        let layout = parse_audio_layout(&bytes, 1, false);
        assert_eq!(layout.tracks.len(), 1);
        assert_eq!(layout.tracks[0].sample_rate, 22050);
        assert_eq!(layout.frame_table_offset, 56);
    }
}
