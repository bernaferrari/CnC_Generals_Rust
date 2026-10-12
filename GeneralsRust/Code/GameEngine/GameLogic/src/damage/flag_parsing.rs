//! Original INI.cpp1780-1812: begin with all bits, then ALL/NONE/+name/-name.
use super::{DamageType, DamageTypeFlags};
use game_engine::common::ini::INIError;
use std::str::FromStr;

pub fn parse_damage_type_flags_tokens(tokens: &[&str]) -> Result<DamageTypeFlags, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let mut flags = DamageTypeFlags::all_flags();
    for token in tokens {
        for entry in token.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if entry.eq_ignore_ascii_case("ALL") {
                flags = DamageTypeFlags::all_flags();
                continue;
            }
            if entry.eq_ignore_ascii_case("NONE") {
                flags = DamageTypeFlags::empty();
                continue;
            }
            let (remove, name) = if let Some(name) = entry.strip_prefix('-') {
                (true, name)
            } else if let Some(name) = entry.strip_prefix('+') {
                (false, name)
            } else {
                return Err(INIError::InvalidData);
            };
            let damage = DamageType::from_str(name).map_err(|_| INIError::InvalidData)?;
            flags.set(
                DamageTypeFlags::from_bits_truncate(1 << damage as u64),
                !remove,
            );
        }
    }
    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_flags_start_from_all_and_apply_operations_in_cpp_order() {
        let flags = parse_damage_type_flags_tokens(&["-FLAME"]).unwrap();
        assert!(!flags.contains_damage_type(DamageType::Flame));
        assert!(flags.contains_damage_type(DamageType::Explosion));
        let flags =
            parse_damage_type_flags_tokens(&["NONE", "+FLAME", "-FLAME", "+SMALL_ARMS"]).unwrap();
        assert_eq!(flags, DamageTypeFlags::SMALL_ARMS);
        assert_eq!(
            parse_damage_type_flags_tokens(&["NONE", "ALL"]).unwrap(),
            DamageTypeFlags::all_flags()
        );
    }

    #[test]
    fn original_damage_flags_reject_missing_bare_and_unknown_tokens() {
        for tokens in [
            &[][..],
            &["FLAME"][..],
            &["+UNKNOWN"][..],
            &["ALL", "-"][..],
        ] {
            assert!(parse_damage_type_flags_tokens(tokens).is_err());
        }
    }
}
