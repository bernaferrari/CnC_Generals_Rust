// Shared binary format fixtures; both live consumers load these bytes.
use glam::{Mat4, Quat, Vec3};

pub fn chunk(kind: u32, payload: &[u8], container: bool) -> Vec<u8> {
    let mut out = kind.to_le_bytes().to_vec();
    out.extend_from_slice(&((payload.len() as u32) | if container { 0x8000_0000 } else { 0 }).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn animation(flavor: u16, frames: u32, channels: &[Vec<u8>]) -> Vec<u8> {
    let mut header = 0x0004_0000u32.to_le_bytes().to_vec();
    for name in [b"CLIP".as_slice(), b"TEST".as_slice()] {
        let mut field = [0; 16];
        field[..name.len()].copy_from_slice(name);
        header.extend_from_slice(&field);
    }
    header.extend_from_slice(&frames.to_le_bytes());
    header.extend_from_slice(&30u16.to_le_bytes());
    header.extend_from_slice(&flavor.to_le_bytes());
    let mut payload = chunk(0x281, &header, false);
    for channel in channels { payload.extend_from_slice(channel); }
    chunk(0x280, &payload, true)
}

pub fn timecoded(kind: u8, len: u8, keys: &[(u32, &[f32])]) -> Vec<u8> {
    let mut payload = (keys.len() as u32).to_le_bytes().to_vec();
    payload.extend_from_slice(&1u16.to_le_bytes());
    payload.extend_from_slice(&[len, kind]);
    for (time, values) in keys {
        payload.extend_from_slice(&time.to_le_bytes());
        for value in *values { payload.extend_from_slice(&value.to_le_bytes()); }
    }
    chunk(0x282, &payload, false)
}

pub fn bits() -> Vec<u8> {
    let mut payload = 3u32.to_le_bytes().to_vec();
    payload.extend_from_slice(&1u16.to_le_bytes());
    payload.extend_from_slice(&[0, 1]);
    for word in [2u32, 0x8000_0003, 5] { payload.extend_from_slice(&word.to_le_bytes()); }
    chunk(0x283, &payload, false)
}

fn adaptive(kind: u8, bases: &[f32], scale: f32, filters: &[u8], factors: &[u8]) -> Vec<u8> {
    let mut payload = 33u32.to_le_bytes().to_vec();
    payload.extend_from_slice(&1u16.to_le_bytes());
    payload.extend_from_slice(&[bases.len() as u8, kind]);
    payload.extend_from_slice(&scale.to_le_bytes());
    for base in bases { payload.extend_from_slice(&base.to_le_bytes()); }
    for block in 0..2 {
        for component in 0..bases.len() {
            payload.push(filters[block * bases.len() + component]);
            payload.extend_from_slice(&[factors[block * bases.len() + component]; 8]);
        }
    }
    chunk(0x282, &payload, false)
}

pub fn adaptive_animation() -> Vec<u8> {
    animation(1, 33, &[
        adaptive(0, &[10.0], 0.5, &[16, 255], &[0x8f, 0x27]),
        adaptive(1, &[20.0], 2.0, &[255, 16], &[0xe7, 0x31]),
        adaptive(2, &[-30.0], 0.25, &[16, 16], &[0x3e, 0xf8]),
        adaptive(6, &[0.0, 0.0, 0.0, 1.0], 0.01,
            &[16, 16, 16, 16, 16, 16, 16, 16],
            &[0x11, 0xff, 0x22, 0x11, 0xff, 0x22, 0x11, 0x11]),
        bits(),
    ])
}

pub fn timecoded_animation() -> Vec<u8> {
    let h = std::f32::consts::FRAC_1_SQRT_2;
    animation(0, 9, &[
        timecoded(0, 1, &[(0, &[2.0]), (0x8000_0004, &[10.0]), (8, &[18.0])]),
        timecoded(1, 1, &[(0, &[3.0])]),
        timecoded(2, 1, &[(0, &[-4.0]), (8, &[4.0])]),
        timecoded(6, 4, &[(0, &[0.0, 0.0, 0.0, 1.0]),
            (4, &[-h, 0.0, 0.0, -h]), (8, &[h, 0.0, 0.0, h])]),
        bits(),
    ])
}

pub fn visible(frame: f32) -> bool { (3.0..5.0).contains(&frame) }

// Independent scalar oracle: source-ordered additions, including nibble sign.
fn sum(base: f32, frame: u32, scale: f32, filters: [f32; 2], factors: [[i32; 2]; 2]) -> f32 {
    let mut value = base;
    for delta in 0..frame {
        let block = delta as usize / 16;
        value += factors[block][delta as usize % 2] as f32 * (filters[block] * scale);
    }
    value
}

pub fn adaptive_expected(frame: f32) -> (Vec3, Quat) {
    let frame = frame.clamp(0.0, 32.0);
    let lo = frame as u32;
    let hi = (lo + 1).min(32);
    let t = frame - lo as f32;
    let last_filter = 1.0 - (239.0f32 / 240.0 * std::f32::consts::FRAC_PI_2).sin();
    assert!(last_filter > 0.0);
    let value = |base, scale, filters, factors| {
        let a = sum(base, lo, scale, filters, factors);
        let b = sum(base, hi, scale, filters, factors);
        a + (b - a) * t
    };
    let trans = Vec3::new(
        value(10.0, 0.5, [1.0, last_filter], [[-1, -8], [7, 2]]),
        value(20.0, 2.0, [last_filter, 1.0], [[7, -2], [1, 3]]),
        value(-30.0, 0.25, [1.0, 1.0], [[-2, 3], [-8, -1]]),
    );
    // Dot >= 1: source Fast_Slerp uses its unnormalized linear branch.
    let q = |base, factors| value(base, 0.01, [1.0, 1.0], factors);
    (trans, Quat::from_xyzw(q(0.0, [[1, 1], [-1, -1]]),
        q(0.0, [[-1, -1], [2, 2]]), q(0.0, [[2, 2], [1, 1]]),
        q(1.0, [[1, 1], [1, 1]])))
}

pub fn timecoded_expected(frame: f32) -> (Vec3, Quat) {
    let frame = frame.clamp(0.0, 8.0);
    (Vec3::new(if frame < 4.0 { 2.0 } else { 10.0 + (frame - 4.0) * 2.0 },
        3.0, -4.0 + frame),
        Quat::from_rotation_x(frame.min(4.0) / 4.0 * std::f32::consts::FRAC_PI_2))
}

pub fn assert_pose(actual: Mat4, expected: (Vec3, Quat), frame: f32) {
    let expected = Mat4::from_rotation_translation(expected.1, expected.0);
    for (a, b) in actual.to_cols_array().into_iter().zip(expected.to_cols_array()) {
        assert!((a - b).abs() < 0.00002, "frame {frame}: {actual:?} != {expected:?}");
    }
}

pub fn malformed_animations() -> Vec<Vec<u8>> {
    let mut packet = adaptive(0, &[1.0], 1.0, &[16, 16], &[0, 0]);
    packet.pop();
    let length = (packet.len() - 8) as u32;
    packet[4..8].copy_from_slice(&length.to_le_bytes());
    let mut truncated = adaptive_animation();
    truncated.pop();
    vec![
        animation(1, 33, &[packet]),
        truncated,
        animation(2, 9, &[]),
        animation(0, 0, &[]),
        animation(0, 9, &[timecoded(0, 1, &[])]),
        animation(0, 9, &[timecoded(0, 1, &[(1, &[1.0])])]),
        animation(0, 9, &[timecoded(0, 1, &[(0, &[1.0]), (0, &[2.0])])]),
        animation(0, 9, &[timecoded(0, 1, &[(0, &[1.0]), (9, &[2.0])])]),
        animation(0, 9, &[timecoded(0, 4, &[(0, &[0.0, 0.0, 0.0, 1.0])])]),
        animation(0, 9, &[chunk(0x283, &[0; 8], false)]),
        animation(1, 32, &[adaptive(0, &[1.0], 1.0, &[16, 16], &[0, 0])]),
        animation(1, 33, &[chunk(0x282, &[0; 12], false)]),
    ]
}

pub fn single_frame_animation() -> Vec<u8> {
    let mut payload = 1u32.to_le_bytes().to_vec();
    payload.extend_from_slice(&1u16.to_le_bytes());
    payload.extend_from_slice(&[1, 0]);
    payload.extend_from_slice(&1.0f32.to_le_bytes());
    payload.extend_from_slice(&2.5f32.to_le_bytes());
    animation(1, 1, &[chunk(0x282, &payload, false)])
}
