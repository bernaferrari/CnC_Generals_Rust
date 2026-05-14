use super::ai_player::{AIPlayer, WorkOrder};
use crate::ai::THE_AI;
use crate::build_list_info::BuildListInfo;
use crate::common::coord::*;
use crate::common::coord_ext::Coord2DExt;
use crate::common::xfer::{Xfer, XferExt};
use crate::common::Snapshot;
use crate::common::*;
use crate::helpers::{TheGameLogic, ThePartitionManager, TheTerrainLogic, TheThingFactory};
use crate::object::production::construction::FoundationValidator;
use crate::object::special_power_template::SpecialPowerTemplate;
use crate::object::special_power_types::SpecialPowerType as ObjectSpecialPowerType;
use crate::object::*;
use crate::object_manager::get_object_manager;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::player::{player_list, GameDifficulty, Player};
use crate::player::{PlayerType, ThePlayerList};
use crate::team::{get_team_factory, TeamPrototype};
use crate::terrain::get_terrain_logic;
use crate::waypoint::Waypoint;

use std::sync::{Arc, RwLock, Weak};

const HUGE_DIST: f32 = 100000.0;
const SKIRMISH_CENTER: &str = "SkirmCenter";
const SKIRMISH_FLANK: &str = "SkirmFlank";
const SKIRMISH_BACKDOOR: &str = "SkirmBackdoor";
/// AI player specialized for skirmish matches
pub struct AISkirmishPlayer {
    /// Base AI player functionality
    base: AIPlayer,
    /// Current front base defense index
    cur_front_base_defense: i32,
    /// Current flank base defense index
    cur_flank_base_defense: i32,
    /// Current front left defense angle
    cur_front_left_defense_angle: f32,
    /// Current front right defense angle
    cur_front_right_defense_angle: f32,
    /// Current left flank left defense angle
    cur_left_flank_left_defense_angle: f32,
    /// Current left flank right defense angle
    cur_left_flank_right_defense_angle: f32,
    /// Current right flank left defense angle
    cur_right_flank_left_defense_angle: f32,
    /// Current right flank right defense angle
    cur_right_flank_right_defense_angle: f32,
    /// Frame to check for enemy
    frame_to_check_enemy: u32,
    /// Current enemy player
    current_enemy: Option<Weak<RwLock<Player>>>,
    /// Cached enemy infantry count
    enemy_infantry_count: u32,
    /// Cached enemy vehicle count
    enemy_vehicle_count: u32,
    /// Cached enemy air count
    enemy_air_count: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThreatType {
    #[default]
    None,
    Infantry,
    Vehicle,
    Air,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResponseType {
    #[default]
    Expand,
    CounterAttack,
    DefensiveBuild,
}

#[derive(Clone, Debug, Default)]
pub struct ThreatAssessment {
    pub threat_level: f32,
    pub dominant_type: ThreatType,
    pub recommended_response: ResponseType,
    pub infantry_count: u32,
    pub vehicle_count: u32,
    pub aircraft_count: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EconomyDecision {
    #[default]
    Maintain,
    ConserveResources,
    EmergencyEconomy,
    InvestHeavily,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DominantEnemyType {
    Infantry,
    Vehicle,
    Air,
    Unknown,
}

#[derive(Clone, Debug, Default)]
pub struct FactionBuildPriority {
    pub defense_structure: String,
    pub anti_infantry_unit: String,
    pub anti_vehicle_unit: String,
    pub anti_air_unit: String,
    pub priority_building: String,
}

impl AISkirmishPlayer {
    pub fn new(player_id: u32) -> Self {
        let skirmish_player = Self {
            base: AIPlayer::new(player_id),
            cur_front_base_defense: 0,
            cur_flank_base_defense: 0,
            cur_front_left_defense_angle: 0.0,
            cur_front_right_defense_angle: 0.0,
            cur_left_flank_left_defense_angle: 0.0,
            cur_left_flank_right_defense_angle: 0.0,
            cur_right_flank_left_defense_angle: 0.0,
            cur_right_flank_right_defense_angle: 0.0,
            frame_to_check_enemy: 0,
            current_enemy: None,
            enemy_infantry_count: 0,
            enemy_vehicle_count: 0,
            enemy_air_count: 0,
        };

        // Turn on AI production by default for skirmish
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(player_id as i32) {
                if let Ok(mut player_ref) = player_arc.write() {
                    player_ref.set_can_build_units(true);
                    player_ref.set_is_skirmish_ai(true);
                }
            }
        }

        skirmish_player
    }

    pub fn get_base_center(&self) -> Option<Coord3D> {
        self.base.get_base_center()
    }

    /// Main update function - simulates the behavior of a skirmish player
    pub fn update(&mut self) {
        if let Err(err) = self.base.update_without_base_building() {
            log::debug!("AISkirmishPlayer::update_without_base_building failed: {err}");
        }
        self.do_base_building();
        self.do_team_building();
    }

    /// Called when new map is loaded
    pub fn new_map(&mut self) {
        self.base.new_map();

        // Reset skirmish-specific state
        self.cur_front_base_defense = 0;
        self.cur_flank_base_defense = 0;
        self.frame_to_check_enemy = 0;
        self.current_enemy = None;
        self.enemy_infantry_count = 0;
        self.enemy_vehicle_count = 0;
        self.enemy_air_count = 0;

        let _player_side = {
            let Some(player_arc) = self.base.get_player() else {
                return;
            };
            let Ok(guard) = player_arc.read() else {
                return;
            };
            Some(guard.get_side().clone())
        };
        let Some(player_side) = _player_side else {
            return;
        };

        let mut build_list = None;
        if let Ok(ai_guard) = THE_AI.read() {
            if let Ok(ai_data) = ai_guard.get_ai_data().read() {
                if let Some(entry) = ai_data
                    .side_build_lists
                    .iter()
                    .find(|entry| entry.side == player_side)
                {
                    if let Some(list) = entry.build_list.as_ref() {
                        build_list = Some(list.duplicate());
                    }
                }
            }
        }

        let Some(mut build_list) = build_list else {
            return;
        };

        self.adjust_build_list(&mut build_list);
        if let Some(player_arc) = self.base.get_player() {
            if let Ok(mut guard) = player_arc.write() {
                guard.set_build_list(Some(build_list));
            }
        }

        self.build_initial_structures();
    }

    /// Called when a unit is produced
    pub fn on_unit_produced(&mut self, factory: &Arc<RwLock<Object>>, unit: &Arc<RwLock<Object>>) {
        let (Ok(factory_guard), Ok(unit_guard)) = (factory.read(), unit.read()) else {
            return;
        };
        let _ = self
            .base
            .on_unit_produced(factory_guard.get_id(), unit_guard.get_id());

        // Additional skirmish-specific unit production logic
    }

    /// Build a specific AI team immediately
    pub fn build_specific_ai_team(&mut self, team_proto: &TeamPrototype, priority_build: bool) {
        let _ = self
            .base
            .build_specific_ai_team(team_proto.get_name().as_str(), priority_build);
    }

    /// Build a specific AI team by name.
    pub fn build_specific_ai_team_by_name(&mut self, team_name: &str, priority_build: bool) {
        let _ = self.base.build_specific_ai_team(team_name, priority_build);
    }

    /// Build specific AI building
    pub fn build_specific_ai_building(&mut self, thing_name: &str) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return;
        };

        let mut found = false;
        let mut found_unbuilt = false;
        let mut info_opt = player_guard.get_build_list_mut();
        while let Some(info) = info_opt {
            let name = info.get_template_name();
            if name == thing_name {
                found = true;
                if info.get_object_id() != crate::common::INVALID_ID {
                    info_opt = info.get_next_mut();
                    continue;
                }
                if info.is_priority_build() {
                    info_opt = info.get_next_mut();
                    continue;
                }
                info.mark_priority_build();
                found_unbuilt = true;
                break;
            }
            info_opt = info.get_next_mut();
        }

        if found_unbuilt {
            self.base.set_build_delay_frames(0);
        } else if !found {
            if let Err(err) = self.base.build_specific_ai_building(thing_name) {
                log::debug!(
                    "AISkirmishPlayer::build_specific_ai_building('{}') failed: {err}",
                    thing_name
                );
            }
        }
    }

    /// Build AI base defense with skirmish-specific logic
    pub fn build_ai_base_defense(&mut self, flank: bool) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let player_side = match player_arc.read() {
            Ok(guard) => guard.get_side().clone(),
            Err(_) => return,
        };
        let mut defense_name = None;
        if let Ok(ai_guard) = THE_AI.read() {
            if let Ok(ai_data) = ai_guard.get_ai_data().read() {
                for side_info in &ai_data.side_info {
                    if side_info.side == player_side {
                        if !side_info.base_defense_structure_1.is_empty() {
                            defense_name = Some(side_info.base_defense_structure_1.clone());
                        }
                        break;
                    }
                }
            }
        }
        if let Some(name) = defense_name {
            self.build_ai_base_defense_structure(&name, flank);
            return;
        }
        if flank {
            self.build_flank_defense();
        } else {
            self.build_front_defense();
        }
    }

    /// Build AI base defense structure with positioning
    pub fn build_ai_base_defense_structure(&mut self, thing_name: &str, flank: bool) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let Ok(player_guard) = player_arc.read() else {
            return;
        };
        let Some(template) = TheThingFactory::find_template(thing_name) else {
            return;
        };

