//! C++ `INI::parseColorInt` / `GameMakeColor` for ThingTemplate `DisplayColor`.
//!
//! Retail Object.ini uses labeled `R:255 G:128 B:0 [A:255]` tokens, not a hex
//! RRGGBB word. Matches INI.cpp:1032-1071.

use crate::common::rts::Color;

/// Packed ARGB (`GameMakeColor`).
pub fn game_make_color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
}

/// Parse a DisplayColor value string (`R:255 G:128 B:0` or `R 255 G 128 B 0`).
pub fn parse_color_int(s: &str) -> Result<Color, ()> {
    let tokens: Vec<&str> = s.split_whitespace().collect();
    parse_color_int_tokens(&tokens)
}

fn parse_color_int_tokens(tokens: &[&str]) -> Result<Color, ()> {
    let mut r: Option<i32> = None;
    let mut g: Option<i32> = None;
    let mut b: Option<i32> = None;
    let mut a: Option<i32> = None;

    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];
        let (key, value) = if let Some((left, right)) = token.split_once(':') {
            if right.is_empty() {
                i += 1;
                if i >= tokens.len() {
                    return Err(());
                }
                (left, tokens[i])
            } else {
                (left, right)
            }
        } else {
            i += 1;
            if i >= tokens.len() {
                return Err(());
            }
            (token, tokens[i])
        };

        let value: i32 = value.parse().map_err(|_| ())?;
        if !(0..=255).contains(&value) {
            return Err(());
        }
        match key.to_ascii_uppercase().as_str() {
            "R" => r = Some(value),
            "G" => g = Some(value),
            "B" => b = Some(value),
            "A" => a = Some(value),
            _ => {}
        }
        i += 1;
    }

    let r = r.ok_or(())?;
    let g = g.ok_or(())?;
    let b = b.ok_or(())?;
    let a = a.unwrap_or(255);
    Ok(game_make_color(r as u8, g as u8, b as u8, a as u8))
}
