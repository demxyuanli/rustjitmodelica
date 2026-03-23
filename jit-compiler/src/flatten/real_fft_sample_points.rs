//! MSL Modelica.Math.FastFourierTransform.realFFTsamplePoints (same algorithm as MSL 4.x).

/// Returns `ns` per MSL `algorithm` block, or `None` if assert conditions fail.
pub fn msl_real_fft_sample_points(f_max: f64, f_resolution: f64, f_max_factor: i64) -> Option<i64> {
    if f_resolution <= 0.0 {
        return None;
    }
    if f_max <= f_resolution {
        return None;
    }
    let ff = f_max_factor as f64;
    let ns1 = (2.0 * (f_max * ff / f_resolution).ceil()) as i64;
    let mut ns = if ns1 % 2 == 0 { ns1 } else { ns1 + 1 };
    loop {
        let ns1_loop = ns;
        let mut n = ns1_loop;
        while n % 2 == 0 {
            n /= 2;
        }
        while n % 3 == 0 {
            n /= 3;
        }
        while n % 5 == 0 {
            n /= 5;
        }
        if n <= 1 {
            break;
        }
        ns += 2;
    }
    Some(ns)
}

#[cfg(test)]
mod tests {
    use super::msl_real_fft_sample_points;

    #[test]
    fn realfft1_defaults_even_positive() {
        let ns = msl_real_fft_sample_points(4.0, 0.2, 5).expect("ns");
        assert!(ns > 0 && ns % 2 == 0);
    }
}
