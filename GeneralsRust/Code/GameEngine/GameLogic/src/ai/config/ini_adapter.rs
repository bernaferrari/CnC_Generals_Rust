use super::{AiData, AiSideBuildList, AiSideInfo, SkillSet};
use crate::{
    build_list_info::BuildListInfo,
    common::{Coord2D, Coord3D},
};
use game_engine::common::ini::{
    AIData as IniAIData, AiSideBuildList as IniAiSideBuildList, AiSideInfo as IniAiSideInfo,
    BuildListEntry as IniBuildListEntry, SkillSet as IniSkillSet,
};

fn convert_skill_set(src: &IniSkillSet) -> SkillSet {
    SkillSet {
        num_skills: src.num_skills,
        skills: src.skills,
    }
}

fn convert_side_info(src: &IniAiSideInfo) -> AiSideInfo {
    AiSideInfo {
        side: src.side.clone(),
        easy: src.easy,
        normal: src.normal,
        hard: src.hard,
        skill_set_1: convert_skill_set(&src.skill_set_1),
        skill_set_2: convert_skill_set(&src.skill_set_2),
        skill_set_3: convert_skill_set(&src.skill_set_3),
        skill_set_4: convert_skill_set(&src.skill_set_4),
        skill_set_5: convert_skill_set(&src.skill_set_5),
        base_defense_structure_1: src.base_defense_structure_1.clone(),
    }
}

fn convert_build_list_entry(entry: &IniBuildListEntry) -> BuildListInfo {
    let mut info = BuildListInfo::new();
    info.set_building_name(entry.building_name.as_str().into());
    info.set_template_name(entry.template_name.as_str().into());
    info.set_location(Coord3D::new(entry.location.0, entry.location.1, 0.0));
    info.set_rally_offset(Coord2D::new(
        entry.rally_point_offset.0,
        entry.rally_point_offset.1,
    ));
    info.set_angle(entry.angle_radians);
    info.set_initially_built(entry.initially_built);
    info.set_automatic_build(entry.automatically_build);
    if entry.rebuilds < 0 {
        info.set_num_rebuilds(0);
    } else {
        info.set_num_rebuilds(entry.rebuilds as u32);
    }
    info
}

fn convert_build_list(entries: &[IniBuildListEntry]) -> Option<Box<BuildListInfo>> {
    let mut next: Option<Box<BuildListInfo>> = None;
    for entry in entries.iter().rev() {
        let mut info = convert_build_list_entry(entry);
        info.set_next_build_list_boxed(next);
        next = Some(Box::new(info));
    }
    next
}

fn convert_side_build_list(src: &IniAiSideBuildList) -> AiSideBuildList {
    let mut build_list = AiSideBuildList::new(src.side.clone());
    build_list.build_list = convert_build_list(&src.entries);
    build_list
}

pub(in crate::ai) fn convert_ai_data(src: &IniAIData) -> AiData {
    let mut data = AiData::default();
    data.structure_seconds = src.structure_seconds;
    data.team_seconds = src.team_seconds;
    data.resources_wealthy = src.resources_wealthy;
    data.resources_poor = src.resources_poor;
    data.force_idle_frames_count = src.force_idle_frames_count;
    data.structures_wealthy_mod = src.structures_wealthy_mod;
    data.team_wealthy_mod = src.team_wealthy_mod;
    data.structures_poor_mod = src.structures_poor_mod;
    data.team_poor_mod = src.team_poor_mod;
    data.team_resources_to_build = src.team_resources_to_build;
    data.guard_inner_modifier_ai = src.guard_inner_modifier_ai;
    data.guard_outer_modifier_ai = src.guard_outer_modifier_ai;
    data.guard_inner_modifier_human = src.guard_inner_modifier_human;
    data.guard_outer_modifier_human = src.guard_outer_modifier_human;
    data.guard_chase_unit_frames = src.guard_chase_unit_frames;
    data.guard_enemy_scan_rate = src.guard_enemy_scan_rate;
    data.guard_enemy_return_scan_rate = src.guard_enemy_return_scan_rate;
    data.wall_height = src.wall_height;
    data.alert_range_modifier = src.alert_range_modifier;
    data.aggressive_range_modifier = src.aggressive_range_modifier;
    data.attack_priority_distance_modifier = src.attack_priority_distance_modifier;
    data.skirmish_group_fudge_value = src.skirmish_group_fudge_value;
    data.max_recruit_distance = src.max_recruit_distance;
    data.skirmish_base_defense_extra_distance = src.skirmish_base_defense_extra_distance;
    data.repulsed_distance = src.repulsed_distance;
    data.enable_repulsors = src.enable_repulsors;
    data.force_skirmish_ai = src.force_skirmish_ai;
    data.rotate_skirmish_bases = src.rotate_skirmish_bases;
    data.attack_uses_line_of_sight = src.attack_uses_line_of_sight;
    data.attack_ignore_insignificant_buildings = src.attack_ignore_insignificant_buildings;
    data.min_infantry_for_group = src.min_infantry_for_group;
    data.min_vehicles_for_group = src.min_vehicles_for_group;
    data.min_distance_for_group = src.min_distance_for_group;
    data.distance_requires_group = src.distance_requires_group;
    data.min_clump_density = src.min_clump_density;
    data.infantry_pathfind_diameter = src.infantry_pathfind_diameter;
    data.vehicle_pathfind_diameter = src.vehicle_pathfind_diameter;
    data.rebuild_delay_seconds = src.rebuild_delay_seconds;
    data.supply_center_safe_radius = src.supply_center_safe_radius;
    data.ai_dozer_bored_radius_modifier = src.ai_dozer_bored_radius_modifier;
    data.ai_crushes_infantry = src.ai_crushes_infantry;
    data.max_retaliate_distance = src.max_retaliate_distance;
    data.retaliate_friends_radius = src.retaliate_friends_radius;
    data.side_info = src.side_info.iter().map(convert_side_info).collect();
    data.side_build_lists = src
        .side_build_lists
        .iter()
        .map(convert_side_build_list)
        .collect();
    data
}
