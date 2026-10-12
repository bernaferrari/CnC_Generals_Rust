//! AI.h291 / AIUpdate.cpp5160: raw32 enum values, independent of Rust layout.
use crate::modules::AIAttitudeType;
pub(super) fn encode(attitude: AIAttitudeType) -> u32 {
    match attitude {
        AIAttitudeType::Normal => 0,
        AIAttitudeType::Defensive => 1, // CPP AI_ALERT
        AIAttitudeType::Aggressive => 2,
        AIAttitudeType::Passive => (-1i32) as u32,
        AIAttitudeType::Sleep => (-2i32) as u32,
    }
}
pub(super) fn decode(value: u32) -> Result<AIAttitudeType, String> {
    Ok(match value {
        0 => AIAttitudeType::Normal,
        1 => AIAttitudeType::Defensive,
        2 => AIAttitudeType::Aggressive,
        u32::MAX => AIAttitudeType::Passive,
        0xffff_fffe => AIAttitudeType::Sleep,
        _ => return Err(format!("invalid or unsupported saved AI attitude {value}")),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_cpp_enum_bytes_do_not_depend_on_rust_variant_order() {
        for (attitude, bytes) in [
            (AIAttitudeType::Sleep, [0xfe, 0xff, 0xff, 0xff]),
            (AIAttitudeType::Passive, [0xff, 0xff, 0xff, 0xff]),
            (AIAttitudeType::Normal, [0, 0, 0, 0]),
            (AIAttitudeType::Defensive, [1, 0, 0, 0]),
            (AIAttitudeType::Aggressive, [2, 0, 0, 0]),
        ] {
            assert_eq!(encode(attitude).to_le_bytes(), bytes);
            assert_eq!(decode(u32::from_le_bytes(bytes)).unwrap(), attitude);
        }
        assert!(decode(3).is_err()); // CPP AI_INVALID has no runnable Rust state.
        assert!(decode(4).is_err());
    }
}
