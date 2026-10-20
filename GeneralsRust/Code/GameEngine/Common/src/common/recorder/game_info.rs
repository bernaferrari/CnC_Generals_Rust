//! Replay GameInfo serialization and the shared receiving/menu decode grammar.
//! Existing Reader admission stays permissive; metadata uses explicit loaded limits.
use super::{PLAYERTEMPLATE_OBSERVER, ReplayGameInfo, ReplaySlot};

/// Serialize lobby options like C++ `GameInfoToAsciiString` (GameInfo.cpp:875-972).
pub(super) fn format_replay_game_info(info: &ReplayGameInfo) -> String {
    let map = info.map.replace('\\', "/");
    let mut options = format!(
        "US={};M={:02x}{};MC={:X};MS={};SD={};C={};SR={};SC={};O={};S=",
        info.use_stats,
        info.map_contents_mask & 0xff,
        map,
        info.map_crc,
        info.map_size,
        info.seed,
        info.crc_interval,
        info.superweapon_restriction,
        info.starting_cash,
        if info.old_factions_only { 'Y' } else { 'N' },
    );
    for slot in info.slots.iter().take(8) {
        if slot.is_occupied && slot.is_human {
            options.push_str(&format!(
                "H{},{:X},{},{}{},{},{},{},{},{}:",
                slot.name,
                slot.ip,
                slot.port,
                if slot.accepted { 'T' } else { 'F' },
                if slot.has_map { 'T' } else { 'F' },
                slot.color,
                slot.player_template,
                slot.start_pos,
                slot.team,
                slot.nat
            ));
        } else if slot.is_occupied {
            let ai = match slot.ai_difficulty {
                1 => 'E',
                3 => 'H',
                _ => 'M',
            };
            options.push_str(&format!(
                "C{},{},{},{},{}:",
                ai, slot.color, slot.player_template, slot.start_pos, slot.team
            ));
        } else if slot.is_open {
            options.push_str("O:");
        } else {
            options.push_str("X:");
        }
    }
    options.push(';');
    options
}

/// Decode menu metadata through the same plain DTO parser as the actual header.
/// Listing retains the original complete-GameInfo admission rule, without
/// constructing a GameNetwork GameInfo or publishing its local-slot cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayMenuMetadataLimits {
    pub color_count: i32,
    pub player_template_count: i32,
}

pub fn decode_replay_menu_metadata(
    options: &str,
    limits: ReplayMenuMetadataLimits,
) -> Option<ReplayGameInfo> {
    if limits.color_count < 0 || limits.player_template_count < 0 {
        return None;
    }
    const KEYS: [&str; 10] = ["US", "M", "MC", "MS", "SD", "C", "SR", "SC", "O", "S"];
    let mut seen = [false; 10];
    for pair in options.split(';').filter(|pair| !pair.is_empty()) {
        let Some((key, value)) = pair.split_once('=') else {
            return None;
        };
        let index = KEYS.iter().position(|expected| *expected == key)?;
        if value.is_empty() || (key == "M" && (value.len() < 3 || !value.is_char_boundary(2))) {
            return None;
        }
        seen[index] = true;
    }
    if !seen.into_iter().all(|present| present) {
        return None;
    }
    let mut metadata = ReplayGameInfo::new();
    parse_replay_game_info_with_limits(options, &mut metadata, Some(limits)).then_some(metadata)
}

/// Parse C++ `GameInfoToAsciiString` output into `ReplayGameInfo`.
/// Matches `ParseAsciiStringToGameInfo` (GameInfo.cpp:982-1470) enough to restore
/// map/seed/CRC/slot color/template/start/team/NAT.
pub(super) fn parse_replay_game_info(options: &str, info: &mut ReplayGameInfo) -> bool {
    parse_replay_game_info_with_limits(options, info, None)
}

fn parse_replay_game_info_with_limits(
    options: &str,
    info: &mut ReplayGameInfo,
    limits: Option<ReplayMenuMetadataLimits>,
) -> bool {
    if options.is_empty() {
        return false;
    }
    let mut saw_seed = false;
    let mut saw_slots = false;
    for pair in options.split(';') {
        if pair.is_empty() {
            continue;
        }
        let (key, val) = match pair.split_once('=') {
            Some(kv) => kv,
            None => continue,
        };
        match key {
            "US" => {
                info.use_stats = val.parse().unwrap_or(0);
            }
            "M" => {
                if val.len() >= 2 {
                    // Corrupt UTF-8 mask bytes have no valid Rust slicing
                    // boundary. Reject instead of panicking during header IO.
                    if !val.is_char_boundary(2) {
                        return false;
                    }
                    info.map_contents_mask = u32::from_str_radix(&val[..2], 16).unwrap_or(0);
                    info.map = val[2..].to_string();
                }
            }
            "MC" => {
                info.map_crc = u32::from_str_radix(val, 16).unwrap_or(0);
            }
            "MS" => {
                info.map_size = val.parse().unwrap_or(0);
            }
            "SD" => {
                info.seed = val.parse().unwrap_or(0);
                saw_seed = true;
            }
            "C" => {
                info.crc_interval = val.parse().unwrap_or(100);
            }
            "SR" => {
                info.superweapon_restriction = val.parse().unwrap_or(0);
            }
            "SC" => {
                info.starting_cash = val.parse().unwrap_or(0);
            }
            "O" => {
                info.old_factions_only = val.eq_ignore_ascii_case("Y");
            }
            "S" => {
                saw_slots = parse_replay_slot_list(val, info, limits);
            }
            _ => {}
        }
    }
    saw_seed && saw_slots
}