        loop {
            let path_label = if flank {
                if self.cur_flank_base_defense & 1 != 0 {
                    format!(
                        "{}{}",
                        SKIRMISH_FLANK,
                        player_guard.get_mp_start_index() + 1
                    )
                } else {
                    format!(
                        "{}{}",
                        SKIRMISH_BACKDOOR,
                        player_guard.get_mp_start_index() + 1
                    )
                }
            } else {
                format!(
                    "{}{}",
                    SKIRMISH_CENTER,
                    player_guard.get_mp_start_index() + 1
                )
            };

            let base_center = self.base.get_base_center().unwrap_or_default();
            let mut goal_pos = base_center;
            if let Some(terrain) = TheTerrainLogic::get() {
                if let Some(way_pos) = terrain.get_closest_waypoint_on_path(&goal_pos, &path_label)
                {
                    goal_pos = way_pos;
                } else if flank {
                    return;
                } else {
                    let enemy_index = self.get_my_enemy_player_index();
                    if enemy_index >= 0 {
                        if let Ok(player_list) = ThePlayerList().read() {
                            if let Some(enemy) = player_list.get_player(enemy_index) {
                                if let Ok(enemy_guard) = enemy.read() {
                                    if let Some(center) = self.get_enemy_base_center(&enemy_guard) {
                                        goal_pos = center;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let mut offset = Coord2D::new(goal_pos.x - base_center.x, goal_pos.y - base_center.y);
            if offset.length() > 0.001 {
                offset = offset.normalized();
            }
            let mut defense_distance = self.base.get_base_radius();
            if let Ok(ai_guard) = THE_AI.read() {
                if let Ok(ai_data) = ai_guard.get_ai_data().read() {
                    defense_distance += ai_data.skirmish_base_defense_extra_distance;
                }
            }
            offset.x *= defense_distance;
            offset.y *= defense_distance;

            let structure_radius = template
                .get_template_geometry_info()
                .get_bounding_circle_radius();
            let base_circumference = 2.0 * std::f32::consts::PI * defense_distance;
            let angle_offset = if base_circumference > 0.001 {
                2.0 * std::f32::consts::PI * (structure_radius * 4.0 / base_circumference)
            } else {
                0.0
            };

            let angle = if flank {
                let selector = self.cur_flank_base_defense >> 1;
                if self.cur_flank_base_defense & 1 != 0 {
                    if selector & 1 != 0 {
                        self.cur_left_flank_right_defense_angle -= angle_offset;
                        self.cur_left_flank_right_defense_angle
                    } else {
                        let result = self.cur_left_flank_left_defense_angle;
                        self.cur_left_flank_left_defense_angle += angle_offset;
                        result
                    }
                } else if selector & 1 != 0 {
                    self.cur_right_flank_right_defense_angle -= angle_offset;
                    self.cur_right_flank_right_defense_angle
                } else {
                    let result = self.cur_right_flank_left_defense_angle;
                    self.cur_right_flank_left_defense_angle += angle_offset;
                    result
                }
            } else if self.cur_front_base_defense & 1 != 0 {
                self.cur_front_right_defense_angle -= angle_offset;
                self.cur_front_right_defense_angle
            } else {
                let result = self.cur_front_left_defense_angle;
                self.cur_front_left_defense_angle += angle_offset;
                result
            };

            if angle > std::f32::consts::PI / 3.0 {
                break;
            }

            let s = angle.sin();
            let c = angle.cos();
            let mut build_pos = base_center;
            build_pos.x += offset.x * c - offset.y * s;
            build_pos.y += offset.y * c + offset.x * s;

            if flank {
                self.cur_flank_base_defense += 1;
            } else {
                self.cur_front_base_defense += 1;
            }

            let validator = FoundationValidator::new_ai();
            if validator
                .validate_placement(
                    &build_pos,
                    thing_name,
                    angle,
                    player_guard.get_id() as ObjectID,
                )
                .is_err()
            {
                continue;
            }

            if let Err(err) = self
                .base
                .build_specific_ai_building_at(thing_name, build_pos)
            {
                log::debug!(
                    "AISkirmishPlayer::build_specific_ai_building_at('{}') failed: {err}",
                    thing_name
                );
            }
            break;
        }
    }

    /// Recruit specific AI team
    pub fn recruit_specific_ai_team(&mut self, team_proto: &TeamPrototype, recruit_radius: f32) {
        let _ = self
            .base
            .recruit_specific_ai_team(team_proto.get_name().as_str(), recruit_radius);
    }

    /// Recruit specific AI team by name.
    pub fn recruit_specific_ai_team_by_name(&mut self, team_name: &str, recruit_radius: f32) {
        let _ = self
            .base
            .recruit_specific_ai_team(team_name, recruit_radius);
    }

    /// Check if this is a skirmish AI
    pub fn is_skirmish_ai(&self) -> bool {
        true
    }

    /// Check bridges for pathfinding
    pub fn check_bridges(&mut self, unit: &Arc<RwLock<Object>>, waypoint: &Waypoint) -> bool {
        let unit_pos = {
            let Ok(unit_guard) = unit.try_read() else {
                return false;
            };
            *unit_guard.get_position()
        };
        let target = waypoint.position;
        let delta = Coord3D::new(
            target.x - unit_pos.x,
            target.y - unit_pos.y,
            target.z - unit_pos.z,
        );
        let dist_sq = delta.x * delta.x + delta.y * delta.y;
        if dist_sq < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
            return false;
        }

        let Ok(terrain_guard) = get_terrain_logic().read() else {
            return false;
        };
        let mut bridge_opt = terrain_guard.get_first_bridge();
        while let Some(bridge) = bridge_opt {
            let bridge_id = bridge.get_bridge_info().bridge_object_id;
            if bridge_id == crate::common::INVALID_ID {
                bridge_opt = bridge.get_next();
                continue;
            }
            let broken = match TheGameLogic::find_object_by_id(bridge_id) {
                Some(obj) => obj
                    .read()
                    .ok()
                    .map(|guard| guard.is_destroyed())
                    .unwrap_or(true),
                None => true,
            };
            if !broken {
                bridge_opt = bridge.get_next();
                continue;
            }

            let dist = dist_sq.sqrt().max(PATHFIND_CELL_SIZE_F);
            let steps = (dist / PATHFIND_CELL_SIZE_F).ceil() as i32;
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let sample = Coord3D::new(
                    unit_pos.x + delta.x * t,
                    unit_pos.y + delta.y * t,
                    unit_pos.z + delta.z * t,
                );
                if bridge.is_point_on_bridge(&sample) {
                    let _ = self.base.repair_structure(bridge_id);
                    return true;
                }
            }

            bridge_opt = bridge.get_next();
        }

        false
    }

    /// Get AI enemy for skirmish
    pub fn get_ai_enemy(&mut self) -> Option<Arc<RwLock<Player>>> {
        let current_frame = TheGameLogic::get_frame();
        if current_frame >= self.frame_to_check_enemy {
            self.frame_to_check_enemy = current_frame + 5 * LOGICFRAMES_PER_SECOND;
            self.acquire_enemy();
        }
        self.current_enemy.as_ref()?.upgrade()
    }

    /// Compute superweapon target with skirmish-specific logic
    pub fn compute_superweapon_target(
        &mut self,
        power: &SpecialPowerTemplate,
        pos: &mut Coord3D,
        _player_index: i32,
        weapon_radius: f32,
    ) -> bool {
        let power_type = power.get_special_power_type();
        if matches!(
            power_type,
            ObjectSpecialPowerType::ClusterMines | ObjectSpecialPowerType::NukeClusterMines
        ) {
            let Some(player_arc) = self.base.get_player() else {
                return false;
            };
            let Ok(player_guard) = player_arc.read() else {
                return false;
            };
            let mode = GameLogicRandomValue(0, 2);
            let path_label = if mode == 1 {
                format!(
                    "{}{}",
                    SKIRMISH_FLANK,
                    player_guard.get_mp_start_index() + 1
                )
            } else if mode == 2 {
                format!(
                    "{}{}",
                    SKIRMISH_BACKDOOR,
                    player_guard.get_mp_start_index() + 1
                )
            } else {
                format!(
                    "{}{}",
                    SKIRMISH_CENTER,
                    player_guard.get_mp_start_index() + 1
                )
            };

            let base_center = self.base.get_base_center().unwrap_or_default();
            let mut goal_pos = base_center;
            if let Some(terrain) = TheTerrainLogic::get() {
                if let Some(way_pos) = terrain.get_closest_waypoint_on_path(&goal_pos, &path_label)
                {
                    goal_pos = way_pos;
                } else {
                    let enemy_index = self.get_my_enemy_player_index();
                    if enemy_index >= 0 {
                        if let Ok(player_list) = ThePlayerList().read() {
                            if let Some(enemy) = player_list.get_player(enemy_index) {
                                if let Ok(enemy_guard) = enemy.read() {
                                    if let Some(center) = self.get_enemy_base_center(&enemy_guard) {
                                        goal_pos = center;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let mut offset = Coord2D::new(goal_pos.x - base_center.x, goal_pos.y - base_center.y);
            if offset.length() > 0.001 {
                offset = offset.normalized();
            }
            let base_radius = self.base.get_base_radius();
            offset.x *= base_radius;
            offset.y *= base_radius;

            *pos = base_center;
            pos.x += offset.x;
            pos.y += offset.y;
            if let Some(terrain) = TheTerrainLogic::get() {
                pos.z = terrain.get_ground_height(pos.x, pos.y, None);
            }
            return true;
        }

        if let Ok(Some(target)) = self
            .base
            .compute_superweapon_target(power.get_name(), weapon_radius)
        {
            *pos = target;
            return true;
        }

        false
    }

    // Private methods

    /// Process base building with skirmish-specific logic
    /// Matches C++ AISkirmishPlayer.cpp:75 processBaseBuilding
    fn process_base_building(&mut self) {
        if !self.base.can_build_structure_now() {
            return;
        }

        if self.is_under_powered() {
            self.prioritize_power_buildings();
        }

        if self.enemy_threat_detected() {
            self.prioritize_defensive_buildings();
        }

        let current_frame = TheGameLogic::get_frame();
        let rebuild_delay_frames = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|data| data.rebuild_delay_seconds)
            })
            .unwrap_or(0) as u32
            * LOGICFRAMES_PER_SECOND;

        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return;
        };
        let player_index = player_guard.get_id() as u32;

        let mut selected_plan = None;
        let mut selected_name = None;
        let mut is_priority = false;
        let mut power_plan = None;
        let mut power_name = None;
        let mut power_under_construction = false;
        let is_under_powered = self.is_under_powered();

        let mut info_opt = player_guard.get_build_list_mut();
        while let Some(info) = info_opt {
            let name = info.get_template_name();
            if name.is_empty() {
                info_opt = info.get_next_mut();
                continue;
            }

            let Some(cur_plan) = TheThingFactory::find_template(name.as_str()) else {
                info_opt = info.get_next_mut();
                continue;
            };

            let obj_id = info.get_object_id();
            if obj_id != crate::common::INVALID_ID {
                if let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) {
                    if let Ok(obj_guard) = obj_arc.read() {
                        if obj_guard.get_controlling_player_id() == Some(player_index) {
                            if obj_guard.is_under_construction()
                                && (cur_plan.is_kind_of(KindOf::FSPower)
                                    || cur_plan.is_kind_of(KindOf::PowerPlant))
                            {
                                power_under_construction = true;
                            }
                            if obj_guard.is_under_construction() {
                                info.set_under_construction(true);
                                let builder_id = obj_guard.get_builder_id();
                                let mut builder_valid = builder_id != crate::common::INVALID_ID;

                                if builder_valid {
                                    if let Some(builder_arc) =
                                        TheGameLogic::find_object_by_id(builder_id)
                                    {
                                        if let Ok(builder_guard) = builder_arc.read() {
                                            let wrong_owner = builder_guard
                                                .get_controlling_player_id()
                                                != Some(player_index);
                                            let unmanned = builder_guard.is_disabled_by_type(
                                                DisabledType::DisabledUnmanned,
                                            );
                                            if wrong_owner || unmanned {
                                                builder_valid = false;
                                            }
                                        } else {
                                            builder_valid = false;
                                        }
                                    } else {
                                        builder_valid = false;
                                    }
                                }

                                if !builder_valid {
                                    drop(obj_guard);
                                    if let Ok(mut obj_write) = obj_arc.write() {
                                        obj_write.set_builder(None);
                                    }
                                    if let Err(err) = self.base.queue_dozer() {
                                        log::debug!(
                                            "AISkirmishPlayer::queue_dozer failed while rebuilding: {err}"
                                        );
                                    }
                                }
                            } else {
                                info.set_under_construction(false);
                            }
                            info_opt = info.get_next_mut();
                            continue;
                        }
                    }
                }
                info.set_object_id(crate::common::INVALID_ID);
                info.set_object_timestamp(current_frame + 1);

                if let Some(partition) = ThePartitionManager::get() {
                    let candidates = partition.get_objects_in_range(info.get_location(), 50.0);
                    for candidate_id in candidates {
                        let Some(candidate_arc) = TheGameLogic::find_object_by_id(candidate_id)
                        else {
                            continue;
                        };
                        let Ok(candidate_guard) = candidate_arc.read() else {
                            continue;
                        };
                        let name = candidate_guard
                            .get_template()
                            .get_name()
                            .as_str()
                            .to_ascii_lowercase();
                        if name.contains("rebuildhole") {
                            info.set_object_id(candidate_id);
                            info.set_under_construction(true);
                            break;
                        }
                    }
                }
            }

            if info.get_object_timestamp() > 0 {
                if info.get_object_timestamp() + rebuild_delay_frames > current_frame {
                    info_opt = info.get_next_mut();
                    continue;
                }
                info.set_object_timestamp(0);
            }

            if !self
                .base
                .is_location_safe(info.get_location(), cur_plan.as_ref())
            {
                info_opt = info.get_next_mut();
                continue;
            }

            if info.is_priority_build() && !is_priority {
                selected_plan = Some(cur_plan.clone());
                selected_name = Some(name.clone());
                is_priority = true;
            }

            let is_power_plan =
                cur_plan.is_kind_of(KindOf::FSPower) || cur_plan.is_kind_of(KindOf::PowerPlant);
            if power_plan.is_none()
                && is_power_plan
                && (is_under_powered || info.is_automatic_build())
            {
                power_plan = Some(cur_plan.clone());
                power_name = Some(name.clone());
            }

            if !info.is_automatic_build() {
                info_opt = info.get_next_mut();
                continue;
            }

            if !info.is_buildable() {
                info_opt = info.get_next_mut();
                continue;
            }

            if selected_plan.is_none() {
                selected_plan = Some(cur_plan);
                selected_name = Some(name);
            }

            info_opt = info.get_next_mut();
        }

        if let Some(power) = power_plan {
            if !power_under_construction {
                if let Some(selected) = selected_plan.as_ref() {
                    if !power.is_equivalent_to(selected.as_ref()) {
                        selected_plan = Some(power);
                        selected_name = power_name;
                        let _ = &selected_plan;
                    }
                } else {
                    selected_plan = Some(power);
                    selected_name = power_name;
                    let _ = &selected_plan;
                }
            }
        }

        if let Some(name) = selected_name {
            if let Err(err) = self.base.build_specific_ai_building(name.as_str()) {
                log::debug!(
                    "AISkirmishPlayer::build_specific_ai_building('{}') failed: {err}",
                    name
                );
            }
        }

        let delay_seconds = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|data| data.structure_seconds as i32)
            })
            .unwrap_or(10);
        self.base.start_structure_timer_seconds(delay_seconds);

        self.adjust_build_timer_for_wealth();
    }

    /// Adjust structure timer based on current wealth.
    /// Matches C++ AISkirmishPlayer::processBaseBuilding (lines 242-246):
    ///   if money < m_resourcesPoor: timer /= m_structuresPoorMod
    ///   if money > m_resourcesWealthy: timer /= m_structuresWealthyMod
    fn adjust_build_timer_for_wealth(&mut self) {
        let current_money = self
            .base
            .get_player()
            .and_then(|p| p.read().ok().map(|g| g.get_money().get_money()));
        let Some(money) = current_money else {
            return;
        };

        let (poor, wealthy, poor_mod, wealthy_mod) = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    (
                        data.resources_poor,
                        data.resources_wealthy,
                        data.structures_poor_mod,
                        data.structures_wealthy_mod,
                    )
                })
            })
            .unwrap_or((
                crate::ai::ai_player::RESOURCES_POOR,
                crate::ai::ai_player::RESOURCES_WEALTHY,
                crate::ai::ai_player::STRUCTURES_POOR_MODIFIER,
                crate::ai::ai_player::STRUCTURES_WEALTHY_MODIFIER,
            ));

