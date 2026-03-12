//! PlayerTemplate INI parsing and store population.

use crate::common::game_common::{VeterancyLevel, VETERANCY_NAMES};
use crate::common::ini::ini::{FieldParse, INIError, INIResult, INI};
use crate::common::language::Language;
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::rts::money::Money;
use crate::common::rts::player_template::{get_player_template_store_mut, PlayerTemplate};

const MAX_STARTING_UNITS: usize = 10;

fn parse_ascii_string(tokens: &[&str]) -> INIResult<String> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_ascii_string(token)
}

fn parse_bool(tokens: &[&str]) -> INIResult<bool> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_bool(token)
}

fn parse_int(tokens: &[&str]) -> INIResult<i32> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_int(token)
}

fn parse_display_name(tokens: &[&str]) -> INIResult<String> {
    let value = parse_ascii_string(tokens)?;
    if let Some(stripped) = value.strip_prefix("INI:") {
        return Ok(Language::get_localized_string(stripped));
    }
    Ok(value)
}

fn parse_rgb_color(tokens: &[&str]) -> INIResult<u32> {
    let (r, g, b) = INI::parse_rgb_color(tokens)?;
    let r = (r * 255.0).round().clamp(0.0, 255.0) as u32;
    let g = (g * 255.0).round().clamp(0.0, 255.0) as u32;
    let b = (b * 255.0).round().clamp(0.0, 255.0) as u32;
    Ok((r << 16) | (g << 8) | b)
}

