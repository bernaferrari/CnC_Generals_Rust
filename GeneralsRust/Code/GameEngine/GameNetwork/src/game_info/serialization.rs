// FILE: game_info/serialization.rs
// Port of GameInfo.cpp serialization functions
// Matches C++ GameInfoToAsciiString and ParseAsciiStringToGameInfo exactly

use super::*;

/// Convert GameInfo to ASCII string format (matches C++ GameInfoToAsciiString)
/// Format: US=val;M=mask+mapname;MC=crc;MS=size;SD=seed;C=crc;SR=restriction;SC=cash;O=Y/N;S=slot1:slot2:...;
pub fn game_info_to_ascii_string(game: &GameInfo) -> String {
    // Process map name - convert to portable format and extract directory
    let map_name = game.get_map();
    let new_map_name = extract_map_directory(map_name);

    let mut options = format!(
        "US={};M={:02x}{};MC={:X};MS={};SD={};C={};SR={};SC={};O={};",
        game.get_use_stats(),
        game.get_map_contents_mask(),
        new_map_name,
        game.get_map_crc(),
        game.get_map_size(),
        game.get_seed(),
        game.get_crc_interval(),
        game.get_superweapon_restriction(),
        game.get_starting_cash().count_money(),
        if game.old_factions_only() { 'Y' } else { 'N' }
    );

    // Add player info for each slot
    options.push('S');
    options.push('=');

    for i in 0..MAX_SLOTS {
        if let Some(slot) = game.get_slot(i) {
            let slot_str = serialize_slot(slot, i, &options);
            options.push_str(&slot_str);
        } else {
            // Empty slots are serialized as closed (matches C++ behavior)
            options.push_str("X:");
        }
    }

    options.push(';');

    // Ensure we don't exceed max length
    if options.len() >= LAN_MAX_OPTIONS_LENGTH {
        eprintln!(
            "WARNING: options string is longer than expected! Length is {}, but max is {}!",
            options.len(),
            LAN_MAX_OPTIONS_LENGTH
        );
    }

    options
}

/// Extract map directory from full path (matches C++ logic)
fn extract_map_directory(map_name: &str) -> String {
    if map_name.is_empty() {
        return String::new();
    }

    let mut tokens: Vec<&str> = map_name.split(&['\\', '/'][..]).collect();

    // Remove the last token (filename)
    if tokens.len() > 1 {
        tokens.pop();
        tokens.join("/")
    } else {
        // No directory, just filename
        String::new()
    }
}

/// Serialize a single slot (matches C++ format exactly)
fn serialize_slot(slot: &GameSlot, slot_idx: usize, current_options: &str) -> String {
    if slot.is_human() {
        // Human player: Hname,IP,port,TT,color,template,pos,team,nat:
        let tmp = format!(
            ",{:X},{},{}{},{},{},{},{},{}:",
            slot.get_ip(),
            slot.get_port(),
            if slot.is_accepted() { 'T' } else { 'F' },
            if slot.has_map() { 'T' } else { 'F' },
            slot.get_color(),
            slot.get_player_template(),
            slot.get_start_pos(),
            slot.get_team_number(),
            slot.get_nat_behavior() as u8
        );

        // Calculate max name length to avoid overflow
        let len_cur = tmp.len() + current_options.len() + 2; // +2 for H and trailing ;
        let len_rem = LAN_MAX_OPTIONS_LENGTH.saturating_sub(len_cur);
        let len_max = len_rem / (MAX_SLOTS - slot_idx);

        let mut name = slot.get_name().to_string();
        while name.len() > len_max {
            name.pop();
        }

        format!("H{}{}", name, tmp)
    } else if slot.is_ai() {
        // AI player: CE/M/H,color,template,pos,team:
        let ai_char = match slot.get_state() {
            SlotState::EasyAI => 'E',
            SlotState::MedAI => 'M',
            SlotState::BrutalAI => 'H',
            _ => 'M',
        };

        format!(
            "C{},{},{},{},{}:",
            ai_char,
            slot.get_color(),
            slot.get_player_template(),
            slot.get_start_pos(),
            slot.get_team_number()
        )
    } else if slot.get_state() == SlotState::Open {
        "O:".to_string()
    } else {
        // Closed
        "X:".to_string()
    }
}