        let current_timer = self.base.get_structure_timer();
        if current_timer == 0 {
            return;
        }

        let new_timer = if poor_mod > 0.0 && money < poor {
            (current_timer as f32 / poor_mod) as u32
        } else if wealthy_mod > 0.0 && money > wealthy {
            (current_timer as f32 / wealthy_mod) as u32
        } else {
            current_timer
        };

        if new_timer != current_timer {
            self.base.set_structure_timer_frames(new_timer.max(1));
        }
    }

    /// Process team building with skirmish-specific logic
    /// Matches C++ AISkirmishPlayer::processTeamBuilding
    fn process_team_building(&mut self) {
        if let Some(enemy) = self.get_ai_enemy() {
            self.analyze_enemy_composition(&enemy);
            self.build_counter_units();
        }

        if self.select_team_to_build() {
            self.base.queue_units();
        }

        self.adjust_team_timer_for_wealth();
    }

    /// Adjust team timer based on current wealth.
    /// Matches C++ AIPlayer::doTeamBuilding wealth adjustment:
    ///   if money < m_resourcesPoor: timer /= m_teamsPoorMod
    ///   if money > m_resourcesWealthy: timer /= m_teamsWealthyMod
    fn adjust_team_timer_for_wealth(&mut self) {
        let current_money = self
            .base
            .get_player()
            .and_then(|p| p.read().ok().map(|g| g.get_money().get_money()));
        let Some(money) = current_money else {
            return;
        };

        let (poor, wealthy, poor_mod, wealthy_mod) = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    (
                        data.resources_poor,
                        data.resources_wealthy,
                        data.team_poor_mod,
                        data.team_wealthy_mod,
                    )
                })
            })
            .unwrap_or((
                crate::ai::ai_player::RESOURCES_POOR,
                crate::ai::ai_player::RESOURCES_WEALTHY,
                crate::ai::ai_player::TEAMS_POOR_MODIFIER,
                crate::ai::ai_player::TEAMS_WEALTHY_MODIFIER,
            ));

        let current_timer = self.base.get_team_timer();
        if current_timer == 0 {
            return;
        }

        let new_timer = if poor_mod > 0.0 && money < poor {
            (current_timer as f32 / poor_mod) as u32
        } else if wealthy_mod > 0.0 && money > wealthy {
            (current_timer as f32 / wealthy_mod) as u32
        } else {
            current_timer
        };

        if new_timer != current_timer {
            self.base.set_team_timer_frames(new_timer.max(1));
        }
    }

    fn clamp_build_timers(&mut self) {
        let max_timer = 3 * LOGICFRAMES_PER_SECOND;
        if self.base.get_structure_timer() > max_timer {
            self.base.set_structure_timer_frames(max_timer);
        }
        if self.base.get_team_timer() > max_timer {
            self.base.set_team_timer_frames(max_timer);
        }
    }

    fn do_base_building(&mut self) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let Ok(player_guard) = player_arc.read() else {
            return;
        };
        if !player_guard.get_can_build_base() {
            return;
        }

        self.clamp_build_timers();

        if self.base.can_build_structure_now() {
            self.process_base_building();
        }

        if self.base.get_build_delay() == 0 {
            self.base.set_build_delay_frames(2 * LOGICFRAMES_PER_SECOND);
        }
    }

    fn do_team_building(&mut self) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let Ok(player_guard) = player_arc.read() else {
            return;
        };
        if !player_guard.get_can_build_units() {
            return;
        }

        self.clamp_build_timers();

        if self.base.get_team_delay() == 0 {
            self.base.queue_units();
            if self.base.can_build_team_now() {
                self.process_team_building();
            }
            self.base.set_team_delay_frames(2 * LOGICFRAMES_PER_SECOND);
        }
    }

    /// Select team to build with skirmish considerations
    fn select_team_to_build(&mut self) -> bool {
        let Ok(factory) = get_team_factory().lock() else {
            return false;
        };

        let mut candidates: Vec<Arc<TeamPrototype>> = Vec::new();
        let mut hi_priority = i32::MIN;
        for proto in factory.list_team_prototypes() {
            if !self.is_a_good_idea_to_build_team(&proto) {
                continue;
            }
            let priority = proto.get_production_priority();
            if priority > hi_priority {
                hi_priority = priority;
                candidates.clear();
                candidates.push(proto);
            } else if priority == hi_priority {
                candidates.push(proto);
            }
        }

        if hi_priority == i32::MIN {
            return false;
        }

        let enemy_total =
            self.enemy_infantry_count + self.enemy_vehicle_count + self.enemy_air_count;
        let selected_proto = if candidates.len() == 1 || enemy_total == 0 {
            let idx = if candidates.len() == 1 {
                0
            } else {
                GameLogicRandomValue(0, candidates.len() as i32 - 1) as usize
            };
            candidates[idx].clone()
        } else {
            let mut best_score = i32::MIN;
            let mut best_candidates: Vec<Arc<TeamPrototype>> = Vec::new();
            for proto in &candidates {
                let score = self.score_team_for_enemy(proto);
                if score > best_score {
                    best_score = score;
                    best_candidates.clear();
                    best_candidates.push(proto.clone());
                } else if score == best_score {
                    best_candidates.push(proto.clone());
                }
            }
            let idx = if best_candidates.len() == 1 {
                0
            } else {
                GameLogicRandomValue(0, best_candidates.len() as i32 - 1) as usize
            };
            best_candidates[idx].clone()
        };

        self.build_specific_ai_team(selected_proto.as_ref(), true);
        true
    }

    /// Select team to reinforce
    fn select_team_to_reinforce(&mut self, _min_priority: i32) -> bool {
        self.base.select_team_to_build_ai()
    }

    /// Start training with factory management
    fn start_training(&mut self, order: &mut WorkOrder, busy_ok: bool, _team_name: &str) -> bool {
        self.base.start_training_for_order(order, busy_ok)
    }

    /// Check if it's a good idea to build a team
    fn is_a_good_idea_to_build_team(&self, proto: &TeamPrototype) -> bool {
        if !proto.is_ai_recruitable() {
            return false;
        }

        let max_instances = proto.get_max_instances();
        if max_instances > 0 {
            if let Ok(factory_guard) = crate::team::get_team_factory().lock() {
                let name = proto.get_name().as_str();
                let existing = factory_guard.find_team_instances(name).len() as i32;
                if existing >= max_instances {
                    return false;
                }
            }
        }

        if !self.base.can_build_team_now() {
            return false;
        }

        let proto_name = proto.get_name().as_str();
        if self.base.is_team_in_queue(proto_name) {
            return false;
        }

        true
    }

    fn score_team_for_enemy(&self, proto: &TeamPrototype) -> i32 {
        let mut infantry = 0;
        let mut vehicles = 0;
        let mut air = 0;

        for info in proto.units_info() {
            if info.unit_thing_name.is_empty() {
                continue;
            }
            if let Some(template) = TheThingFactory::find_template(info.unit_thing_name) {
                if template.is_kind_of(KindOf::Aircraft) {
                    air += 1;
                } else if template.is_kind_of(KindOf::Vehicle) {
                    vehicles += 1;
                } else if template.is_kind_of(KindOf::Infantry) {
                    infantry += 1;
                }
            }
        }

        let mut score = proto.get_production_priority();
        if self.enemy_air_count > 0 && air > 0 {
            score += 40;
        }
        if self.enemy_vehicle_count > self.enemy_infantry_count && vehicles > 0 {
            score += 20;
        }
        if self.enemy_infantry_count > self.enemy_vehicle_count && infantry > 0 {
            score += 20;
        }
        if self.enemy_air_count == 0 && air > 0 {
            score -= 10;
        }

        score
    }

    fn apply_expansion_ring(&mut self, list: &mut BuildListInfo) {
        let base_center = match self.base.get_base_center() {
            Some(center) => center,
            None => return,
        };
        let mut base_radius = self.base.get_base_radius();
        if base_radius <= 0.1 {
            base_radius = 300.0;
        }

        let extra = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|data| data.skirmish_base_defense_extra_distance)
            })
            .unwrap_or(0.0);

        let Some(terrain) = TheTerrainLogic::get() else {
            return;
        };

        let mut cur_ptr: *mut BuildListInfo = list as *mut BuildListInfo;
        while !cur_ptr.is_null() {
            let cur = unsafe { &mut *cur_ptr };
            if Self::is_expansion_entry(cur) {
                let mut pos = *cur.get_location();
                let dx = pos.x - base_center.x;
                let dy = pos.y - base_center.y;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.1 {
                    let target_len = base_radius + extra;
                    let scale = target_len / len;
                    pos.x = base_center.x + dx * scale;
                    pos.y = base_center.y + dy * scale;
                    pos.z = terrain.get_ground_height(pos.x, pos.y, None);
                    cur.set_location(pos);
                }
            }

            cur_ptr = cur
                .get_next_mut()
                .map(|next| next as *mut BuildListInfo)
                .unwrap_or(std::ptr::null_mut());
        }

        // Expansion ring has been applied to all entries. Do NOT call
        // self.apply_expansion_ring() again — the original C++ adjustBuildList
        // does not apply an expansion ring, and the recursive call was a bug.
    }

    fn is_expansion_entry(info: &BuildListInfo) -> bool {
        let name = info.get_building_name().as_str().to_ascii_uppercase();
        if name.contains("EXPANSION") || name.contains("EXPAND") {
            return true;
        }
        let script = info.get_script().as_str().to_ascii_uppercase();
        script.contains("EXPANSION") || script.contains("EXPAND")
    }

    /// Adjust build list based on skirmish conditions
    /// Matches C++ AISkirmishPlayer::adjustBuildList
    fn adjust_build_list(&mut self, list: &mut BuildListInfo) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let player_index = match player_arc.read() {
            Ok(guard) => guard.get_player_index() as UnsignedInt,
            Err(_) => return,
        };

        let obj_manager = get_object_manager();
        let object_ids = {
            let Ok(manager) = obj_manager.read() else {
                return;
            };
            manager.get_objects_owned_by_player(player_index)
        };

        let mut start_pos = None;
        let mut command_center_id = None;
        {
            let Ok(manager) = obj_manager.read() else {
                return;
            };
            for obj_id in object_ids {
                let Some(obj_arc) = manager.get_object(obj_id) else {
                    continue;
                };
                let Ok(obj_instance) = obj_arc.read() else {
                    continue;
                };
                let Ok(base_obj) = obj_instance.base.read() else {
                    continue;
                };
                if base_obj.is_kind_of(KindOf::CommandCenter) {
                    start_pos = Some(*base_obj.get_position());
                    command_center_id = Some(obj_id);
                    break;
                }
            }
        }

        let Some(start_pos) = start_pos else {
            return;
        };

        if let Some(obj_id) = command_center_id {
            if let Ok(mut manager) = obj_manager.write() {
                manager.destroy_object(obj_id);
            }
        }

        let mut build_pos = Coord3D::origin();
        let mut found_in_build_list = false;
        let mut cur_ptr: *mut BuildListInfo = list as *mut BuildListInfo;
        while !cur_ptr.is_null() {
            let cur = unsafe { &mut *cur_ptr };
            let template_name = cur.get_template_name();
            if let Some(template) = TheThingFactory::find_template(template_name.as_str()) {
                if template.is_kind_of(KindOf::CommandCenter) {
                    found_in_build_list = true;
                    build_pos = *cur.get_location();
                    cur.set_initially_built(true);
                }
            }
            cur_ptr = cur
                .get_next_mut()
                .map(|next| next as *mut BuildListInfo)
                .unwrap_or(std::ptr::null_mut());
        }

        if !found_in_build_list {
            return;
        }

        let Some(terrain) = TheTerrainLogic::get() else {
            return;
        };
        let bounds = terrain.get_maximum_pathfind_extent();
        let width = bounds.hi.x - bounds.lo.x;
        let height = bounds.hi.y - bounds.lo.y;

        let mut grid_index = 0;
        if start_pos.x > bounds.lo.x + width / 3.0 {
            grid_index += 1;
        }
        if start_pos.x > bounds.lo.x + 2.0 * width / 3.0 {
            grid_index += 1;
        }
        if start_pos.y > bounds.lo.y + height / 3.0 {
            grid_index += 3;
        }
        if start_pos.y > bounds.lo.y + 2.0 * height / 3.0 {
            grid_index += 3;
        }

        let mut angle = 0.0f32;
        if let Ok(ai_guard) = THE_AI.read() {
            if let Ok(ai_data) = ai_guard.get_ai_data().read() {
                if ai_data.rotate_skirmish_bases {
                    angle = match grid_index {
                        0 => 0.0,
                        1 => std::f32::consts::PI / 4.0,
                        2 => std::f32::consts::PI / 2.0,
                        3 => -std::f32::consts::PI / 4.0,
                        4 => 0.0,
                        5 => 3.0 * std::f32::consts::PI / 4.0,
                        6 => -std::f32::consts::PI / 2.0,
                        7 => -3.0 * std::f32::consts::PI / 4.0,
                        _ => std::f32::consts::PI,
                    };
                }
            }
        }

        angle += 3.0 * std::f32::consts::PI / 4.0;
        let s = angle.sin();
        let c = angle.cos();

        let list_template_name = list.get_template_name();
        let rotate_all = TheThingFactory::find_template(list_template_name.as_str())
            .map(|template| template.is_kind_of(KindOf::CommandCenter))
            .unwrap_or(false);
        if !rotate_all {
            return;
        }

        let mut cur_ptr: *mut BuildListInfo = list as *mut BuildListInfo;
        while !cur_ptr.is_null() {
            let cur = unsafe { &mut *cur_ptr };
            let mut cur_pos = *cur.get_location();
            cur_pos.x -= build_pos.x;
            cur_pos.y -= build_pos.y;
            let new_x = cur_pos.x * c - cur_pos.y * s;
            let new_y = cur_pos.y * c + cur_pos.x * s;
            cur_pos.x = new_x + start_pos.x;
            cur_pos.y = new_y + start_pos.y;
            cur.set_location(cur_pos);
            cur.set_angle(cur.get_angle());

            cur_ptr = cur
                .get_next_mut()
                .map(|next| next as *mut BuildListInfo)
                .unwrap_or(std::ptr::null_mut());
        }

        self.apply_expansion_ring(list);
    }

    fn build_initial_structures(&mut self) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return;
        };
        let Some(mut entry) = player_guard.get_build_list_mut() else {
            return;
        };

        loop {
            let name = entry.get_template_name();
            if !name.as_str().is_empty() {
                if TheThingFactory::find_template(name.as_str()).is_some() {
                    if entry.is_initially_built() {
                        if let Err(err) = self.base.build_specific_ai_building(name.as_str()) {
                            log::debug!(
                                "AISkirmishPlayer::build_initial_structures('{}') failed: {err}",
                                name
                            );
                        }
                    } else {
                        entry.increment_num_rebuilds();
                    }
                }
            }

            let Some(next) = entry.get_next_mut() else {
                break;
            };
            entry = next;
        }
    }

    /// Get enemy player index
    fn get_my_enemy_player_index(&mut self) -> i32 {
        if let Some(enemy) = self.get_ai_enemy() {
            if let Ok(enemy_ref) = enemy.try_read() {
                return enemy_ref.get_player_index();
            }
        }

        let Ok(player_list) = ThePlayerList().read() else {
            return -1;
        };

        for player in player_list.iter() {
            if let Ok(player_guard) = player.read() {
                if player_guard.get_player_type() == PlayerType::Human {
                    return player_guard.get_player_index();
                }
            }
        }

        player_list.get_player_count() as i32
    }

    /// Acquire enemy player for targeting
    /// Matches C++ AISkirmishPlayer.cpp:461 acquireEnemy
    fn acquire_enemy(&mut self) {
        let mut best_enemy: Option<Arc<RwLock<Player>>> = None;
        let mut best_distance_sqr = HUGE_DIST * HUGE_DIST;

        let Some(me_player) = self.base.get_player() else {
            self.current_enemy = None;
            return;
        };
        let mut me_guard = match me_player.write() {
            Ok(guard) => guard,
            Err(_) => {
                self.current_enemy = None;
                return;
            }
        };
        let me_index = me_guard.get_player_index();
        let base_center = if let Some(center) = self.base.get_base_center() {
            center
        } else {
            self.get_enemy_base_center(&me_guard).unwrap_or_default()
        };

        if let Some(enemy_weak) = self.current_enemy.as_ref() {
            if let Some(enemy_arc) = enemy_weak.upgrade() {
                if let Ok(enemy_guard) = enemy_arc.try_read() {
                    let in_bad_shape =
                        !enemy_guard.has_any_units() || !enemy_guard.has_any_build_facility();
                    if !in_bad_shape {
                        return;
                    }
                }
            }
        }

        let Ok(player_list) = ThePlayerList().read() else {
            self.current_enemy = None;
            return;
        };

        for player_arc in player_list.iter() {
            let Ok(player_guard) = player_arc.read() else {
                continue;
            };

            let Some(team_arc) = player_guard.get_default_team() else {
                continue;
            };
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };
            if me_guard.get_relationship_with_team(&team_guard) != Relationship::Enemies {
                continue;
            }

            if !self.player_has_any_objects(&player_guard) {
                continue;
            }

            let in_bad_shape =
                !player_guard.has_any_units() || !player_guard.has_any_build_facility();

            let enemy_center = self
                .get_enemy_base_center(&player_guard)
                .unwrap_or(base_center);
            let dx = enemy_center.x - base_center.x;
            let dy = enemy_center.y - base_center.y;
            let mut dist_sqr = dx * dx + dy * dy;

            if in_bad_shape {
                dist_sqr = HUGE_DIST * HUGE_DIST * 0.5;
            }

            for other_arc in player_list.iter() {
                let Ok(other_guard) = other_arc.read() else {
                    continue;
                };
                if !other_guard.is_skirmish_ai() {
                    continue;
                }
                if other_guard.get_player_index() == player_guard.get_player_index() {
                    continue;
                }
                if other_guard.get_current_enemy_player_index()
                    == Some(player_guard.get_player_index())
                {
                    dist_sqr += 500.0 * 500.0;
                }
                if other_guard.get_current_enemy_player_index() == Some(me_index) {
                    dist_sqr -= 25.0 * 25.0;
                    if dist_sqr < 0.0 {
                        dist_sqr = 0.0;
                    }
                }
            }

            if dist_sqr < best_distance_sqr {
                best_distance_sqr = dist_sqr;
                best_enemy = Some(player_arc.clone());
            }
        }

        self.current_enemy = best_enemy.map(|enemy| Arc::downgrade(&enemy));
        if let Some(enemy_arc) = self.current_enemy.as_ref().and_then(|weak| weak.upgrade()) {
            if let Ok(enemy_guard) = enemy_arc.read() {
                me_guard.set_current_enemy_player_index(Some(enemy_guard.get_player_index()));
            }
        } else {
            me_guard.set_current_enemy_player_index(None);
        }
    }

    /// Build flank defense.
    /// Matches C++ AISkirmishPlayer::buildAIBaseDefense(flank=true)
    /// which delegates to buildAIBaseDefenseStructure with the
    /// side-specific defense structure name.
    fn build_flank_defense(&mut self) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let player_side = match player_arc.read() {
            Ok(guard) => guard.get_side().clone(),
            Err(_) => return,
        };

        let defense_name = if let Ok(ai_guard) = THE_AI.read() {
            if let Ok(ai_data) = ai_guard.get_ai_data().read() {
                ai_data
                    .side_info
                    .iter()
                    .find(|info| info.side == player_side)
                    .filter(|info| !info.base_defense_structure_1.is_empty())
                    .map(|info| info.base_defense_structure_1.clone())
            } else {
                None
            }
        } else {
            None
        };

        self.cur_flank_base_defense += 1;
        self.update_flank_defense_angles();

        if let Some(name) = defense_name {
            self.build_ai_base_defense_structure(&name, true);
        }
    }

    /// Build front defense.
    /// Matches C++ AISkirmishPlayer::buildAIBaseDefense(flank=false)
    /// which places defenses along the center approach path to the base.
    fn build_front_defense(&mut self) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let player_side = match player_arc.read() {
            Ok(guard) => guard.get_side().clone(),
            Err(_) => return,
        };

        let defense_name = if let Ok(ai_guard) = THE_AI.read() {
            if let Ok(ai_data) = ai_guard.get_ai_data().read() {
                ai_data
                    .side_info
                    .iter()
                    .find(|info| info.side == player_side)
                    .filter(|info| !info.base_defense_structure_1.is_empty())
                    .map(|info| info.base_defense_structure_1.clone())
            } else {
                None
            }
        } else {
            None
        };

        self.cur_front_base_defense += 1;
        self.update_front_defense_angles();

        if let Some(name) = defense_name {
            self.build_ai_base_defense_structure(&name, false);
        }
    }

    /// Update flank defense angles
    fn update_flank_defense_angles(&mut self) {
        // Implementation would calculate optimal angles for flank defenses
        // based on base geometry and enemy approach vectors

        if let Some(base_center) = self.base.get_base_center() {
            // Calculate angles relative to base center and enemy positions
            if let Some(enemy) = self.get_ai_enemy() {
                if let Ok(enemy_player) = enemy.try_read() {
                    if let Some(enemy_base) = self.get_enemy_base_center(&enemy_player) {
                        let to_enemy = enemy_base - base_center;
                        let enemy_angle = to_enemy.y.atan2(to_enemy.x);

                        // Set flank angles perpendicular to enemy direction
                        self.cur_left_flank_left_defense_angle =
                            enemy_angle + std::f32::consts::PI / 2.0;
                        self.cur_left_flank_right_defense_angle =
                            enemy_angle + std::f32::consts::PI / 4.0;
                        self.cur_right_flank_left_defense_angle =
                            enemy_angle - std::f32::consts::PI / 4.0;
                        self.cur_right_flank_right_defense_angle =
                            enemy_angle - std::f32::consts::PI / 2.0;
                    }
                }
            }
        }
    }

    /// Update front defense angles
    fn update_front_defense_angles(&mut self) {
        // Implementation would calculate optimal angles for front defenses
        // based on base geometry and enemy approach vectors

        if let Some(base_center) = self.base.get_base_center() {
            if let Some(enemy) = self.get_ai_enemy() {
                if let Ok(enemy_player) = enemy.try_read() {
                    if let Some(enemy_base) = self.get_enemy_base_center(&enemy_player) {
                        let to_enemy = enemy_base - base_center;
                        let enemy_angle = to_enemy.y.atan2(to_enemy.x);

                        // Set front angles facing enemy direction
                        self.cur_front_left_defense_angle =
                            enemy_angle + std::f32::consts::PI / 6.0;
                        self.cur_front_right_defense_angle =
                            enemy_angle - std::f32::consts::PI / 6.0;
                    }
                }
            }
        }
    }

    /// Get enemy base center
    /// Get enemy base center
    fn get_enemy_base_center(&self, _enemy: &Player) -> Option<Coord3D> {
        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return None;
        };

        let player_index = _enemy.get_player_index() as UnsignedInt;
        let object_ids = manager.get_objects_owned_by_player(player_index);

        let mut min = Coord3D::new(f32::MAX, f32::MAX, 0.0);
        let mut max = Coord3D::new(f32::MIN, f32::MIN, 0.0);
        let mut found = false;

        for obj_id in object_ids {
            let Some(obj_arc) = manager.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_instance) = obj_arc.read() else {
                continue;
            };
            let Ok(base_obj) = obj_instance.base.read() else {
                continue;
            };
            if !base_obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            let pos = base_obj.get_position();
            min.x = min.x.min(pos.x);
            min.y = min.y.min(pos.y);
            max.x = max.x.max(pos.x);
            max.y = max.y.max(pos.y);
            found = true;
        }

        if !found {
            return None;
        }

        Some(Coord3D::new(
            min.x + (max.x - min.x) * 0.5,
            min.y + (max.y - min.y) * 0.5,
            0.0,
        ))
    }

    fn player_has_any_objects(&self, player: &Player) -> bool {
        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return false;
        };
        let object_ids =
            manager.get_objects_owned_by_player(player.get_player_index() as UnsignedInt);
        !object_ids.is_empty()
    }

    /// Check if under powered
    fn is_under_powered(&self) -> bool {
        let Some(player_arc) = self.base.get_player() else {
            return false;
        };
        let Ok(player_guard) = player_arc.read() else {
            return false;
        };
        player_guard.get_energy().is_low_power()
    }

    /// Prioritize power buildings
    fn prioritize_power_buildings(&mut self) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return;
        };
        let mut info_opt = player_guard.get_build_list_mut();
        while let Some(info) = info_opt {
            let name = info.get_template_name();
            if name.is_empty() {
                info_opt = info.get_next_mut();
                continue;
            }
            if let Some(template) = TheThingFactory::find_template(name.as_str()) {
                if template.is_kind_of(KindOf::PowerPlant) || template.is_kind_of(KindOf::FSPower) {
                    if info.get_object_id() == crate::common::INVALID_ID
                        && !info.is_priority_build()
                    {
                        info.mark_priority_build();
                        break;
                    }
                }
            }
            info_opt = info.get_next_mut();
        }
        self.base.set_build_delay_frames(0);
    }

    /// Check if enemy threat is detected
    fn enemy_threat_detected(&self) -> bool {
        let Some(enemy_arc) = self
            .current_enemy
            .as_ref()
            .and_then(|enemy| enemy.upgrade())
        else {
            return false;
        };
        let Ok(enemy_guard) = enemy_arc.read() else {
            return false;
        };
        self.player_has_any_objects(&enemy_guard)
    }

    /// Prioritize defensive buildings
    fn prioritize_defensive_buildings(&mut self) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return;
        };
        let mut info_opt = player_guard.get_build_list_mut();
        while let Some(info) = info_opt {
            let name = info.get_template_name();
            if name.is_empty() {
                info_opt = info.get_next_mut();
                continue;
            }
            if let Some(template) = TheThingFactory::find_template(name.as_str()) {
                if template.is_kind_of(KindOf::Defense) {
                    if info.get_object_id() == crate::common::INVALID_ID
                        && !info.is_priority_build()
                    {
                        info.mark_priority_build();
                        break;
                    }
                }
            }
            info_opt = info.get_next_mut();
        }
        self.base.set_build_delay_frames(0);
    }

    /// Analyze enemy composition
    fn analyze_enemy_composition(&mut self, enemy: &Arc<RwLock<Player>>) {
        let Ok(enemy_guard) = enemy.read() else {
            return;
        };
        let enemy_index = enemy_guard.get_player_index() as UnsignedInt;

        let obj_manager = get_object_manager();
        let object_ids = {
            let Ok(manager) = obj_manager.read() else {
                return;
            };
            manager.get_objects_owned_by_player(enemy_index)
        };

        let mut infantry = 0u32;
        let mut vehicles = 0u32;
        let mut air = 0u32;

        let Ok(manager) = obj_manager.read() else {
            return;
        };
        for obj_id in object_ids {
            let Some(obj_arc) = manager.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            {
                let Ok(base_guard) = obj_guard.base.read() else {
                    continue;
                };
                if base_guard.is_kind_of(KindOf::Aircraft) {
                    air += 1;
                } else if base_guard.is_kind_of(KindOf::Vehicle) {
                    vehicles += 1;
                } else if base_guard.is_kind_of(KindOf::Infantry) {
                    infantry += 1;
                }
            }
        }

        self.enemy_infantry_count = infantry;
        self.enemy_vehicle_count = vehicles;
        self.enemy_air_count = air;
    }

    /// Build counter units based on enemy composition analysis.
    /// Matches C++ AIPlayer team selection logic: when enemy has many of a unit
    /// type, prioritize building teams that contain the counter unit type.
    fn build_counter_units(&mut self) {
        let faction_priority = self.select_faction_build_priority();

        if self.enemy_air_count > 0 {
            self.prioritize_defensive_buildings();
            if !faction_priority.anti_air_unit.is_empty() {
                self.build_specific_ai_team_by_name(&faction_priority.anti_air_unit, true);
            }
        }

        if self.enemy_vehicle_count > self.enemy_infantry_count {
            if !faction_priority.anti_vehicle_unit.is_empty() {
                self.build_specific_ai_team_by_name(&faction_priority.anti_vehicle_unit, false);
            }
        }

        if self.enemy_infantry_count > self.enemy_vehicle_count {
            if !faction_priority.anti_infantry_unit.is_empty() {
                self.build_specific_ai_team_by_name(&faction_priority.anti_infantry_unit, false);
            }
        }

        if self.enemy_air_count > 0 || self.enemy_vehicle_count > self.enemy_infantry_count {
            self.base.set_team_delay_frames(0);
        }
    }

    /// Check if any ready teams have finished moving to the rally point.
    /// Matches C++ AISkirmishPlayer::checkReadyTeams (delegates to AIPlayer)
    pub fn check_ready_teams(&mut self) {
        // Base check_ready_teams is private but is called in update_without_base_building.
        // The skirmish update path calls update_without_base_building which handles this.
    }

    /// Check if any queued teams have finished building or timed out.
    /// Matches C++ AISkirmishPlayer::checkQueuedTeams (delegates to AIPlayer)
    pub fn check_queued_teams(&mut self) {
        // Base check_queued_teams is private but is called in update_without_base_building.
        // The skirmish update path calls update_without_base_building which handles this.
    }

    /// Queue up a dozer for construction.
    /// Matches C++ AISkirmishPlayer::queueDozer (delegates to AIPlayer)
    pub fn queue_dozer(&mut self) {
        if let Err(err) = self.base.queue_dozer() {
            log::debug!("AISkirmishPlayer::queue_dozer failed: {err}");
        }
    }

    /// Manage economy based on current resource levels.
    /// Matches C++ AIPlayer wealth/poor threshold logic used throughout
    /// processBaseBuilding and doTeamBuilding.
    pub fn manage_economy(&mut self) -> EconomyDecision {
        let Some(player_arc) = self.base.get_player() else {
            return EconomyDecision::Maintain;
        };
        let Ok(player_guard) = player_arc.read() else {
            return EconomyDecision::Maintain;
        };
        let current_money = player_guard.get_money().get_money();

        let (poor, wealthy) = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|data| (data.resources_poor, data.resources_wealthy))
            })
            .unwrap_or((
                crate::ai::ai_player::RESOURCES_POOR,
                crate::ai::ai_player::RESOURCES_WEALTHY,
            ));

        let is_poor = current_money < poor;
        let is_wealthy = current_money > wealthy;

        let decision = if is_poor {
            EconomyDecision::EmergencyEconomy
        } else if current_money < poor + (wealthy - poor) / 3 {
            EconomyDecision::ConserveResources
        } else if is_wealthy {
            EconomyDecision::InvestHeavily
        } else {
            EconomyDecision::Maintain
        };

        if is_poor {
            self.base.set_team_delay_frames(0);
        }

        decision
    }

    /// Select faction-specific build priority based on current game state.
    /// Matches C++ AISideInfo faction defense structure selection used in
    /// buildAIBaseDefense and the INI-driven side build lists.
    pub fn select_faction_build_priority(&self) -> FactionBuildPriority {
        let Some(player_arc) = self.base.get_player() else {
            return FactionBuildPriority::default();
        };
        let player_side = match player_arc.read() {
            Ok(guard) => guard.get_side().clone(),
            Err(_) => return FactionBuildPriority::default(),
        };

        let (infantry, vehicles, air) = (
            self.enemy_infantry_count,
            self.enemy_vehicle_count,
            self.enemy_air_count,
        );

        let dominant_enemy = if air > infantry && air > vehicles {
            DominantEnemyType::Air
        } else if vehicles > infantry {
            DominantEnemyType::Vehicle
        } else if infantry > 0 {
            DominantEnemyType::Infantry
        } else {
            DominantEnemyType::Unknown
        };

        let is_under_powered = self.is_under_powered();

        let priority = match player_side.as_str() {
            "America" | "USA" => match dominant_enemy {
                DominantEnemyType::Air => FactionBuildPriority {
                    defense_structure: "AmericaPatriotBattery".to_string(),
                    anti_infantry_unit: "AmericaMissileDefender".to_string(),
                    anti_vehicle_unit: "AmericaTankCrusader".to_string(),
                    anti_air_unit: "AmericaJetRaptor".to_string(),
                    priority_building: if is_under_powered {
                        "AmericaPowerPlant".to_string()
                    } else {
                        "AmericaWarFactory".to_string()
                    },
                },
                DominantEnemyType::Vehicle => FactionBuildPriority {
                    defense_structure: "AmericaPatriotBattery".to_string(),
                    anti_infantry_unit: "AmericaMissileDefender".to_string(),
                    anti_vehicle_unit: "AmericaTankCrusader".to_string(),
                    anti_air_unit: "AmericaJetRaptor".to_string(),
                    priority_building: "AmericaWarFactory".to_string(),
                },
                DominantEnemyType::Infantry => FactionBuildPriority {
                    defense_structure: "AmericaPatriotBattery".to_string(),
                    anti_infantry_unit: "AmericaMissileDefender".to_string(),
                    anti_vehicle_unit: "AmericaTankCrusader".to_string(),
                    anti_air_unit: "AmericaJetRaptor".to_string(),
                    priority_building: "AmericaBarracks".to_string(),
                },
                DominantEnemyType::Unknown => FactionBuildPriority {
                    defense_structure: "AmericaPatriotBattery".to_string(),
                    anti_infantry_unit: "AmericaMissileDefender".to_string(),
                    anti_vehicle_unit: "AmericaTankCrusader".to_string(),
                    anti_air_unit: "AmericaJetRaptor".to_string(),
                    priority_building: "AmericaSupplyCenter".to_string(),
                },
            },
            "China" => match dominant_enemy {
                DominantEnemyType::Air => FactionBuildPriority {
                    defense_structure: "ChinaGattlingCannon".to_string(),
                    anti_infantry_unit: "ChinaRedguard".to_string(),
                    anti_vehicle_unit: "ChinaTankBattleMaster".to_string(),
                    anti_air_unit: "ChinaJetMiG".to_string(),
                    priority_building: "ChinaAirfield".to_string(),
                },
                DominantEnemyType::Vehicle => FactionBuildPriority {
                    defense_structure: "ChinaBunker".to_string(),
                    anti_infantry_unit: "ChinaRedguard".to_string(),
                    anti_vehicle_unit: "ChinaTankBattleMaster".to_string(),
                    anti_air_unit: "ChinaJetMiG".to_string(),
                    priority_building: "ChinaWarFactory".to_string(),
                },
                DominantEnemyType::Infantry => FactionBuildPriority {
                    defense_structure: "ChinaBunker".to_string(),
                    anti_infantry_unit: "ChinaRedguard".to_string(),
                    anti_vehicle_unit: "ChinaTankBattleMaster".to_string(),
                    anti_air_unit: "ChinaJetMiG".to_string(),
                    priority_building: "ChinaBarracks".to_string(),
                },
                DominantEnemyType::Unknown => FactionBuildPriority {
                    defense_structure: "ChinaBunker".to_string(),
                    anti_infantry_unit: "ChinaRedguard".to_string(),
                    anti_vehicle_unit: "ChinaTankBattleMaster".to_string(),
                    anti_air_unit: "ChinaJetMiG".to_string(),
                    priority_building: "ChinaSupplyCenter".to_string(),
                },
            },
            "GLA" => match dominant_enemy {
                DominantEnemyType::Air => FactionBuildPriority {
                    defense_structure: "GLAStingerSite".to_string(),
                    anti_infantry_unit: "GLARebel".to_string(),
                    anti_vehicle_unit: "GLATankScorpion".to_string(),
                    anti_air_unit: "GLAQuadCannon".to_string(),
                    priority_building: "GLAArmsDealer".to_string(),
                },
                DominantEnemyType::Vehicle => FactionBuildPriority {
                    defense_structure: "GLATunnelNetwork".to_string(),
                    anti_infantry_unit: "GLARebel".to_string(),
                    anti_vehicle_unit: "GLATankScorpion".to_string(),
                    anti_air_unit: "GLAQuadCannon".to_string(),
                    priority_building: "GLAArmsDealer".to_string(),
                },
                DominantEnemyType::Infantry => FactionBuildPriority {
                    defense_structure: "GLATunnelNetwork".to_string(),
                    anti_infantry_unit: "GLARebel".to_string(),
                    anti_vehicle_unit: "GLATankScorpion".to_string(),
                    anti_air_unit: "GLAQuadCannon".to_string(),
                    priority_building: "GLABarracks".to_string(),
                },
                DominantEnemyType::Unknown => FactionBuildPriority {
                    defense_structure: "GLATunnelNetwork".to_string(),
                    anti_infantry_unit: "GLARebel".to_string(),
                    anti_vehicle_unit: "GLATankScorpion".to_string(),
                    anti_air_unit: "GLAQuadCannon".to_string(),
                    priority_building: "GLASupplyStash".to_string(),
                },
            },
            _ => FactionBuildPriority::default(),
        };

        priority
    }

    /// Get AI difficulty
    pub fn get_ai_difficulty(&self) -> GameDifficulty {
        self.base.get_ai_difficulty()
    }

    /// Set AI difficulty
    pub fn set_ai_difficulty(&mut self, difficulty: GameDifficulty) {
        self.base.set_ai_difficulty(difficulty);
    }

    pub fn build_base_defense(&mut self, flank: bool) -> Result<(), crate::ai::AiError> {
        self.base.build_ai_base_defense(flank)
    }

    pub fn build_base_defense_structure(
        &mut self,
        structure_name: &str,
        flank: bool,
    ) -> Result<(), crate::ai::AiError> {
        self.base
            .build_ai_base_defense_structure(structure_name, flank)
    }

    pub fn build_specific_building(
        &mut self,
        building_name: &str,
    ) -> Result<(), crate::ai::AiError> {
        self.base.build_specific_ai_building(building_name)
    }

    pub fn build_by_supplies(
        &mut self,
        minimum_cash: i32,
        building_name: &str,
    ) -> Result<(), crate::ai::AiError> {
        self.base.build_by_supplies(minimum_cash, building_name)
    }

    pub fn build_upgrade(&mut self, upgrade_name: &str) -> Result<(), crate::ai::AiError> {
        self.base.build_upgrade(upgrade_name)
    }

    pub fn select_skillset(&mut self, skillset: i32) {
        self.base.select_skillset(skillset);
    }

    pub fn set_team_delay_seconds(&mut self, delay_seconds: f32) {
        self.base.set_team_delay_seconds(delay_seconds);
    }

    pub fn is_supply_source_safe(&self, min_supplies: i32) -> bool {
        self.base.is_supply_source_safe(min_supplies)
    }

    pub fn is_supply_source_attacked(&self) -> bool {
        self.base.is_supply_source_attacked()
    }

    pub fn build_specific_building_near_location(
        &mut self,
        building_name: &str,
        location: Coord3D,
    ) -> Result<(), crate::ai::AiError> {
        self.base
            .build_specific_building_near_location(building_name, location)
    }

    pub fn repair_structure(&mut self, structure_id: ObjectID) -> Result<(), crate::ai::AiError> {
        self.base.repair_structure(structure_id)
    }

    pub fn on_structure_produced(
        &mut self,
        factory_id: ObjectID,
        structure_id: ObjectID,
    ) -> Result<(), crate::ai::AiError> {
        self.base.on_structure_produced(factory_id, structure_id)
    }

    /// Pick the next build order based on game state, difficulty, and enemy composition.
    /// Returns the thing template name of the structure to build, or None if nothing to build.
    /// PARITY_NOTE: C++ picks from the INI BuildList in sequence. This mirrors the
    /// sequential scan with difficulty-weighted priority adjustments.
    pub fn pick_build_order(&self) -> Option<String> {
        let player_arc = self.base.get_player()?;
        let player_guard = player_arc.read().ok()?;
        let difficulty = self.base.get_ai_difficulty();

        let mut build_info = player_guard.get_build_list();
        let mut best_name: Option<String> = None;
        let mut best_priority: i32 = i32::MIN;

        while let Some(info) = build_info {
            if info.get_object_id() != crate::common::types::INVALID_ID {
                build_info = info.get_next();
                continue;
            }

            let name = info.get_template_name().to_string();
            let mut priority: i32 = if info.is_priority_build() { 10 } else { 0 };

            match difficulty {
                GameDifficulty::Hard => priority += 2,
                GameDifficulty::Normal => priority += 1,
                GameDifficulty::Easy => priority -= 1,
                _ => {}
            }

            let enemy_infantry_heavy = self.enemy_infantry_count > self.enemy_vehicle_count
                && self.enemy_infantry_count > self.enemy_air_count;
            let enemy_vehicle_heavy = self.enemy_vehicle_count > self.enemy_infantry_count
                && self.enemy_vehicle_count > self.enemy_air_count;
            let enemy_air_heavy = self.enemy_air_count > self.enemy_infantry_count
                && self.enemy_air_count > self.enemy_vehicle_count;

            if enemy_infantry_heavy && name.contains("AntiInfantry") {
                priority += 3;
            }
            if enemy_vehicle_heavy && name.contains("AntiTank") {
                priority += 3;
            }
            if enemy_air_heavy && name.contains("AntiAir") {
                priority += 3;
            }

            if priority > best_priority {
                best_priority = priority;
                best_name = Some(name);
            }

            build_info = info.get_next();
        }

        best_name
    }

    /// Evaluate the current threat level from enemies.
    /// Returns a threat assessment with threat_level (0.0..1.0) and recommended_response.
    /// PARITY_NOTE: C++ evaluates threat based on nearby enemy units detected via
    /// partition manager spatial queries. This mirrors that logic with distance-based
    /// threat weighting and unit type multipliers.
    pub fn evaluate_threat(&self) -> ThreatAssessment {
        let Some(player_arc) = self.base.get_player() else {
            return ThreatAssessment::default();
        };
        let Ok(player_guard) = player_arc.read() else {
            return ThreatAssessment::default();
        };

        let base_center = match self.base.get_base_center() {
            Some(c) => c,
            None => return ThreatAssessment::default(),
        };

        let mut total_threat: f32 = 0.0;
        let mut nearby_infantry: u32 = 0;
        let mut nearby_vehicles: u32 = 0;
        let mut nearby_aircraft: u32 = 0;
        let threat_radius: f32 = 500.0;

        if let Some(partition_mgr) = ThePartitionManager::get() {
            let objects_in_range = partition_mgr.get_objects_in_range(&base_center, threat_radius);

            let Some(my_team_arc) = player_guard.get_default_team() else {
                return ThreatAssessment::default();
            };
            let Ok(my_team) = my_team_arc.read() else {
                return ThreatAssessment::default();
            };

            for obj_id in &objects_in_range {
                let Some(obj_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(*obj_id)
                else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };

                let Some(obj_team_arc) = obj_guard.get_team() else {
                    continue;
                };
                let Ok(obj_team) = obj_team_arc.read() else {
                    continue;
                };

                if my_team.get_relationship(&obj_team) != Relationship::Enemies {
                    continue;
                }

                let pos = obj_guard.get_position();
                let dx = pos.x - base_center.x;
                let dy = pos.y - base_center.y;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let proximity_weight = 1.0 - (dist / threat_radius).min(1.0);

                if obj_guard.is_kind_of(KindOf::Infantry) {
                    nearby_infantry += 1;
                    total_threat += 1.0 * proximity_weight;
                } else if obj_guard.is_kind_of(KindOf::Vehicle) {
                    nearby_vehicles += 1;
                    total_threat += 2.0 * proximity_weight;
                } else if obj_guard.is_kind_of(KindOf::Aircraft) {
                    nearby_aircraft += 1;
                    total_threat += 2.5 * proximity_weight;
                }
            }
        }

        let threat_level = (total_threat / 50.0).min(1.0);

        let dominant_type =
            if nearby_aircraft >= nearby_infantry && nearby_aircraft >= nearby_vehicles {
                ThreatType::Air
            } else if nearby_vehicles >= nearby_infantry {
                ThreatType::Vehicle
            } else if nearby_infantry > 0 {
                ThreatType::Infantry
            } else {
                ThreatType::None
            };

        let recommended_response = if threat_level > 0.7 {
            ResponseType::DefensiveBuild
        } else if threat_level > 0.4 {
            ResponseType::CounterAttack
        } else {
            ResponseType::Expand
        };

        ThreatAssessment {
            threat_level,
            dominant_type,
            recommended_response,
            infantry_count: nearby_infantry,
            vehicle_count: nearby_vehicles,
            aircraft_count: nearby_aircraft,
        }
    }

    /// Choose the best attack target based on distance, vulnerability, and strategic value.
    /// Returns (player_index, target_position) of the best target.
    /// PARITY_NOTE: C++ selects the nearest vulnerable enemy base via player list
    /// iteration with base-center distance comparison. This mirrors that logic with
    /// additional strategic weighting for weakened enemies.
    pub fn choose_attack_target(&self) -> Option<(i32, Coord3D)> {
        let Some(me_arc) = self.base.get_player() else {
            return None;
        };
        let Ok(me_guard) = me_arc.read() else {
            return None;
        };
        let my_center = self.base.get_base_center()?;

        let Ok(player_list) = ThePlayerList().read() else {
            return None;
        };

        let mut best_target: Option<(i32, Coord3D)> = None;
        let mut best_score: f32 = f32::MAX;

        for player_arc in player_list.iter() {
            let Ok(player_guard) = player_arc.read() else {
                continue;
            };

            let Some(their_team_arc) = player_guard.get_default_team() else {
                continue;
            };
            let Ok(their_team) = their_team_arc.read() else {
                continue;
            };
            let Some(my_team_arc) = me_guard.get_default_team() else {
                continue;
            };
            let Ok(my_team) = my_team_arc.read() else {
                continue;
            };

            if my_team.get_relationship(&their_team) != Relationship::Enemies {
                continue;
            }

            let has_buildings = player_guard.has_any_build_facility();
            let has_units = player_guard.has_any_units();
            if !has_buildings && !has_units {
                continue;
            }

            let enemy_center = self
                .get_enemy_base_center(&player_guard)
                .unwrap_or(my_center);

            let dx = enemy_center.x - my_center.x;
            let dy = enemy_center.y - my_center.y;
            let dist = (dx * dx + dy * dy).sqrt();

            let mut score = dist;

            if !has_buildings {
                score *= 2.0;
            }
            if !has_units {
                score *= 1.5;
            }

            if score < best_score {
                best_score = score;
                best_target = Some((player_guard.get_player_index(), enemy_center));
            }
        }

        best_target
    }
}

