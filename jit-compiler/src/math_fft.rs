//! Host-side real FFT used by JIT intrinsic `rustmodlica_math_real_fft`.
//! Numerical behavior follows Modelica.Math.FastFourierTransform.realFFT structure (mean, demean, RFFT, scaling).

use realfft::num_complex::Complex;
use realfft::RealFftPlanner;

/// Returns Modelica `info` code: 0 ok, 1 if nu odd, 3 if nfi invalid.
pub fn math_real_fft_ms_style(
    u: &[f64],
    nfi: usize,
    out_info: &mut f64,
    out_amp: &mut [f64],
    out_phase: &mut [f64],
    write_phases: bool,
) -> i32 {
    let nu = u.len();
    if nu % 2 != 0 {
        *out_info = 1.0;
        return 1;
    }
    let nf = nu / 2 + 1;
    if nfi == 0 || nfi > nf {
        *out_info = 3.0;
        return 3;
    }
    if out_amp.len() < nfi {
        *out_info = 3.0;
        return 3;
    }
    if write_phases && out_phase.len() < nfi {
        *out_info = 3.0;
        return 3;
    }

    let mean: f64 = u.iter().sum::<f64>() / nu as f64;
    let mut u2: Vec<f64> = u.iter().map(|x| x - mean).collect();

    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(nu);
    let mut spectrum: Vec<Complex<f64>> = r2c.make_output_vec();
    if r2c.process(&mut u2, &mut spectrum).is_err() {
        *out_info = 3.0;
        return 3;
    }

    let mut amps = vec![0.0_f64; nfi];
    let mut phases = vec![0.0_f64; nfi];

    for i in 0..nfi {
        if i == 0 {
            amps[0] = mean;
            phases[0] = 0.0;
            continue;
        }
        let c = spectrum[i];
        let mag = (c.re * c.re + c.im * c.im).sqrt() * 2.0 / nu as f64;
        let ph_deg = c.im.atan2(c.re).to_degrees();
        amps[i] = mag;
        phases[i] = ph_deg;
    }

    let mut mx = 0.0_f64;
    for a in &amps {
        mx = mx.max(a.abs());
    }
    let aeps = 0.0001 * mx;
    for i in 1..nfi {
        if amps[i] < aeps {
            phases[i] = 0.0;
        }
    }

    out_amp[..nfi].copy_from_slice(&amps[..nfi]);
    if write_phases {
        out_phase[..nfi].copy_from_slice(&phases[..nfi]);
    }
    *out_info = 0.0;
    0
}

#[no_mangle]
pub unsafe extern "C" fn rustmodlica_math_real_fft(
    u: *const f64,
    nu: i64,
    nfi: i64,
    info_out: *mut f64,
    amp_out: *mut f64,
    phase_out: *mut f64,
    write_phases: i32,
) -> i32 {
    if u.is_null() || info_out.is_null() || amp_out.is_null() {
        return -1;
    }
    if nu <= 0 || nfi <= 0 {
        return -1;
    }
    let nu = nu as usize;
    let nfi = nfi as usize;
    let write_phases = write_phases != 0;
    if write_phases && phase_out.is_null() {
        return -1;
    }
    let slice_u = std::slice::from_raw_parts(u, nu);
    let out_amp = std::slice::from_raw_parts_mut(amp_out, nfi);
    let mut phase_tmp = vec![0.0_f64; nfi];
    let out_phase: &mut [f64] = if write_phases {
        std::slice::from_raw_parts_mut(phase_out, nfi)
    } else {
        &mut phase_tmp[..]
    };
    let mut info = 0.0_f64;
    let code = math_real_fft_ms_style(
        slice_u,
        nfi,
        &mut info,
        out_amp,
        out_phase,
        write_phases,
    );
    *info_out = info;
    code
}

/// Writes a simple CSV (f_hz, amplitude[, phase]) for regression; not binary MAT.
#[no_mangle]
pub unsafe extern "C" fn rustmodlica_real_fft_write_to_file(
    t: f64,
    path: *const u8,
    f_max: f64,
    amp: *const f64,
    n_amp: i64,
    phases: *const f64,
    n_phase: i64,
) -> f64 {
    if path.is_null() || amp.is_null() || n_amp <= 0 {
        return 0.0;
    }
    let n_amp = n_amp as usize;
    let n_phase = n_phase.max(0) as usize;
    let use_phase = n_phase == n_amp && !phases.is_null();
    let slice_a = std::slice::from_raw_parts(amp, n_amp);
    let slice_p = if use_phase {
        std::slice::from_raw_parts(phases, n_phase)
    } else {
        &[] as &[f64]
    };
    let cstr = std::ffi::CStr::from_ptr(path as *const i8);
    let path_os = match cstr.to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };
    let mut lines = String::new();
    use std::fmt::Write;
    let _ = writeln!(&mut lines, "# t={:.9}", t);
    if use_phase {
        let _ = writeln!(&mut lines, "f_hz,amplitude,phase_deg");
    } else {
        let _ = writeln!(&mut lines, "f_hz,amplitude");
    }
    if n_amp == 1 {
        let _ = writeln!(&mut lines, "0.0,{:.9}", slice_a[0]);
    } else {
        for i in 0..n_amp {
            let f = f_max * (i as f64) / (n_amp.saturating_sub(1) as f64).max(1.0);
            if use_phase {
                let _ = writeln!(
                    &mut lines,
                    "{:.9},{:.9},{:.9}",
                    f,
                    slice_a[i],
                    slice_p[i]
                );
            } else {
                let _ = writeln!(&mut lines, "{:.9},{:.9}", f, slice_a[i]);
            }
        }
    }
    std::fs::write(path_os, lines).is_ok() as i32 as f64
}