fn parse_veterancy_level(token: &str) -> INIResult<VeterancyLevel> {
    let index = INI::parse_index_list(token, &VETERANCY_NAMES)?;
    match index {
        0 => Ok(VeterancyLevel::Regular),
        1 => Ok(VeterancyLevel::Veteran),
        2 => Ok(VeterancyLevel::Elite),
        3 => Ok(VeterancyLevel::Heroic),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_science_list(tokens: &[&str]) -> Vec<String> {
    let mut values = Vec::new();
    for token in tokens {
        let cleaned = token.trim().trim_end_matches(',');
        if cleaned.eq_ignore_ascii_case("None") {
            return Vec::new();
        }
        if !cleaned.is_empty() {
            values.push(cleaned.to_string());
        }
    }
    values
}

fn parse_side(_ini: &mut INI, template: &mut PlayerTemplate, tokens: &[&str]) -> INIResult<()> {
    template.side = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_base_side(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.base_side = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_playable_side(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.playable = parse_bool(tokens)?;
    Ok(())
}

fn parse_display_name_field(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.display_name = parse_display_name(tokens)?;
    Ok(())
}

fn parse_start_money(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    let value = parse_int(tokens)? as u32;
    template.starting_money = Money::new_with_amount(value);
    Ok(())
}

fn parse_preferred_color(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.preferred_color = parse_rgb_color(tokens)?;
    Ok(())
}

fn parse_starting_building(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.starting_building = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_starting_unit(
    index: usize,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    if index >= MAX_STARTING_UNITS {
        return Err(INIError::InvalidData);
    }
    template.starting_units[index] = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_starting_unit0(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    parse_starting_unit(0, template, tokens)
}

fn parse_starting_unit1(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    parse_starting_unit(1, template, tokens)
}

fn parse_starting_unit2(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    parse_starting_unit(2, template, tokens)
}

fn parse_starting_unit3(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    parse_starting_unit(3, template, tokens)
}

fn parse_starting_unit4(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    parse_starting_unit(4, template, tokens)
}

fn parse_starting_unit5(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    parse_starting_unit(5, template, tokens)
}

fn parse_starting_unit6(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    parse_starting_unit(6, template, tokens)
}

fn parse_starting_unit7(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    parse_starting_unit(7, template, tokens)
}

fn parse_starting_unit8(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    parse_starting_unit(8, template, tokens)
}

fn parse_starting_unit9(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    parse_starting_unit(9, template, tokens)
}

fn parse_intrinsic_sciences(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.intrinsic_sciences = parse_science_list(tokens);
    Ok(())
}

fn parse_purchase_science_rank1(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.purchase_science_command_set_rank1 = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_purchase_science_rank3(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.purchase_science_command_set_rank3 = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_purchase_science_rank8(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.purchase_science_command_set_rank8 = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_special_power_shortcut_command_set(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.special_power_shortcut_command_set = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_special_power_shortcut_win_name(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.special_power_shortcut_win_name = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_special_power_shortcut_button_count(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.special_power_shortcut_button_count = parse_int(tokens)?;
    Ok(())
}

fn parse_is_observer(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.is_observer = parse_bool(tokens)?;
    Ok(())
}

fn parse_old_faction(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.old_faction = parse_bool(tokens)?;
    Ok(())
}

fn parse_intrinsic_spp(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.intrinsic_science_purchase_points = parse_int(tokens)?;
    Ok(())
}

fn parse_score_screen_image(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.score_screen_image = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_load_screen_image(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.load_screen_image = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_load_screen_music(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.load_screen_music = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_score_screen_music(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.score_screen_music = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_head_water_mark(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.head_water_mark = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_flag_water_mark(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.flag_water_mark = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_enabled_image(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.enabled_image = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_side_icon_image(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.side_icon_image = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_general_image(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.general_image = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_beacon_name(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.beacon_name = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_army_tooltip(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.army_tooltip = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_features(_ini: &mut INI, template: &mut PlayerTemplate, tokens: &[&str]) -> INIResult<()> {
    template.features = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_medallion_regular(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.medallion_regular = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_medallion_hilite(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.medallion_hilite = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_medallion_select(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    template.medallion_select = parse_ascii_string(tokens)?;
    Ok(())
}

fn parse_production_cost_change(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    let name = tokens.get(0).ok_or(INIError::InvalidData)?;
    let percent = tokens.get(1).ok_or(INIError::InvalidData)?;
    let change = INI::parse_percent_to_real(percent)?;
    let key = NameKeyGenerator::name_to_key(name);
    template.production_cost_changes.insert(key, change);
    Ok(())
}

fn parse_production_time_change(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    let name = tokens.get(0).ok_or(INIError::InvalidData)?;
    let percent = tokens.get(1).ok_or(INIError::InvalidData)?;
    let change = INI::parse_percent_to_real(percent)?;
    let key = NameKeyGenerator::name_to_key(name);
    template.production_time_changes.insert(key, change);
    Ok(())
}

fn parse_production_veterancy_level(
    _ini: &mut INI,
    template: &mut PlayerTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    let name = tokens.get(0).ok_or(INIError::InvalidData)?;
    let level = tokens.get(1).ok_or(INIError::InvalidData)?;
    let key = NameKeyGenerator::name_to_key(name);
    let vet = parse_veterancy_level(level)?;
    template.production_veterancy_levels.insert(key, vet);
    Ok(())
}

const PLAYER_TEMPLATE_FIELDS: &[FieldParse<PlayerTemplate>] = &[
    FieldParse {
        token: "Side",
        parse: parse_side,
    },
    FieldParse {
        token: "BaseSide",
        parse: parse_base_side,
    },
    FieldParse {
        token: "PlayableSide",
        parse: parse_playable_side,
    },
    FieldParse {
        token: "DisplayName",
        parse: parse_display_name_field,
    },
    FieldParse {
        token: "StartMoney",
        parse: parse_start_money,
    },
    FieldParse {
        token: "PreferredColor",
        parse: parse_preferred_color,
    },
    FieldParse {
        token: "StartingBuilding",
        parse: parse_starting_building,
    },
    FieldParse {
        token: "StartingUnit0",
        parse: parse_starting_unit0,
    },
    FieldParse {
        token: "StartingUnit1",
        parse: parse_starting_unit1,
    },
    FieldParse {
        token: "StartingUnit2",
        parse: parse_starting_unit2,
    },
    FieldParse {
        token: "StartingUnit3",
        parse: parse_starting_unit3,
    },
    FieldParse {
        token: "StartingUnit4",
        parse: parse_starting_unit4,
    },
    FieldParse {
        token: "StartingUnit5",
        parse: parse_starting_unit5,
    },
    FieldParse {
        token: "StartingUnit6",
        parse: parse_starting_unit6,
    },
    FieldParse {
        token: "StartingUnit7",
        parse: parse_starting_unit7,
    },
    FieldParse {
        token: "StartingUnit8",
        parse: parse_starting_unit8,
    },
    FieldParse {
        token: "StartingUnit9",
        parse: parse_starting_unit9,
    },
    FieldParse {
        token: "ProductionCostChange",
        parse: parse_production_cost_change,
    },
    FieldParse {
        token: "ProductionTimeChange",
        parse: parse_production_time_change,
    },
    FieldParse {
        token: "ProductionVeterancyLevel",
        parse: parse_production_veterancy_level,
    },
    FieldParse {
        token: "IntrinsicSciences",
        parse: parse_intrinsic_sciences,
    },
    FieldParse {
        token: "PurchaseScienceCommandSetRank1",
        parse: parse_purchase_science_rank1,
    },
    FieldParse {
        token: "PurchaseScienceCommandSetRank3",
        parse: parse_purchase_science_rank3,
    },
    FieldParse {
        token: "PurchaseScienceCommandSetRank8",
        parse: parse_purchase_science_rank8,
    },
    FieldParse {
        token: "SpecialPowerShortcutCommandSet",
        parse: parse_special_power_shortcut_command_set,
    },
    FieldParse {
        token: "SpecialPowerShortcutWinName",
        parse: parse_special_power_shortcut_win_name,
    },
    FieldParse {
        token: "SpecialPowerShortcutButtonCount",
        parse: parse_special_power_shortcut_button_count,
    },
    FieldParse {
        token: "IsObserver",
        parse: parse_is_observer,
    },
    FieldParse {
        token: "OldFaction",
        parse: parse_old_faction,
    },
    FieldParse {
        token: "IntrinsicSciencePurchasePoints",
        parse: parse_intrinsic_spp,
    },
    FieldParse {
        token: "ScoreScreenImage",
        parse: parse_score_screen_image,
    },
    FieldParse {
        token: "LoadScreenImage",
        parse: parse_load_screen_image,
    },
    FieldParse {
        token: "LoadScreenMusic",
        parse: parse_load_screen_music,
    },
    FieldParse {
        token: "ScoreScreenMusic",
        parse: parse_score_screen_music,
    },
    FieldParse {
        token: "HeadWaterMark",
        parse: parse_head_water_mark,
    },
    FieldParse {
        token: "FlagWaterMark",
        parse: parse_flag_water_mark,
    },
    FieldParse {
        token: "EnabledImage",
        parse: parse_enabled_image,
    },
    FieldParse {
        token: "SideIconImage",
        parse: parse_side_icon_image,
    },
    FieldParse {
        token: "GeneralImage",
        parse: parse_general_image,
    },
    FieldParse {
        token: "BeaconName",
        parse: parse_beacon_name,
    },
    FieldParse {
        token: "ArmyTooltip",
        parse: parse_army_tooltip,
    },
    FieldParse {
        token: "Features",
        parse: parse_features,
    },
    FieldParse {
        token: "MedallionRegular",
        parse: parse_medallion_regular,
    },
    FieldParse {
        token: "MedallionHilite",
        parse: parse_medallion_hilite,
    },
    FieldParse {
        token: "MedallionSelect",
        parse: parse_medallion_select,
    },
];

pub fn parse_player_template_definition(ini: &mut INI) -> INIResult<()> {
    let tokens = ini.get_line_tokens();
    let name = tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .ok_or(INIError::InvalidData)?;

    let mut store = get_player_template_store_mut();
    let template_index = store.find_template_index(name).unwrap_or_else(|| {
        store.add_template(PlayerTemplate::new((*name).to_string()));
        store.len().saturating_sub(1)
    });

    if let Some(template) = store.get_nth_player_template_mut(template_index) {
        ini.init_from_ini_with_fields(template, PLAYER_TEMPLATE_FIELDS)?;
        Ok(())
    } else {
        Err(INIError::InvalidData)
    }
}
