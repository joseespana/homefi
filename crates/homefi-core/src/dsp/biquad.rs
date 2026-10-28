//! RBJ Audio-EQ-Cookbook biquad coefficients.

use std::f64::consts::PI;

use serde::Serialize;

use super::{EqBand, EqBandKind};

/// Rate used when a caller passes 0 Hz.
const FALLBACK_RATE: u32 = 48_000;
/// Highest corner frequency, as a fraction of the sample rate (below Nyquist).
const MAX_FREQ_FRACTION: f64 = 0.49;

/// Normalized second-order section (`a0` = 1):
/// `y = b0·x + b1·x[n-1] + b2·x[n-2] - a1·y[n-1] - a2·y[n-2]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Biquad {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

impl Biquad {
    /// Passes the signal unchanged.
    pub const IDENTITY: Self = Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 };

    /// A plain gain stage (`linear` = amplitude factor).
    pub fn gain(linear: f64) -> Self {
        Self { b0: linear, ..Self::IDENTITY }
    }

    /// Coefficients of `band` (expected in range, see [`EqBand::clamped`]) at
    /// `sample_rate` Hz. Corners above Nyquist are pulled just below it.
    pub fn design(band: &EqBand, sample_rate: u32) -> Self {
        let fs = rate(sample_rate);
        let f0 = (band.freq_hz as f64).clamp(1.0, fs * MAX_FREQ_FRACTION);
        let q = (band.q as f64).max(1e-3);
        let w0 = 2.0 * PI * f0 / fs;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let a = 10f64.powf(band.gain_db as f64 / 40.0);
        let sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let (b0, b1, b2, a0, a1, a2) = match band.kind {
            EqBandKind::Peak => {
                (1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a)
            }
            EqBandKind::LowShelf => (
                a * ((a + 1.0) - (a - 1.0) * cos + sqrt_a_alpha),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                a * ((a + 1.0) - (a - 1.0) * cos - sqrt_a_alpha),
                (a + 1.0) + (a - 1.0) * cos + sqrt_a_alpha,
                -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                (a + 1.0) + (a - 1.0) * cos - sqrt_a_alpha,
            ),
            EqBandKind::HighShelf => (
                a * ((a + 1.0) + (a - 1.0) * cos + sqrt_a_alpha),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                a * ((a + 1.0) + (a - 1.0) * cos - sqrt_a_alpha),
                (a + 1.0) - (a - 1.0) * cos + sqrt_a_alpha,
                2.0 * ((a - 1.0) - (a + 1.0) * cos),
                (a + 1.0) - (a - 1.0) * cos - sqrt_a_alpha,
            ),
            EqBandKind::LowPass => {
                ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
            EqBandKind::HighPass => {
                ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
        };
        let s = Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 };
        if s.is_finite() {
            s
        } else {
            Self::IDENTITY
        }
    }

    /// Gain in dB at `freq` Hz.
    pub fn magnitude_db(&self, freq: f64, sample_rate: u32) -> f64 {
        let fs = rate(sample_rate);
        let f = if freq.is_finite() { freq.clamp(0.0, fs / 2.0) } else { 0.0 };
        let w = 2.0 * PI * f / fs;
        let (s1, c1) = w.sin_cos();
        let (s2, c2) = (2.0 * w).sin_cos();
        // H(e^jw) = (b0 + b1 e^-jw + b2 e^-2jw) / (1 + a1 e^-jw + a2 e^-2jw)
        let (nr, ni) = (self.b0 + self.b1 * c1 + self.b2 * c2, -(self.b1 * s1 + self.b2 * s2));
        let (dr, di) = (1.0 + self.a1 * c1 + self.a2 * c2, -(self.a1 * s1 + self.a2 * s2));
        let num = nr * nr + ni * ni;
        let den = (dr * dr + di * di).max(f64::MIN_POSITIVE);
        let db = 10.0 * (num / den).max(1e-30).log10();
        if db.is_finite() {
            db
        } else {
            0.0
        }
    }

    pub(crate) fn lerp(a: &Self, b: &Self, t: f64) -> Self {
        let l = |x: f64, y: f64| x + (y - x) * t;
        Self { b0: l(a.b0, b.b0), b1: l(a.b1, b.b1), b2: l(a.b2, b.b2), a1: l(a.a1, b.a1), a2: l(a.a2, b.a2) }
    }

    fn is_finite(&self) -> bool {
        [self.b0, self.b1, self.b2, self.a1, self.a2].iter().all(|v| v.is_finite())
    }
}

fn rate(sample_rate: u32) -> f64 {
    if sample_rate == 0 {
        FALLBACK_RATE as f64
    } else {
        sample_rate as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poles_are_stable() {
        for kind in
            [EqBandKind::Peak, EqBandKind::LowShelf, EqBandKind::HighShelf, EqBandKind::LowPass, EqBandKind::HighPass]
        {
            for freq in [20.0, 1000.0, 20_000.0] {
                for q in [0.1, 0.707, 10.0] {
                    for gain in [-24.0, 0.0, 24.0] {
                        let b = Biquad::design(&EqBand::new(kind, freq, gain, q), 44_100);
                        // the stability triangle of a second-order section
                        assert!(b.a2.abs() < 1.0 && b.a1.abs() < 1.0 + b.a2, "{kind:?} {freq} {q} {gain}: {b:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn corners_above_nyquist_are_pulled_below() {
        let b = Biquad::design(&EqBand::new(EqBandKind::Peak, 20_000.0, 6.0, 1.0), 32_000);
        assert!(b.magnitude_db(15_680.0, 32_000) > 5.0);
        assert!(Biquad::design(&EqBand::new(EqBandKind::Peak, 1000.0, 6.0, 1.0), 0).is_finite());
    }
}