/// Parse ASCII string to GameInfo (matches C++ ParseAsciiStringToGameInfo)
pub fn parse_ascii_string_to_game_info(game: &mut GameInfo, options: &str) -> bool {
    let mut new_slots = vec![GameSlot::new(); MAX_SLOTS];
    let mut map_name = String::new();
    let mut map_contents_mask = 0i32;
    let mut map_crc = 0u32;
    let mut map_size = 0u32;
    let mut seed = 0i32;
    let mut crc = 100i32;
    let mut use_stats = 1i32;
    let mut starting_cash = Money::default();
    let mut restriction = 0u16;
    let mut old_factions_only = false;

    let mut saw_map = false;
    let mut saw_map_crc = false;
    let mut saw_map_size = false;
    let mut saw_seed = false;
    let mut saw_slotlist = false;
    let mut saw_crc = false;
    let mut saw_use_stats = false;
    let mut saw_superweapon_restriction = false;
    let mut saw_starting_cash = false;
    let mut saw_old_factions = false;

    // Parse key-value pairs separated by semicolons
    for pair in options.split(';') {
        if pair.is_empty() {
            continue;
        }

        let parts: Vec<&str> = pair.splitn(2, '=').collect();
        if parts.len() != 2 {
            continue;
        }

        let key = parts[0];
        let val = parts[1];

        if val.is_empty() {
            return false;
        }

        match key {
            "US" => {
                use_stats = val.parse().unwrap_or(1);
                saw_use_stats = true;
            }
            "M" => {
                if val.len() < 3 {
                    return false;
                }

                map_contents_mask = parse_hex_byte(&val[0..2]);
                let map_path = &val[2..];

                // Reconstruct full map path
                let tokens: Vec<&str> = map_path.split('/').collect();
                map_name = String::new();
                for token in &tokens {
                    if !map_name.is_empty() {
                        map_name.push('\\');
                    }
                    map_name.push_str(token);
                }

                // Add directory name as filename with .map extension
                if let Some(last) = tokens.last() {
                    if !map_name.is_empty() {
                        map_name.push('\\');
                    }
                    map_name.push_str(last);
                    map_name.push_str(".map");
                }

                saw_map = true;
            }
            "MC" => {
                map_crc = u32::from_str_radix(val, 16).unwrap_or(0);
                saw_map_crc = true;
            }
            "MS" => {
                map_size = val.parse().unwrap_or(0);
                saw_map_size = true;
            }
            "SD" => {
                seed = val.parse().unwrap_or(0);
                saw_seed = true;
            }
            "C" => {
                crc = val.parse().unwrap_or(100);
                saw_crc = true;
            }
            "SR" => {
                restriction = val.parse().unwrap_or(0);
                saw_superweapon_restriction = true;
            }
            "SC" => {
                let amount = val.parse().unwrap_or(10000);
                starting_cash.init();
                starting_cash.deposit(amount);
                saw_starting_cash = true;
            }
            "O" => {
                old_factions_only = val.eq_ignore_ascii_case("Y");
                saw_old_factions = true;
            }
            "S" => {
                saw_slotlist = true;
                if !parse_slot_list(val, &mut new_slots) {
                    return false;
                }
            }
            _ => {
                // Unknown key
                return false;
            }
        }
    }

    // Verify all required fields were present
    if !(saw_map
        && saw_map_crc
        && saw_map_size
        && saw_seed
        && saw_slotlist
        && saw_crc
        && saw_use_stats
        && saw_superweapon_restriction
        && saw_starting_cash
        && saw_old_factions)
    {
        return false;
    }

    // Apply to game info
    for (i, slot) in new_slots.into_iter().enumerate() {
        game.set_slot(i, slot);
    }

    game.set_map(map_name);
    game.set_map_crc(map_crc);
    game.set_map_size(map_size);
    game.set_map_contents_mask(map_contents_mask);
    game.set_seed(seed);
    game.set_crc_interval(crc);
    game.set_use_stats(use_stats);
    game.set_superweapon_restriction(restriction);
    game.set_starting_cash(starting_cash);
    game.set_old_factions_only(old_factions_only);

    true
}

/// Parse hex byte from string (e.g., "0F" -> 15)
fn parse_hex_byte(s: &str) -> i32 {
    i32::from_str_radix(s, 16).unwrap_or(0)
}