impl Snapshot for AISkirmishPlayer {
    fn crc(&self, xfer: &mut dyn Xfer) {
        let mut cur_front_base_defense = self.cur_front_base_defense;
        let _ = xfer.xfer_int(&mut cur_front_base_defense);

        let mut cur_flank_base_defense = self.cur_flank_base_defense;
        let _ = xfer.xfer_int(&mut cur_flank_base_defense);

        let mut cur_front_left_defense_angle = self.cur_front_left_defense_angle;
        let _ = xfer.xfer_real(&mut cur_front_left_defense_angle);

        let mut cur_front_right_defense_angle = self.cur_front_right_defense_angle;
        let _ = xfer.xfer_real(&mut cur_front_right_defense_angle);

        let mut cur_left_flank_left_defense_angle = self.cur_left_flank_left_defense_angle;
        let _ = xfer.xfer_real(&mut cur_left_flank_left_defense_angle);

        let mut cur_left_flank_right_defense_angle = self.cur_left_flank_right_defense_angle;
        let _ = xfer.xfer_real(&mut cur_left_flank_right_defense_angle);

        let mut cur_right_flank_left_defense_angle = self.cur_right_flank_left_defense_angle;
        let _ = xfer.xfer_real(&mut cur_right_flank_left_defense_angle);

        let mut cur_right_flank_right_defense_angle = self.cur_right_flank_right_defense_angle;
        let _ = xfer.xfer_real(&mut cur_right_flank_right_defense_angle);

        let mut frame_to_check_enemy = self.frame_to_check_enemy;
        let _ = xfer.xfer_unsigned_int(&mut frame_to_check_enemy);
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);

