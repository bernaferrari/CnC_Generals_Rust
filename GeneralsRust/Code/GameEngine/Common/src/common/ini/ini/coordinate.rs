//! Original ordered labeled INI components for coordinates and packed colors.

use super::{INI, INIError, INIResult};

fn subtokens<'a>(tokens: &'a [&'a str]) -> impl Iterator<Item = &'a str> {
    // INI.cpp:170 m_sepsColon, also used by getNextSubToken:1648-1654.
    tokens
        .iter()
        .flat_map(|token| token.split([':', '=', ' ', '\t', '\r', '\n']))
        .filter(|part| !part.is_empty())
}

fn labeled_component<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
    expected: &str,
) -> INIResult<&'a str> {
    if !parts
        .next()
        .is_some_and(|label| label.eq_ignore_ascii_case(expected))
    {
        return Err(INIError::InvalidData);
    }
    parts.next().ok_or(INIError::InvalidData)
}

fn color_component<'a>(parts: &mut impl Iterator<Item = &'a str>, expected: &str) -> INIResult<u8> {
    let value = INI::parse_int(labeled_component(parts, expected)?)?;
    u8::try_from(value).map_err(|_| INIError::InvalidData)
}

impl INI {
    /// C++ INI.cpp:1079-1087 parses X, Y, Z in that order using
    /// getNextSubToken's case-insensitive labels and getSepsColon's whitespace,
    /// '=' and ':' separators. Coordinates remain in their authored axes.
    /// Like the original callback, this consumes three values, not later tokens.
    pub fn parse_coord_3d_ordered(tokens: &[&str]) -> INIResult<(f32, f32, f32)> {
        let mut parts = subtokens(tokens);
        let x = Self::parse_real(labeled_component(&mut parts, "X")?)?;
        let y = Self::parse_real(labeled_component(&mut parts, "Y")?)?;
        let z = Self::parse_real(labeled_component(&mut parts, "Z")?)?;
        Ok((x, y, z))
    }

    /// C++ INI.cpp:1032-1073 reads ordered R/G/B/[A] byte components.
    /// Omitted A is 255; Color.h:37-40 GameMakeColor packs ARGB. As in that
    /// callback, later tokens after the fourth component are not consumed.
    pub fn parse_color_int_ordered(tokens: &[&str]) -> INIResult<u32> {
        let mut parts = subtokens(tokens).peekable();
        let red = color_component(&mut parts, "R")?;
        let green = color_component(&mut parts, "G")?;
        let blue = color_component(&mut parts, "B")?;
        let alpha = if parts.peek().is_some() {
            color_component(&mut parts, "A")?
        } else {
            255
        };
        Ok((u32::from(alpha) << 24)
            | (u32::from(red) << 16)
            | (u32::from(green) << 8)
            | u32::from(blue))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_coord_3d_accepts_cpp_separators_and_preserves_authored_axes() {
        for values in [
            &["X:1.5", "Y:-2.25", "Z:30"][..],
            &["X:", "1.5", "Y:", "-2.25", "Z:", "30"],
            &["=", "x=1.5", "y=-2.25", "z=30"],
            &["X:=1.5", "Y:=-2.25", "Z:=30"],
            &["X", ":", "1.5", "Y", ":", "-2.25", "Z", ":", "30"],
            &["X:1.5 Y:-2.25 Z:30"],
            &[
                "X:1.5",
                "Y:-2.25",
                "Z:30",
                "//",
                "later tokens are not coordinates",
            ],
        ] {
            assert_eq!(INI::parse_coord_3d_ordered(values), Ok((1.5, -2.25, 30.0)));
        }
    }

    #[test]
    fn ordered_coord_3d_rejects_missing_reordered_or_invalid_components() {
        for values in [
            &[][..],
            &["1.5", "-2.25", "30"],
            &["Y:1", "X:2", "Z:3"],
            &["X:1", "Q:2", "Z:3"],
            &["X:1", "Y:2"],
            &["X:1", "Y:2", "Z:"],
            &["X:1", "Y:invalid", "Z:3"],
            &["X:1", "X:2", "Y:3", "Z:4"],
        ] {
            assert!(INI::parse_coord_3d_ordered(values).is_err(), "{values:?}");
        }
    }

    #[test]
    fn ordered_color_int_uses_cpp_argb_optional_alpha_and_split_subtokens() {
        for values in [
            &["R:32", "G:64", "B:128", "A:7"][..],
            &["R:", "32", "G:", "64", "B:", "128", "A:", "7"],
            &["=", "r=32", "g=64", "b=128", "a=7"],
            &["R:=32", "G:=64", "B:=128", "A:=7"],
            &["R : 32 G : 64 B : 128 A : 7"],
            &["R:32", "G:64", "B:128", "A:7", "//", "ignored after alpha"],
        ] {
            assert_eq!(INI::parse_color_int_ordered(values), Ok(0x0720_4080));
        }
        assert_eq!(
            INI::parse_color_int_ordered(&["R:32", "G:64", "B:128"]),
            Ok(0xff20_4080)
        );
        assert_eq!(
            INI::parse_color_int_ordered(&["R:0", "G:255", "B:0", "A:0"]),
            Ok(0x0000_ff00)
        );
    }

    #[test]
    fn ordered_color_int_rejects_unlabeled_reordered_and_out_of_range_components() {
        for values in [
            &[][..],
            &["32", "64", "128", "7"],
            &["287454020"],
            &["G:64", "R:32", "B:128"],
            &["R:32", "G:64"],
            &["R:-1", "G:64", "B:128"],
            &["R:256", "G:64", "B:128"],
            &["R:32", "G:invalid", "B:128"],
            &["R:32", "G:64", "B:128", "A:"],
            &["R:32", "G:64", "B:128", "A:256"],
            &["R:32", "G:64", "B:128", "Q:7"],
            &["R:32", "G:64", "B:128", "//", "not alpha"],
        ] {
            assert!(INI::parse_color_int_ordered(values).is_err(), "{values:?}");
        }
    }
}