fn decode_replay_field<T: std::str::FromStr>(text: &str, fallback: T, strict: bool) -> Option<T> {
    match text.parse() {
        Ok(value) => Some(value),
        Err(_) if !strict => Some(fallback),
        Err(_) => None,
    }
}

fn parse_replay_slot_list(
    raw: &str,
    info: &mut ReplayGameInfo,
    limits: Option<ReplayMenuMetadataLimits>,
) -> bool {
    let strict = limits.is_some();
    let mut parts: Vec<&str> = raw.split(':').collect();
    if strict {
        parts.retain(|part| !part.is_empty());
    } else if parts.last() == Some(&"") {
        parts.pop();
    }
    if parts.len() < 8 || (strict && parts.len() != 8) {
        return false;
    }
    for (i, part) in parts.iter().take(8).enumerate() {
        let Some(slot) = info.slots.get_mut(i) else {
            return false;
        };
        *slot = ReplaySlot::default();
        if part.is_empty() {
            return false;
        }
        match part.as_bytes()[0] {
            b'X' => {}
            b'O' => {
                slot.is_open = true;
            }
            b'H' => {
                let fields: Vec<&str> = part[1..].split(',').collect();
                if fields.len() < 9 || (strict && fields.len() != 9) {
                    return false;
                }
                slot.name = fields[0].to_string();
                slot.ip = match u32::from_str_radix(fields[1], 16) {
                    Ok(value) => value,
                    Err(_) if !strict => 0,
                    Err(_) => return false,
                };
                let Some(port) = decode_replay_field(fields[2], 0u32, strict) else {
                    return false;
                };
                if strict && u16::try_from(port).is_err() {
                    return false;
                }
                slot.port = port;
                let flags = fields[3].as_bytes();
                if strict && flags.len() != 2 {
                    return false;
                }
                slot.accepted = flags.first() == Some(&b'T');
                slot.has_map = flags.get(1) == Some(&b'T');
                let Some(color) = decode_replay_field(fields[4], -1, strict) else {
                    return false;
                };
                slot.color = color;
                let Some(template) = decode_replay_field(fields[5], -1, strict) else {
                    return false;
                };
                slot.player_template = template;
                let Some(start) = decode_replay_field(fields[6], -1, strict) else {
                    return false;
                };
                slot.start_pos = start;
                let Some(team) = decode_replay_field(fields[7], -1, strict) else {
                    return false;
                };
                slot.team = team;
                let Some(nat) = decode_replay_field(fields[8], 0, strict) else {
                    return false;
                };
                if strict && !(0..=128).contains(&nat) {
                    return false;
                }
                slot.nat = nat;
                slot.is_human = true;
                slot.is_occupied = true;
                slot.is_observer = slot.player_template == PLAYERTEMPLATE_OBSERVER;
            }
            b'C' => {
                let fields: Vec<&str> = part.split(',').collect();
                if fields.len() < 5 || (strict && fields.len() != 5) {
                    return false;
                }
                let difficulty = fields[0].as_bytes().get(1);
                if strict
                    && (fields[0].len() != 2 || !matches!(difficulty, Some(&b'E' | &b'M' | &b'H')))
                {
                    return false;
                }
                slot.ai_difficulty = match difficulty {
                    Some(&b'E') => 1,
                    Some(&b'H') => 3,
                    _ => 2,
                };
                let Some(color) = decode_replay_field(fields[1], -1, strict) else {
                    return false;
                };
                slot.color = color;
                let Some(template) = decode_replay_field(fields[2], -1, strict) else {
                    return false;
                };
                slot.player_template = template;
                let Some(start) = decode_replay_field(fields[3], -1, strict) else {
                    return false;
                };
                slot.start_pos = start;
                let Some(team) = decode_replay_field(fields[4], -1, strict) else {
                    return false;
                };
                slot.team = team;
                slot.is_occupied = true;
                slot.is_observer = slot.player_template == PLAYERTEMPLATE_OBSERVER;
            }
            _ => return false,
        }
        if let Some(limits) = limits
            && slot.is_occupied
        {
            if slot.color < -1
                || slot.color >= limits.color_count
                || slot.player_template < -2
                || slot.player_template >= limits.player_template_count
                || slot.start_pos < -1
                || slot.start_pos >= 8
                || slot.team < -1
                || slot.team >= 4
            {
                return false;
            }
        }
    }
    true
}