fn max_multiplayer_colors() -> i32 {
    lookup_multiplayer_settings()
        .map(|settings| settings.color_values.len() as i32)
        .filter(|count| *count > 0)
        .unwrap_or(16)
}

fn is_valid_player_template_index(index: i32) -> bool {
    if index < PLAYERTEMPLATE_MIN {
        return false;
    }
    if index < 0 {
        return true;
    }
    lookup_player_template_display_name(index).is_some() || index < 32
}

/// Parse slot list (matches C++ parsing logic exactly)
fn parse_slot_list(slot_data: &str, slots: &mut [GameSlot]) -> bool {
    // Split by ':' and filter out trailing empty entry (slots end with ':')
    let raw_slots: Vec<&str> = slot_data.split(':').filter(|s| !s.is_empty()).collect();

    if raw_slots.len() != MAX_SLOTS {
        return false;
    }

    for (i, raw_slot) in raw_slots.iter().enumerate() {
        if raw_slot.is_empty() {
            return false;
        }

        let first_char = raw_slot.chars().next().unwrap();

        match first_char {
            'H' => {
                // Human player: Hname,IP,port,TT,color,template,pos,team,nat
                if !parse_human_slot(&raw_slot[1..], &mut slots[i]) {
                    return false;
                }
            }
            'C' => {
                // AI player: CE/M/H,color,template,pos,team
                if !parse_ai_slot(&raw_slot[1..], &mut slots[i]) {
                    return false;
                }
            }
            'O' => {
                // Open slot
                slots[i].set_state(SlotState::Open, String::new(), 0);
            }
            'X' => {
                // Closed slot
                slots[i].set_state(SlotState::Closed, String::new(), 0);
            }
            _ => {
                return false;
            }
        }
    }

    true
}

/// Parse human player slot
fn parse_human_slot(data: &str, slot: &mut GameSlot) -> bool {
    let parts: Vec<&str> = data.split(',').collect();

    if parts.len() != 9 {
        return false;
    }

    // Parse components
    let name = parts[0].to_string();
    let ip = match u32::from_str_radix(parts[1], 16) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let port: u16 = match parts[2].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };

    if parts[3].len() != 2 {
        return false;
    }
    let is_accepted = match parts[3].chars().nth(0) {
        Some(c) => c == 'T',
        None => return false,
    };
    let has_map = match parts[3].chars().nth(1) {
        Some(c) => c == 'T',
        None => return false,
    };

    let color: i32 = match parts[4].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let player_template: i32 = match parts[5].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let start_pos: i32 = match parts[6].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let team: i32 = match parts[7].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let nat_behavior: u8 = match parts[8].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };

    // Validate ranges
    if color < -1 || color >= max_multiplayer_colors() {
        return false;
    }

    if !is_valid_player_template_index(player_template) {
        return false;
    }

    if start_pos < -1 || start_pos >= MAX_SLOTS as i32 {
        return false;
    }

    if team < -1 || team >= (MAX_SLOTS / 2) as i32 {
        return false;
    }

    // Set slot data
    slot.set_state(SlotState::Player, name, ip);
    slot.set_port(port);

    if is_accepted {
        slot.set_accept();
    } else {
        slot.un_accept();
    }

    slot.set_map_availability(has_map);
    slot.set_color(color);
    slot.set_player_template(player_template);
    slot.set_start_pos(start_pos);
    slot.set_team_number(team);

    // Set NAT behavior
    let nat = match nat_behavior {
        0 => FirewallBehaviorType::Unknown,
        1 => FirewallBehaviorType::Simple,
        2 => FirewallBehaviorType::DumbMangling,
        4 => FirewallBehaviorType::SmartMangling,
        8 => FirewallBehaviorType::NetgearBug,
        16 => FirewallBehaviorType::SimplePortAllocation,
        32 => FirewallBehaviorType::RelativePortAllocation,
        64 => FirewallBehaviorType::DestinationPortDelta,
        _ => return false,
    };
    slot.set_nat_behavior(nat);

    true
}

