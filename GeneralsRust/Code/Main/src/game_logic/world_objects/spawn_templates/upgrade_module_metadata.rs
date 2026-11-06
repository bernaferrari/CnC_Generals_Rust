//! Authored `HordeUpdate`, `ModelConditionUpgrade` and `SubObjectsUpgrade`
//! module data retained on host templates.
//!
//! C++ field parsers: HordeUpdate.cpp `HordeUpdateModuleData::buildFieldParse`,
//! ModelConditionUpgrade.cpp and SubObjectsUpgrade.cpp `buildFieldParse`.
use super::*;
use crate::game_logic::{HordeUpdateRules, ModelConditionUpgradeRule, SubObjectsUpgradeRule};

/// C++ `INI::parseBool`.
fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

/// C++ `INI::parseDurationUnsignedInt`: unsigned millisecond prefix rounded
/// up to 30 Hz logic frames.
fn parse_duration_frames(value: &str) -> Option<u32> {
    let digits: String = value
        .trim_start()
        .bytes()
        .take_while(u8::is_ascii_digit)
        .map(char::from)
        .collect();
    let milliseconds = digits.parse::<u64>().ok()?;
    u32::try_from(milliseconds.checked_mul(30)?.checked_add(999)? / 1_000).ok()
}

/// C++ `KindOfMaskType::parseFromINI` as ThingTemplate KindOf bits.
fn parse_kind_of_mask(value: &str) -> u128 {
    use game_engine::common::system::kind_of::KindOfMask;
    KindOfMask::parse_ini(KindOfMask::empty(), value)
        .map(|mask| mask.bits())
        .unwrap_or(0)
}

fn module_tag(module: &crate::assets::BehaviorModuleDefinition, class: &str, index: usize) -> String {
    module
        .module_tag
        .clone()
        .unwrap_or_else(|| format!("{class}{index}"))
}

/// The first authored `HordeUpdate` (C++ `getHUI` finds the first module
/// exposing `HordeUpdateInterface`).
pub(super) fn authored_horde_update(definition: &ObjectDefinition) -> Option<HordeUpdateRules> {
    let module = definition
        .behavior_modules
        .iter()
        .find(|module| module.class_name.eq_ignore_ascii_case("HordeUpdate"))?;
    let mut rules = HordeUpdateRules::default();
    if let Some(frames) = module.attribute("UpdateRate").and_then(parse_duration_frames) {
        rules.update_rate_frames = frames;
    }
    if let Some(kind_of) = module.attribute("KindOf") {
        rules.kind_of_mask = parse_kind_of_mask(kind_of);
    }
    if let Some(count) = module.attribute("Count").and_then(|v| v.trim().parse().ok()) {
        rules.min_count = count;
    }
    if let Some(radius) = module.attribute("Radius").and_then(|v| v.trim().parse().ok()) {
        rules.min_dist = radius;
    }
    if let Some(radius) = module
        .attribute("RubOffRadius")
        .and_then(|v| v.trim().parse().ok())
    {
        rules.rub_off_radius = radius;
    }
    if let Some(value) = module.attribute("AlliesOnly").and_then(parse_bool) {
        rules.allies_only = value;
    }
    if let Some(value) = module.attribute("ExactMatch").and_then(parse_bool) {
        rules.exact_match = value;
    }
    if let Some(value) = module.attribute("AllowedNationalism").and_then(parse_bool) {
        rules.allowed_nationalism = value;
    }
    Some(rules)
}

fn names(module: &crate::assets::BehaviorModuleDefinition, key: &str) -> Vec<String> {
    module
        .attribute(key)
        .map(|value| value.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

impl GameLogic {
    /// Retain the authored HordeUpdate / ModelConditionUpgrade /
    /// SubObjectsUpgrade module data of `definition`.
    pub(super) fn apply_authored_horde_and_visual_upgrade_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::world_objects::authored_upgrade_modules::upgrade_mux_data;
        template.horde_update = authored_horde_update(definition);
        template.model_condition_upgrades = definition
            .behavior_modules
            .iter()
            .filter(|module| module.class_name.eq_ignore_ascii_case("ModelConditionUpgrade"))
            .enumerate()
            .map(|(index, module)| ModelConditionUpgradeRule {
                tag: module_tag(module, "ModelConditionUpgrade", index),
                mux: upgrade_mux_data(module),
                condition_bit: module.attribute("ConditionFlag").and_then(|flag| {
                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                        flag.trim(),
                    )
                    .and_then(|bit| u32::try_from(bit).ok())
                }),
            })
            .collect();
        template.sub_objects_upgrades = definition
            .behavior_modules
            .iter()
            .filter(|module| module.class_name.eq_ignore_ascii_case("SubObjectsUpgrade"))
            .enumerate()
            .map(|(index, module)| SubObjectsUpgradeRule {
                tag: module_tag(module, "SubObjectsUpgrade", index),
                mux: upgrade_mux_data(module),
                show: names(module, "ShowSubObjects"),
                hide: names(module, "HideSubObjects"),
            })
            .collect();
    }
}