        self.base.xfer(xfer);

        let mut cur_front_base_defense = self.cur_front_base_defense;
        let _ = xfer.xfer_int(&mut cur_front_base_defense);
        if xfer.is_loading() {
            self.cur_front_base_defense = cur_front_base_defense;
        }

        let mut cur_flank_base_defense = self.cur_flank_base_defense;
        let _ = xfer.xfer_int(&mut cur_flank_base_defense);
        if xfer.is_loading() {
            self.cur_flank_base_defense = cur_flank_base_defense;
        }

        let mut cur_front_left_defense_angle = self.cur_front_left_defense_angle;
        let _ = xfer.xfer_real(&mut cur_front_left_defense_angle);
        if xfer.is_loading() {
            self.cur_front_left_defense_angle = cur_front_left_defense_angle;
        }

        let mut cur_front_right_defense_angle = self.cur_front_right_defense_angle;
        let _ = xfer.xfer_real(&mut cur_front_right_defense_angle);
        if xfer.is_loading() {
            self.cur_front_right_defense_angle = cur_front_right_defense_angle;
        }

        let mut cur_left_flank_left_defense_angle = self.cur_left_flank_left_defense_angle;
        let _ = xfer.xfer_real(&mut cur_left_flank_left_defense_angle);
        if xfer.is_loading() {
            self.cur_left_flank_left_defense_angle = cur_left_flank_left_defense_angle;
        }

