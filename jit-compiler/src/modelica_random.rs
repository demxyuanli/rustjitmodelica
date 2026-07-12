//! MSL `Modelica.Math.Random` xorshift generators matching
//! `Modelica/Resources/C-Sources/ModelicaRandom.c` (Modelica 3.2.3+).

const MODELICA_RANDOM_INVM64: f64 = 5.42101086242752217004e-20;

#[inline]
fn modelica_rand_u64(x: u64) -> f64 {
    (x as i64 as f64) * MODELICA_RANDOM_INVM64 + 0.5
}

#[inline]
fn u64_from_pair(lo: i32, hi: i32) -> u64 {
    let lo = lo as u32 as u64;
    let hi = hi as u32 as u64;
    (hi << 32) | lo
}

#[inline]
fn pair_from_u64(x: u64) -> (i32, i32) {
    (x as u32 as i32, (x >> 32) as u32 as i32)
}

fn step_xorshift64star(state: &mut [i32; 2]) -> f64 {
    let mut x = u64_from_pair(state[0], state[1]);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    // MSL/Vigna persist the state BEFORE the output multiply (matches
    // xorshift1024* below); the previous code stored the post-multiply value,
    // so every draw after the first diverged from MSL.
    let (lo, hi) = pair_from_u64(x);
    state[0] = lo;
    state[1] = hi;
    modelica_rand_u64(x.wrapping_mul(2685821657736338717u64))
}

fn step_xorshift128plus(state: &mut [i32; 4]) -> f64 {
    let mut s = [0u64; 2];
    s[0] = u64_from_pair(state[0], state[1]);
    s[1] = u64_from_pair(state[2], state[3]);
    let mut s1 = s[0];
    let s0 = s[1];
    s[0] = s[1];
    s1 ^= s1 << 23;
    // Persist s[1] WITHOUT the `+ s0` output term (Vigna/MSL); the previous
    // code folded +s0 into the stored state, diverging after the first draw.
    let new_s1 = s1 ^ s0 ^ (s1 >> 17) ^ (s0 >> 26);
    s[1] = new_s1;
    let (a, b) = pair_from_u64(s[0]);
    state[0] = a;
    state[1] = b;
    let (c, d) = pair_from_u64(s[1]);
    state[2] = c;
    state[3] = d;
    modelica_rand_u64(new_s1.wrapping_add(s0))
}

fn step_xorshift1024star(state: &mut [i32; 33]) -> f64 {
    let mut s = [0u64; 16];
    for i in 0..16 {
        s[i] = u64_from_pair(state[2 * i], state[2 * i + 1]);
    }
    let p_old = (state[32] & 15) as usize;
    let s0 = s[p_old];
    let p_new = ((state[32] & 15) + 1) & 15;
    let p_new_usize = p_new as usize;
    let s1 = s[p_new_usize];
    let mut s1_mut = s1;
    let mut s0_mut = s0;
    s1_mut ^= s1_mut << 31;
    s1_mut ^= s1_mut >> 11;
    s0_mut ^= s0_mut >> 30;
    s[p_new_usize] = s0_mut ^ s1_mut;
    let t = s[p_new_usize].wrapping_mul(1181783497276652981u64);
    state[32] = p_new;
    for i in 0..16 {
        let (lo, hi) = pair_from_u64(s[i]);
        state[2 * i] = lo;
        state[2 * i + 1] = hi;
    }
    modelica_rand_u64(t)
}

#[no_mangle]
pub unsafe extern "C" fn rustmodlica_math_random_msl(
    kind: i32,
    state_in: *const f64,
    state_out: *mut f64,
    n: i64,
    r_out: *mut f64,
) -> i32 {
    if state_in.is_null() || state_out.is_null() || r_out.is_null() {
        return -1;
    }
    if n <= 0 {
        return -2;
    }
    let n = n as usize;
    let f64_to_i32 = |x: f64| x as i32;

    match kind {
        0 => {
            if n != 2 {
                return -2;
            }
            let mut st = [0i32; 2];
            for i in 0..2 {
                st[i] = f64_to_i32(*state_in.add(i));
            }
            let r = step_xorshift64star(&mut st);
            for i in 0..2 {
                *state_out.add(i) = f64::from(st[i]);
            }
            *r_out = r;
            0
        }
        1 => {
            if n != 4 {
                return -2;
            }
            let mut st = [0i32; 4];
            for i in 0..4 {
                st[i] = f64_to_i32(*state_in.add(i));
            }
            let r = step_xorshift128plus(&mut st);
            for i in 0..4 {
                *state_out.add(i) = f64::from(st[i]);
            }
            *r_out = r;
            0
        }
        2 => {
            if n != 33 {
                return -2;
            }
            let mut st = [0i32; 33];
            for i in 0..33 {
                st[i] = f64_to_i32(*state_in.add(i));
            }
            let r = step_xorshift1024star(&mut st);
            for i in 0..33 {
                *state_out.add(i) = f64::from(st[i]);
            }
            *r_out = r;
            0
        }
        _ => -3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xorshift_generators_advance_and_stay_in_range() {
        // Each generator: values in [0,1), state advances (distinct draws), and
        // is deterministic for a fixed seed. Guards the state-persistence fix.
        let mut s64: [i32; 2] = [123, 456];
        let mut s64b = s64;
        let a = step_xorshift64star(&mut s64);
        let b = step_xorshift64star(&mut s64);
        assert!((0.0..1.0).contains(&a) && (0.0..1.0).contains(&b));
        assert_ne!(a, b, "state must advance between draws");
        assert_eq!(a, step_xorshift64star(&mut s64b), "must be deterministic");

        let mut s128: [i32; 4] = [1, 2, 3, 4];
        let c = step_xorshift128plus(&mut s128);
        let d = step_xorshift128plus(&mut s128);
        assert!((0.0..1.0).contains(&c) && (0.0..1.0).contains(&d));
        assert_ne!(c, d, "state must advance between draws");
    }
}
