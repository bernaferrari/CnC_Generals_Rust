//! Exercise the same decoder used by native playback independently of a device.
#[path = "../src/common/audio/retail_adpcm.rs"]
mod retail_adpcm;

#[test]
#[ignore = "requires local retail audio and FFmpeg reference paths"]
fn retail_file_matches_reference() {
    use std::{path::PathBuf, sync::Arc};
    let wav = PathBuf::from(std::env::var_os("RETAIL_IMA_WAV").unwrap());
    let reference = PathBuf::from(std::env::var_os("RETAIL_IMA_REFERENCE_F32LE").unwrap());
    let decoded: Vec<f32> = retail_adpcm::decode(Arc::new(std::fs::read(wav).unwrap()))
        .unwrap()
        .collect();
    let raw = std::fs::read(reference).unwrap();
    let expected: Vec<f32> = raw
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect();
    // FFmpeg emits the final padded block; the WAV fact chunk declares
    // 42,735 valid frames for this retail file.
    assert_eq!(decoded.len(), 42_735);
    assert!(expected.len() >= decoded.len());
    let max = decoded
        .iter()
        .zip(&expected)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    let mean = decoded
        .iter()
        .zip(&expected)
        .map(|(a, b)| (a - b).abs() as f64)
        .sum::<f64>()
        / decoded.len() as f64;
    println!(
        "samples={}, max_error={max}, mean_error={mean}",
        decoded.len()
    );
    assert!(max < 0.02 && mean < 0.002);
}