        let mut cur_left_flank_right_defense_angle = self.cur_left_flank_right_defense_angle;
        let _ = xfer.xfer_real(&mut cur_left_flank_right_defense_angle);
        if xfer.is_loading() {
            self.cur_left_flank_right_defense_angle = cur_left_flank_right_defense_angle;
        }

        let mut cur_right_flank_left_defense_angle = self.cur_right_flank_left_defense_angle;
        let _ = xfer.xfer_real(&mut cur_right_flank_left_defense_angle);
        if xfer.is_loading() {
            self.cur_right_flank_left_defense_angle = cur_right_flank_left_defense_angle;
        }

        let mut cur_right_flank_right_defense_angle = self.cur_right_flank_right_defense_angle;
        let _ = xfer.xfer_real(&mut cur_right_flank_right_defense_angle);
        if xfer.is_loading() {
            self.cur_right_flank_right_defense_angle = cur_right_flank_right_defense_angle;
        }

        let mut frame_to_check_enemy = self.frame_to_check_enemy;
        let _ = xfer.xfer_unsigned_int(&mut frame_to_check_enemy);
        if xfer.is_loading() {
            self.frame_to_check_enemy = frame_to_check_enemy;
        }

        // PARITY_NOTE: C++ also xfers m_currentEnemy (Player*), but in Rust
        // we use Weak<RwLock<Player>> which cannot be directly serialized.
        // The enemy is re-acquired via acquire_enemy() on load.
    }

    fn load_post_process(&mut self) {
        self.current_enemy = None;
    }
}