/// Parse AI player slot
fn parse_ai_slot(data: &str, slot: &mut GameSlot) -> bool {
    let parts: Vec<&str> = data.split(',').collect();

    if parts.len() != 5 {
        return false;
    }

    // Parse AI difficulty
    if parts[0].len() != 1 {
        return false;
    }

    let state = match parts[0].chars().next() {
        Some('E') => SlotState::EasyAI,
        Some('M') => SlotState::MedAI,
        Some('H') => SlotState::BrutalAI,
        _ => return false,
    };

    let color: i32 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let player_template: i32 = match parts[2].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let start_pos: i32 = match parts[3].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let team: i32 = match parts[4].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };

    // Validate ranges
    if color < -1 || color >= max_multiplayer_colors() {
        return false;
    }

    if !is_valid_player_template_index(player_template) {
        return false;
    }

    if start_pos < -1 || start_pos >= MAX_SLOTS as i32 {
        return false;
    }

    if team < -1 || team >= (MAX_SLOTS / 2) as i32 {
        return false;
    }

    slot.set_state(state, String::new(), 0);
    slot.set_color(color);
    slot.set_player_template(player_template);
    slot.set_start_pos(start_pos);
    slot.set_team_number(team);

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_empty() {
        let mut info = GameInfo::new();
        // Set a proper map path - games always have valid map paths
        info.set_map("Maps\\Tournament\\TourneyDesert.map".to_string());
        let serialized = game_info_to_ascii_string(&info);

        let mut info2 = GameInfo::new();
        let result = parse_ascii_string_to_game_info(&mut info2, &serialized);

        assert!(result);
        assert_eq!(info2.get_seed(), info.get_seed());
        // Map path gets normalized during roundtrip
        assert!(info2.get_map().contains("Tournament"));
    }

    #[test]
    fn test_serialize_with_player() {
        let mut info = GameInfo::new();
        // Set a valid map path for proper serialization roundtrip
        info.set_map("Maps\\Tournament\\TourneyDesert.map".to_string());

        let mut slot = GameSlot::new();
        slot.set_state(SlotState::Player, "TestPlayer".to_string(), 0x12345678);
        slot.set_color(5);
        slot.set_player_template(2);
        slot.set_start_pos(3);
        slot.set_team_number(1);
        info.set_slot(0, slot);

        let serialized = game_info_to_ascii_string(&info);

        let mut info2 = GameInfo::new();
        let result = parse_ascii_string_to_game_info(&mut info2, &serialized);

        assert!(result);

        let slot2 = info2.get_slot(0).unwrap();
        assert_eq!(slot2.get_name(), "TestPlayer");
        assert_eq!(slot2.get_ip(), 0x12345678);
        assert_eq!(slot2.get_color(), 5);
        assert_eq!(slot2.get_player_template(), 2);
        assert_eq!(slot2.get_start_pos(), 3);
        assert_eq!(slot2.get_team_number(), 1);
    }

    #[test]
    fn test_serialize_with_ai() {
        let mut info = GameInfo::new();
        // Set a valid map path for proper serialization roundtrip
        info.set_map("Maps\\Tournament\\TourneyDesert.map".to_string());

        let mut slot = GameSlot::new();
        slot.set_state(SlotState::BrutalAI, String::new(), 0);
        slot.set_color(2);
        slot.set_player_template(1);
        slot.set_start_pos(4);
        slot.set_team_number(0);
        info.set_slot(1, slot);

        let serialized = game_info_to_ascii_string(&info);

        let mut info2 = GameInfo::new();
        let result = parse_ascii_string_to_game_info(&mut info2, &serialized);

        assert!(result);

        let slot2 = info2.get_slot(1).unwrap();
        assert_eq!(slot2.get_state(), SlotState::BrutalAI);
        assert_eq!(slot2.get_color(), 2);
        assert_eq!(slot2.get_player_template(), 1);
        assert_eq!(slot2.get_start_pos(), 4);
        assert_eq!(slot2.get_team_number(), 0);
    }

    #[test]
    fn test_parse_hex_byte() {
        assert_eq!(parse_hex_byte("00"), 0);
        assert_eq!(parse_hex_byte("FF"), 255);
        assert_eq!(parse_hex_byte("0A"), 10);
        assert_eq!(parse_hex_byte("10"), 16);
    }

    #[test]
    fn test_extract_map_directory() {
        assert_eq!(
            extract_map_directory("Maps\\Tournament\\TourneyDesert1.map"),
            "Maps/Tournament"
        );
        assert_eq!(
            extract_map_directory("Maps/Official/Desert.map"),
            "Maps/Official"
        );
        assert_eq!(extract_map_directory("simple.map"), "");
    }
}
