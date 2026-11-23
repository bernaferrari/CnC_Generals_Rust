//! Bink1 inverse DCT using an owned real inverse FFT.
use realfft::{ComplexToReal, RealFftPlanner, num_complex::Complex32};
use std::sync::Arc;

pub(super) struct BinkDct {
    inverse: Arc<dyn ComplexToReal<f32>>,
    phases: Vec<Complex32>,
    spectrum: Vec<Complex32>,
    output: Vec<f32>,
    scratch: Vec<Complex32>,
}

impl BinkDct {
    pub(super) fn new(len: usize) -> Self {
        let inverse = RealFftPlanner::<f32>::new().plan_fft_inverse(2 * len);
        let phases = (0..len)
            .map(|bin| {
                let angle = std::f64::consts::PI * bin as f64 / (2 * len) as f64;
                let (sin, cos) = angle.sin_cos();
                Complex32::new(cos as f32, sin as f32)
            })
            .collect();
        Self {
            phases,
            spectrum: inverse.make_input_vec(),
            output: inverse.make_output_vec(),
            scratch: inverse.make_scratch_vec(),
            inverse,
        }
    }

    pub(super) fn run(&mut self, coefficients: &mut [f32]) {
        let len = coefficients.len();
        // A 2N real inverse FFT supplies both conjugate terms for each
        // positive bin. The phase shifts the cosine by half a sample.
        self.spectrum[0] = Complex32::new(coefficients[0], 0.);
        for (bin, (&coefficient, &phase)) in
            coefficients.iter().zip(&self.phases).enumerate().skip(1)
        {
            self.spectrum[bin] = phase * (coefficient * 0.5);
        }
        self.spectrum[len] = Complex32::new(0., 0.);
        self.inverse
            .process_with_scratch(&mut self.spectrum, &mut self.output, &mut self.scratch)
            .expect("Bink DCT owns matching FFT buffers");
        // Bink's inverse DCT and DC correction give every cosine term 2/N.
        let scale = 2. / len as f32;
        for (sample, &unscaled) in coefficients.iter_mut().zip(&self.output) {
            *sample = unscaled * scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_dct_matches_bink_cosine_formula_for_each_transform_size() {
        for len in [512, 1024, 2048] {
            let mut transform = BinkDct::new(len);
            // Reuse the same buffers on a second spectrum without DC.
            for bins in [
                [(0, 0.125), (1, 0.5), (len / 4, -0.25), (len - 1, 1.0)],
                [(0, 0.0), (2, 0.75), (len / 2, 0.25), (len - 2, -0.375)],
            ] {
                let mut coefficients = vec![0.; len];
                for (bin, amplitude) in bins {
                    coefficients[bin] = amplitude;
                }
                transform.run(&mut coefficients);

                for (index, &sample) in coefficients.iter().enumerate() {
                    let expected = bins
                        .iter()
                        .map(|&(bin, amplitude)| {
                            let angle = std::f64::consts::PI * bin as f64 * (index as f64 + 0.5)
                                / len as f64;
                            f64::from(amplitude) * angle.cos()
                        })
                        .sum::<f64>()
                        * (2. / len as f64);
                    assert!(
                        (f64::from(sample) - expected).abs() < 1e-8,
                        "{len} at {index}"
                    );
                }
            }
        }
    }
}
