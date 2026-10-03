use super::*;
use crate::common::Snapshot;
use game_engine::common::ini::{
    AIData as IniAIData, AiSideBuildList as IniBuildList, BuildListEntry,
};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

#[test]
fn constructor_defaults_match_cpp_tai_data() {
    // AI.cpp:875-924. Authored omission must retain constructor values at 30 logic Hz.
    let data = AiData::default();
    assert_eq!(data.guard_enemy_scan_rate, 15);
    assert_eq!(data.guard_enemy_return_scan_rate, 30);
    assert_eq!(data.skirmish_base_defense_extra_distance, 0.0);
    assert_eq!(data.rebuild_delay_seconds, 10);
    let ini = IniAIData::default();
    assert_eq!(ini.guard_enemy_scan_rate, 15);
    assert_eq!(ini.guard_enemy_return_scan_rate, 30);
}

#[test]
fn explicit_authored_install_and_replacement_are_instance_owned() {
    let first_rules = IniAIData {
        resources_wealthy: 1200,
        guard_enemy_scan_rate: 9,
        ..Default::default()
    };
    let second_rules = IniAIData {
        resources_wealthy: 8000,
        guard_enemy_scan_rate: 21,
        ..Default::default()
    };
    let mut first = crate::ai::AI::new();
    first.init_from_authored_data(&first_rules);
    let mut second = crate::ai::AI::new();
    assert_eq!(
        first.get_ai_data().resources_wealthy,
        1200,
        "constructing another AI is inert"
    );
    second.init_from_authored_data(&second_rules);
    first.init_from_authored_data(&IniAIData {
        resources_wealthy: 4500,
        ..first_rules.clone()
    });
    assert_eq!(first.get_ai_data().resources_wealthy, 4500);
    assert_eq!(second.get_ai_data().resources_wealthy, 8000);
    assert_eq!(second.get_ai_data().guard_enemy_scan_rate, 21);
    assert_eq!(
        first_rules.resources_wealthy, 1200,
        "authored input is unchanged"
    );
}

#[test]
fn conversion_preserves_build_list_order_values_and_negative_rebuild_clamp() {
    let source = IniAIData {
        side_build_lists: vec![IniBuildList {
            side: "China".into(),
            entries: vec![
                BuildListEntry {
                    building_name: "first".into(),
                    template_name: "ChinaPowerPlant".into(),
                    location: (10.0, 20.0),
                    rebuilds: -1,
                    ..Default::default()
                },
                BuildListEntry {
                    building_name: "second".into(),
                    template_name: "ChinaBarracks".into(),
                    ..Default::default()
                },
            ],
        }],
        ..Default::default()
    };
    let result = convert_ai_data(&source);
    let first = result.side_build_lists[0].build_list.as_ref().unwrap();
    assert_eq!(first.get_building_name().as_str(), "first");
    assert_eq!(first.get_location().x, 10.0);
    assert_eq!(first.get_num_rebuilds(), 0);
    assert_eq!(
        first.get_next().unwrap().get_building_name().as_str(),
        "second"
    );
}

#[test]
fn authored_data_xfer_remains_version_only_and_load_keeps_definition() {
    // TAiData::xfer (AI.cpp:960) serializes version 1 only.
    let mut first = AiData {
        resources_wealthy: 1200,
        ..Default::default()
    };
    let mut bytes = Vec::new();
    first.xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1));
    assert_eq!(bytes, [1]);
    let mut second = AiData {
        resources_wealthy: 8000,
        ..Default::default()
    };
    second.xfer(&mut XferLoad::new(Cursor::new(bytes), 1));
    assert_eq!(second.resources_wealthy, 8000);
    assert_eq!(first.resources_wealthy, 1200);
}

#[test]
fn crc_matches_cpp_field_order_and_excludes_wall_height() {
    // TAiData::crc, AI.cpp:929-955. Four-byte fields precede one byte bool.
    let data = AiData {
        structure_seconds: 1.0,
        team_seconds: 2.0,
        resources_wealthy: 3,
        resources_poor: 4,
        force_idle_frames_count: 5,
        structures_wealthy_mod: 6.0,
        team_wealthy_mod: 7.0,
        structures_poor_mod: 8.0,
        team_poor_mod: 9.0,
        team_resources_to_build: 10.0,
        guard_inner_modifier_ai: 11.0,
        guard_outer_modifier_ai: 12.0,
        guard_inner_modifier_human: 13.0,
        guard_outer_modifier_human: 14.0,
        guard_chase_unit_frames: 15,
        guard_enemy_scan_rate: 16,
        guard_enemy_return_scan_rate: 17,
        alert_range_modifier: 18.0,
        aggressive_range_modifier: 19.0,
        attack_priority_distance_modifier: 20.0,
        max_recruit_distance: 21.0,
        skirmish_base_defense_extra_distance: 22.0,
        repulsed_distance: 23.0,
        enable_repulsors: true,
        wall_height: 999.0,
        ..Default::default()
    };
    let mut actual = Vec::new();
    data.crc(&mut XferSave::new(Cursor::new(&mut actual), 1));
    let mut expected = Vec::new();
    for value in 1u32..=23 {
        let bits = if matches!(value, 3..=5 | 15..=17) {
            value
        } else {
            (value as f32).to_bits()
        };
        expected.extend_from_slice(&bits.to_le_bytes());
    }
    expected.push(1);
    assert_eq!(actual, expected);
}
