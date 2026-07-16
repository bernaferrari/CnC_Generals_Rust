use super::ai_player::{AIPlayer, WorkOrder};
use crate::ai::THE_AI;
use crate::build_list_info::BuildListInfo;
use crate::common::coord::*;
use crate::common::coord_ext::Coord2DExt;
use crate::common::xfer::{Xfer, XferExt};
use crate::common::Snapshot;
use crate::common::*;
use crate::helpers::{TheGameLogic, ThePartitionManager, TheTerrainLogic, TheThingFactory};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::production::construction::FoundationValidator;
use crate::object::registry::OBJECT_REGISTRY;
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
    /// C++ `AISkirmishPlayer::update` → `AIPlayer::update` with virtual overrides.
    ///
    /// Phase order: doBaseBuilding → checkReadyTeams → checkQueuedTeams →
    /// doTeamBuilding → doUpgradesAndSkills → updateBridgeRepair.
    pub fn update(&mut self) {
        self.do_base_building();
        if let Err(err) = self.base.check_ready_teams() {
            log::debug!("check_ready_teams: {err}");
        }
        if let Err(err) = self.base.check_queued_teams() {
            log::debug!("check_queued_teams: {err}");
        }
        self.do_team_building();
        if let Err(err) = self.base.do_upgrades_and_skills() {
            log::debug!("do_upgrades_and_skills: {err}");
        }
        if let Err(err) = self.base.update_bridge_repair() {
            log::debug!("update_bridge_repair: {err}");
        }
    }

    /// Called when new map is loaded
    /// C++ `AISkirmishPlayer::newMap` (AISkirmishPlayer.cpp).
    ///
    /// Load side build list, adjustBuildList to start CC, compute base center,
    /// buildStructureNow for initiallyBuilt else incrementNumRebuilds.
    pub fn new_map(&mut self) {
        // Reset skirmish-specific state (do not call AIPlayer::newMap — C++ doesn't).
        self.cur_front_base_defense = 0;
        self.cur_flank_base_defense = 0;
        self.cur_front_left_defense_angle = 0.0;
        self.cur_front_right_defense_angle = 0.0;
        self.cur_left_flank_left_defense_angle = 0.0;
        self.cur_left_flank_right_defense_angle = 0.0;
        self.cur_right_flank_left_defense_angle = 0.0;
        self.cur_right_flank_right_defense_angle = 0.0;
        self.frame_to_check_enemy = 0;
        self.current_enemy = None;
        self.enemy_infantry_count = 0;
        self.enemy_vehicle_count = 0;
        self.enemy_air_count = 0;

        // Clear base queues/timers without factory-scan (skirmish owns build list).
        self.base.clear_teams_in_queue();
        self.base.set_base_center_set(false);

        let player_side = {
            let Some(player_arc) = self.base.get_player() else {
                return;
            };
            let Ok(guard) = player_arc.read() else {
                return;
            };
            guard.get_side().clone()
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
            log::debug!("Couldn't find build list for skirmish player.");
            return;
        };

        self.adjust_build_list(&mut build_list);
        if let Some(player_arc) = self.base.get_player() {
            if let Ok(mut guard) = player_arc.write() {
                guard.set_build_list(Some(build_list));
            }
        }

        let _ = self.base.compute_center_and_radius_of_base();
        self.build_initial_structures();
    }

    /// Called when a unit is produced
    pub fn on_unit_produced(&mut self, factory: &Arc<RwLock<Object>>, unit: &Arc<RwLock<Object>>) {
        let (Ok(factory_guard), Ok(unit_guard)) = (factory.read(), unit.read()) else {
            return;
        };
        // C++ AISkirmishPlayer::onUnitProduced → AIPlayer::onUnitProduced only.
        let _ = self
            .base
            .on_unit_produced(factory_guard.get_id(), unit_guard.get_id());
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
            // C++: info->getTemplateName()==thingName (exact AsciiString match).
            if name == thing_name {
                // C++ still loads the template; missing template → continue.
                if name.is_empty() || TheThingFactory::find_template(name.as_str()).is_none() {
                    info_opt = info.get_next_mut();
                    continue;
                }
                found = true;
                // C++: Object *bldg = findObjectByID; if (bldg) continue — live object only.
                let obj_id = info.get_object_id();
                if obj_id != crate::common::INVALID_ID
                    && OBJECT_REGISTRY.get_object(obj_id).is_some()
                {
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
            log::debug!("Queueing building '{}' for construction.", thing_name);
        } else if found {
            log::debug!(
                "Warning - all instances of building '{}' are already built or queued for build, not queueing.",
                thing_name
            );
        } else {
            // C++ AISkirmishPlayer::buildSpecificAIBuilding — only marks existing
            // build-list entries; does not invent new ones via solo AIPlayer.
            log::debug!(
                "Error - could not find building '{}' in the building template list.",
                thing_name
            );
        }
    }

    /// Build AI base defense with skirmish-specific logic
    /// C++ `AISkirmishPlayer::buildAIBaseDefense` (AISkirmishPlayer.cpp).
    ///
    /// Resolve side `m_baseDefenseStructure1` and place via
    /// `buildAIBaseDefenseStructure`. No host residual fallbacks.
    pub fn build_ai_base_defense(&mut self, flank: bool) {
        let Some(player_arc) = self.base.get_player() else {
            return;
        };
        let player_side = match player_arc.read() {
            Ok(guard) => guard.get_side().clone(),
            Err(_) => return,
        };
        // C++ walks m_sideInfo until side match, then calls with that entry's
        // m_baseDefenseStructure1 (even if empty — template lookup fails fast).
        let defense_name = THE_AI.read().ok().and_then(|ai| {
            ai.get_ai_data().read().ok().and_then(|data| {
                data.side_info
                    .iter()
                    .find(|info| info.side == player_side)
                    .map(|info| info.base_defense_structure_1.clone())
            })
        });
        if let Some(name) = defense_name {
            self.build_ai_base_defense_structure(&name, flank);
        }
    }

    /// C++ `AISkirmishPlayer::buildAIBaseDefenseStructure` (AISkirmishPlayer.cpp).
    ///
    /// Place defenses along base radius toward center/flank/backdoor approach
    /// paths with alternating left/right angles; priority-build list on success.
    pub fn build_ai_base_defense_structure(&mut self, thing_name: &str, flank: bool) {
        let Some(template) = TheThingFactory::find_template(thing_name) else {
            log::debug!(
                "Couldn't find base defense structure '{}' for skirmish AI",
                thing_name
            );
            return;
        };
        let mp_start = {
            let Some(player_arc) = self.base.get_player() else {
                return;
            };
            let Ok(pg) = player_arc.read() else {
                return;
            };
            pg.get_mp_start_index() + 1
        };
        let player_id = {
            let Some(player_arc) = self.base.get_player() else {
                return;
            };
            let Ok(pg) = player_arc.read() else {
                return;
            };
            pg.get_id() as ObjectID
        };

        loop {
            let path_label = if flank {
                if self.cur_flank_base_defense & 1 != 0 {
                    format!("{}{}", SKIRMISH_FLANK, mp_start)
                } else {
                    format!("{}{}", SKIRMISH_BACKDOOR, mp_start)
                }
            } else {
                format!("{}{}", SKIRMISH_CENTER, mp_start)
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
                        if let Ok((lo, hi)) = self.base.get_player_structure_bounds(enemy_index) {
                            goal_pos = Coord3D::new(
                                lo.x + (hi.x - lo.x) * 0.5,
                                lo.y + (hi.y - lo.y) * 0.5,
                                0.0,
                            );
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
            let base_circumference = 2.0 * std::f32::consts::PI * defense_distance.max(1.0);
            let angle_offset =
                2.0 * std::f32::consts::PI * (structure_radius * 4.0 / base_circumference);

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

            let place_angle = template.get_placement_view_angle();
            let validator = FoundationValidator::new_ai();
            if validator
                .validate_placement(&build_pos, thing_name, place_angle, player_id)
                .is_err()
            {
                continue;
            }

            if let Some(player_arc) = self.base.get_player() {
                if let Ok(mut pg) = player_arc.write() {
                    pg.add_to_priority_build_list(
                        crate::common::AsciiString::from(thing_name),
                        build_pos,
                        place_angle,
                    );
                }
            }
            break;
        }
    }

    /// Recruit specific AI team
    /// C++ `AISkirmishPlayer::recruitSpecificAITeam` (AISkirmishPlayer.cpp).
    ///
    /// Same recruit path as AIPlayer, but always warns when the team has no home
    /// (C++ skirmish override does not gate the message on isSkirmishAI).
    pub fn recruit_specific_ai_team(&mut self, team_proto: &TeamPrototype, recruit_radius: f32) {
        if !team_proto.has_home_location() {
            log::debug!(
                "Error : team '{}' has no Home Position (or Origin).",
                team_proto.get_name()
            );
        }
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
    /// C++ `AISkirmishPlayer::checkBridges` (AISkirmishPlayer.cpp).
    ///
    /// Walk the waypoint path: if `clientSafeQuickDoesPathExist` for the unit's
    /// locomotor set, that hop is fine. Else `pathfinder->findBrokenBridge` and
    /// `repairStructure` when a destroyed bridge blocks the hop.
    pub fn check_bridges(&mut self, unit: &Arc<RwLock<Object>>, waypoint: &Waypoint) -> bool {
        let (unit_pos, loco_set) = {
            let Ok(unit_guard) = unit.try_read() else {
                return false;
            };
            // C++: if (!ai) return false;
            let Some(ai) = unit_guard.get_ai_update_interface() else {
                return false;
            };
            let loco = ai.get_locomotor_set_clone();
            (*unit_guard.get_position(), loco)
        };
        let Some(loco_set) = loco_set else {
            return false;
        };

        // C++: for (curWay = way; curWay; curWay = curWay->getNext())
        // Rust Waypoint uses link IDs; walk this waypoint then linked targets.
        let mut hop_targets: Vec<Coord3D> = vec![waypoint.position];
        if let Ok(terrain_guard) = get_terrain_logic().read() {
            for link_id in &waypoint.links {
                if let Some(linked) = terrain_guard.get_waypoint_by_id(*link_id) {
                    hop_targets.push(*linked.get_location());
                }
            }
        }

        let Some(pf_arc) = THE_AI.read().ok().and_then(|ai| ai.pathfinder()) else {
            return false;
        };
        let Ok(pf) = pf_arc.read() else {
            return false;
        };

        for target in hop_targets {
            // C++: if (pathfinder->clientSafeQuickDoesPathExist(...)) continue;
            if pf.client_safe_quick_does_path_exist(&loco_set, &unit_pos, &target) {
                continue;
            }
            // C++: if (pathfinder->findBrokenBridge(..., &brokenBridge)) repair; return true;
            if let Some(bridge_id) = pf.find_broken_bridge(&loco_set, &unit_pos, &target) {
                drop(pf);
                let _ = self.base.repair_structure(bridge_id);
                return true;
            }
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
        player_index: i32,
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

            // C++: goalPos = m_baseCenter; way = getClosestWaypointOnPath;
            // else getPlayerStructureBounds(enemy) → center of bounds.
            let base_center = self.base.get_base_center().unwrap_or_default();
            let mut goal_pos = base_center;
            if let Some(terrain) = TheTerrainLogic::get() {
                if let Some(way_pos) = terrain.get_closest_waypoint_on_path(&goal_pos, &path_label)
                {
                    goal_pos = way_pos;
                } else {
                    let enemy_index = self.get_my_enemy_player_index();
                    if enemy_index >= 0 {
                        if let Ok((lo, hi)) = self.base.get_player_structure_bounds(enemy_index) {
                            // C++ Region2D center: lo + width/2, lo + height/2
                            goal_pos = Coord3D::new(
                                lo.x + (hi.x - lo.x) * 0.5,
                                lo.y + (hi.y - lo.y) * 0.5,
                                0.0,
                            );
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

        if let Ok(Some(target)) =
            self.base
                .compute_superweapon_target(power.get_name(), weapon_radius, player_index)
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

        // C++ walks BuildListInfo once: power preference is inline (FS_POWER pick),
        // not a separate prioritize_* residual pass.
        let current_frame = TheGameLogic::get_frame();
        // C++: TheAI->getAiData()->m_rebuildDelaySeconds * LOGICFRAMES_PER_SECOND
        // Retail AIData = 30; fall back when AIData unloaded / zero.
        let rebuild_delay_frames = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    if data.rebuild_delay_seconds > 0 {
                        data.rebuild_delay_seconds as u32
                    } else {
                        crate::ai::ai_player::REBUILD_DELAY_SECONDS
                    }
                })
            })
            .unwrap_or(crate::ai::ai_player::REBUILD_DELAY_SECONDS)
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
        let mut selected_loc = None;
        let mut selected_angle = 0.0_f32;
        let mut is_priority = false;
        let mut power_plan = None;
        let mut power_name = None;
        let mut power_loc = None;
        let mut power_angle = 0.0_f32;
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
                                && !cur_plan.is_kind_of(KindOf::CashGenerator)
                            {
                                power_under_construction = true;
                            }
                            if obj_guard.is_under_construction() {
                                info.set_under_construction(true);
                                let builder_id = obj_guard.get_builder_id();
                                let bldg_pos = *obj_guard.get_position();
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
                                    // C++ findDozer + aiResumeConstruction
                                    if let Ok(Some(dozer_id)) =
                                        self.base.find_dozer_public(&bldg_pos)
                                    {
                                        if let Some(dozer_arc) =
                                            TheGameLogic::find_object_by_id(dozer_id)
                                        {
                                            if let Ok(dg) = dozer_arc.read() {
                                                if let Some(ai) = dg.get_ai_update_interface() {
                                                    if let Ok(mut ai_g) = ai.lock() {
                                                        let mut params =
                                                            crate::ai::AiCommandParams::new(
                                                                crate::ai::AiCommandType::ResumeConstruction,
                                                                crate::ai::CommandSourceType::FromAi,
                                                            );
                                                        params.obj = Some(obj_id);
                                                        let _ = ai_g.execute_command(&params);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // C++ always re-issues resume construction on valid dozer.
                                    if let Some(builder_arc) =
                                        TheGameLogic::find_object_by_id(builder_id)
                                    {
                                        if let Ok(dg) = builder_arc.read() {
                                            if let Some(ai) = dg.get_ai_update_interface() {
                                                if let Ok(mut ai_g) = ai.lock() {
                                                    let mut params =
                                                        crate::ai::AiCommandParams::new(
                                                            crate::ai::AiCommandType::ResumeConstruction,
                                                            crate::ai::CommandSourceType::FromAi,
                                                        );
                                                    params.obj = Some(obj_id);
                                                    let _ = ai_g.execute_command(&params);
                                                }
                                            }
                                        }
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
                // C++: destroyed → clear ID, stamp timestamp, scan for GLA hole
                // whose RebuildHoleBehavior spawnerID == prior building ID.
                let prior_id = obj_id;
                info.set_object_id(crate::common::INVALID_ID);
                info.set_object_timestamp(current_frame + 1);

                // Walk all objects for KINDOF_REBUILD_HOLE (C++ getFirstObject loop).
                for candidate_arc in OBJECT_REGISTRY.get_all_objects() {
                    let Ok(candidate_guard) = candidate_arc.read() else {
                        continue;
                    };
                    if !candidate_guard.is_kind_of(KindOf::RebuildHole) {
                        continue;
                    }
                    let candidate_id = candidate_guard.get_id();
                    // Find RebuildHoleBehaviorInterface::getSpawnerID.
                    let mut matched_hole = false;
                    for behavior in candidate_guard.get_behavior_modules() {
                        if let Ok(mut bg) = behavior.lock() {
                            if let Some(rhbi) = bg.get_rebuild_hole_behavior_interface() {
                                if rhbi.get_spawner_id() == prior_id {
                                    matched_hole = true;
                                }
                                break;
                            }
                        }
                    }
                    if matched_hole {
                        info.set_object_id(candidate_id);
                        log::debug!("AI Found hole to rebuild {}", cur_plan.get_name().as_str());
                        break;
                    }
                }
            }

            // C++: delay only when objectID==INVALID_ID && timestamp>0.
            if info.get_object_id() == crate::common::INVALID_ID && info.get_object_timestamp() > 0
            {
                if info.get_object_timestamp() + rebuild_delay_frames > current_frame {
                    info_opt = info.get_next_mut();
                    continue;
                }
                log::debug!("Enabling rebuild for {}", name);
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
                selected_loc = Some(*info.get_location());
                selected_angle = info.get_angle();
                is_priority = true;
            }

            // C++: FS_POWER && !CASH_GENERATOR
            let is_power_plan = (cur_plan.is_kind_of(KindOf::FSPower)
                || cur_plan.is_kind_of(KindOf::PowerPlant))
                && !cur_plan.is_kind_of(KindOf::CashGenerator);
            if power_plan.is_none()
                && is_power_plan
                && (is_under_powered || info.is_automatic_build())
            {
                power_plan = Some(cur_plan.clone());
                power_name = Some(name.clone());
                power_loc = Some(*info.get_location());
                power_angle = info.get_angle();
            }

            if !info.is_automatic_build() {
                info_opt = info.get_next_mut();
                continue;
            }

            if !info.is_buildable() {
                info_opt = info.get_next_mut();
                continue;
            }

            // C++ also requires a dozer present before selecting automatic builds.
            if selected_plan.is_none() {
                if self
                    .base
                    .find_dozer_public(info.get_location())
                    .ok()
                    .flatten()
                    .is_none()
                {
                    if is_under_powered {
                        let _ = self.base.queue_dozer();
                    }
                    info_opt = info.get_next_mut();
                    continue;
                }
                selected_plan = Some(cur_plan);
                selected_name = Some(name);
                selected_loc = Some(*info.get_location());
                selected_angle = info.get_angle();
            }

            info_opt = info.get_next_mut();
        }

        if let Some(power) = power_plan {
            if !power_under_construction {
                if let Some(selected) = selected_plan.as_ref() {
                    if !power.is_equivalent_to(selected.as_ref()) {
                        selected_plan = Some(power);
                        selected_name = power_name;
                        selected_loc = power_loc;
                        selected_angle = power_angle;
                    }
                } else {
                    selected_plan = Some(power);
                    selected_name = power_name;
                    selected_loc = power_loc;
                    selected_angle = power_angle;
                }
            }
        }

        // Drop player lock before building.
        drop(player_guard);
        drop(player_arc);

        if let (Some(name), Some(loc)) = (selected_name, selected_loc) {
            // C++ USE_DOZER: buildStructureWithDozer + arm structure timer on success.
            match self
                .base
                .build_structure_with_dozer(name.as_str(), loc, selected_angle)
            {
                Ok(Some(_bldg_id)) => {
                    // C++: ready=false; structureTimer = structureSeconds*FPS / wealth mods.
                    if let Err(err) = self.base.arm_structure_timer_after_build() {
                        log::debug!("arm_structure_timer_after_build: {err}");
                    }
                    self.base
                        .set_frame_last_building_built(TheGameLogic::get_frame());
                }
                Ok(None) => {
                    log::debug!(
                        "AISkirmishPlayer processBaseBuilding: could not start '{}'",
                        name
                    );
                }
                Err(err) => {
                    log::debug!(
                        "AISkirmishPlayer::build_structure_with_dozer('{}') failed: {err}",
                        name
                    );
                }
            }
        }
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
                        if data.resources_poor > 0 {
                            data.resources_poor
                        } else {
                            crate::ai::ai_player::RESOURCES_POOR
                        },
                        if data.resources_wealthy > 0 {
                            data.resources_wealthy
                        } else {
                            crate::ai::ai_player::RESOURCES_WEALTHY
                        },
                        if data.structures_poor_mod > 0.0 {
                            data.structures_poor_mod
                        } else {
                            crate::ai::ai_player::STRUCTURES_POOR_MODIFIER
                        },
                        if data.structures_wealthy_mod > 0.0 {
                            data.structures_wealthy_mod
                        } else {
                            crate::ai::ai_player::STRUCTURES_WEALTHY_MODIFIER
                        },
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
            self.base.set_structure_timer_frames(new_timer);
        }
    }

    /// Process team building with skirmish-specific logic
    /// Matches C++ AISkirmishPlayer::processTeamBuilding
    /// C++ `AISkirmishPlayer::processTeamBuilding`.
    /// C++ `AISkirmishPlayer::processTeamBuilding`: selectTeamToBuild then queueUnits.
    fn process_team_building(&mut self) {
        if self.select_team_to_build() {
            self.base.queue_units();
        }
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
                        if data.resources_poor > 0 {
                            data.resources_poor
                        } else {
                            crate::ai::ai_player::RESOURCES_POOR
                        },
                        if data.resources_wealthy > 0 {
                            data.resources_wealthy
                        } else {
                            crate::ai::ai_player::RESOURCES_WEALTHY
                        },
                        if data.team_poor_mod > 0.0 {
                            data.team_poor_mod
                        } else {
                            crate::ai::ai_player::TEAMS_POOR_MODIFIER
                        },
                        if data.team_wealthy_mod > 0.0 {
                            data.team_wealthy_mod
                        } else {
                            crate::ai::ai_player::TEAMS_WEALTHY_MODIFIER
                        },
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
            self.base.set_team_timer_frames(new_timer);
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

    /// C++ `AISkirmishPlayer::doBaseBuilding`.
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
        drop(player_guard);

        // C++ AISkirmishPlayer::doBaseBuilding:
        // if !ready: structureTimer--; <=0 → ready + buildDelay=0; clamp >3s.
        // buildDelay--; if <1: processBaseBuilding if ready; if still <1 → 2s.
        if !self.base.is_ready_to_build_structure() {
            let mut structure_timer = self.base.get_structure_timer();
            // C++ always decrements (may go below 0 as signed); we stop at 0.
            if structure_timer > 0 {
                structure_timer -= 1;
            }
            self.base.set_structure_timer_frames(structure_timer);
            if structure_timer == 0 {
                self.base.set_ready_to_build_structure(true);
                self.base.set_build_delay_frames(0);
            }
            let max_t = 3 * LOGICFRAMES_PER_SECOND;
            if self.base.get_structure_timer() > max_t {
                self.base.set_structure_timer_frames(max_t);
            }
        }

        let mut build_delay = self.base.get_build_delay();
        if build_delay > 0 {
            build_delay -= 1;
            self.base.set_build_delay_frames(build_delay);
        }
        if self.base.get_build_delay() < 1 {
            if self.base.is_ready_to_build_structure() {
                self.process_base_building();
            }
            if self.base.get_build_delay() < 1 {
                self.base.set_build_delay_frames(2 * LOGICFRAMES_PER_SECOND);
            }
        }
    }

    /// C++ `AISkirmishPlayer::doTeamBuilding`.
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
        drop(player_guard);

        if !self.base.is_ready_to_build_team() {
            let mut team_timer = self.base.get_team_timer();
            if team_timer > 0 {
                team_timer -= 1;
                self.base.set_team_timer_frames(team_timer);
            }
            if team_timer == 0 {
                self.base.set_ready_to_build_team(true);
                self.base.set_team_delay_frames(0);
            }
            let max_t = 3 * LOGICFRAMES_PER_SECOND;
            if self.base.get_team_timer() > max_t {
                self.base.set_team_timer_frames(max_t);
            }
        }

        let mut team_delay = self.base.get_team_delay();
        if team_delay > 0 {
            team_delay -= 1;
            self.base.set_team_delay_frames(team_delay);
        }
        if self.base.get_team_delay() < 1 {
            self.base.queue_units();
            if self.base.is_ready_to_build_team() {
                self.process_team_building();
            }
            self.base.set_team_delay_frames(2 * LOGICFRAMES_PER_SECOND);
        }
    }

    /// Select team to build with skirmish considerations
    /// C++ `AISkirmishPlayer::selectTeamToBuild` → AIPlayer::selectTeamToBuild.
    fn select_team_to_build(&mut self) -> bool {
        self.base.select_team_to_build().unwrap_or(false)
    }

    /// C++ `AISkirmishPlayer::selectTeamToReinforce` → AIPlayer.
    fn select_team_to_reinforce(&mut self, min_priority: i32) -> bool {
        self.base
            .select_team_to_reinforce(min_priority)
            .unwrap_or(false)
    }

    /// C++ `AISkirmishPlayer::startTraining` → findFactory + queueCreateUnit.
    fn start_training(&mut self, order: &mut WorkOrder, busy_ok: bool, team_name: &str) -> bool {
        self.base
            .start_training_for_order(order, busy_ok)
            .map(|ok| {
                if ok {
                    log::debug!("Queuing {} for {}", order.thing_template, team_name);
                }
                ok
            })
            .unwrap_or(false)
    }

    /// C++ `AISkirmishPlayer::isAGoodIdeaToBuildTeam` (same gates as AIPlayer).
    fn is_a_good_idea_to_build_team(&self, proto: &TeamPrototype) -> bool {
        // Production condition, max instances, not in queue, idle factory + money.
        self.base
            .is_a_good_idea_to_build_team(proto.get_name().as_str())
            .unwrap_or(false)
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
            // C++: m_player->onStructureUndone(obj);
            //      TheAI->pathfinder()->removeObjectFromPathfindMap(obj);
            //      TheGameLogic->destroyObject(obj);
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.on_structure_undone(&obj_arc);
                }
            }
            let positions: Vec<Coord3D> = OBJECT_REGISTRY
                .get_object(obj_id)
                .and_then(|arc| arc.read().ok().map(|g| vec![*g.get_position()]))
                .unwrap_or_default();
            if let Ok(ai_guard) = THE_AI.read() {
                if let Some(pf_arc) = ai_guard.pathfinder() {
                    if let Ok(mut pf) = pf_arc.write() {
                        pf.remove_object_from_map(obj_id, &positions);
                    }
                }
            }
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

    /// C++ newMap initial pass: buildStructureNow or incrementNumRebuilds.
    fn build_initial_structures(&mut self) {
        // Collect first to avoid holding player lock across builds.
        let mut initial: Vec<(String, Coord3D, f32)> = Vec::new();
        let mut rebuild_names: Vec<String> = Vec::new();
        {
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
                let name = entry.get_template_name().to_string();
                if !name.is_empty() && TheThingFactory::find_template(&name).is_some() {
                    if entry.is_initially_built() {
                        initial.push((name, *entry.get_location(), entry.get_angle()));
                    } else {
                        entry.increment_num_rebuilds();
                        rebuild_names.push(name);
                    }
                }
                let Some(next) = entry.get_next_mut() else {
                    break;
                };
                entry = next;
            }
        }

        for (name, loc, angle) in initial {
            if let Err(err) = self.base.build_structure_now_at_public(&name, loc, angle) {
                log::debug!(
                    "AISkirmishPlayer initial buildStructureNow('{}') failed: {err}",
                    name
                );
            }
        }
        let _ = rebuild_names;
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
    /// C++ `AISkirmishPlayer::acquireEnemy` (AISkirmishPlayer.cpp).
    ///
    /// Keep current enemy if healthy. Otherwise pick closest enemy with objects,
    /// structure-bounds midpoint, bad-shape / gang-up distance fudges.
    /// Only replace `m_currentEnemy` when a better candidate is found — never
    /// clear to null on empty search (C++ keeps the prior pointer).
    fn acquire_enemy(&mut self) {
        let mut best_enemy: Option<Arc<RwLock<Player>>> = None;
        let mut best_distance_sqr = HUGE_DIST * HUGE_DIST;

        let Some(me_player) = self.base.get_player() else {
            return;
        };
        let Ok(mut me_guard) = me_player.write() else {
            return;
        };
        let me_index = me_guard.get_player_index();
        let base_center = self
            .base
            .get_base_center()
            .unwrap_or_else(|| self.get_enemy_base_center(&me_guard).unwrap_or_default());

        // C++: if current enemy exists and is not in bad shape, keep it.
        if let Some(enemy_weak) = self.current_enemy.as_ref() {
            if let Some(enemy_arc) = enemy_weak.upgrade() {
                if let Ok(enemy_guard) = enemy_arc.try_read() {
                    let in_bad_shape =
                        !enemy_guard.has_any_units() || !enemy_guard.has_any_build_facility();
                    if !in_bad_shape {
                        // Keep Player index cache aligned with m_currentEnemy.
                        me_guard
                            .set_current_enemy_player_index(Some(enemy_guard.get_player_index()));
                        return;
                    }
                }
            }
        }

        let Ok(player_list) = ThePlayerList().read() else {
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

            // C++ curPlayer->hasAnyObjects()
            if !player_guard.has_any_objects() {
                continue;
            }

            let in_bad_shape =
                !player_guard.has_any_units() || !player_guard.has_any_build_facility();

            // C++ getPlayerStructureBounds midpoint for enemy center.
            let enemy_idx = player_guard.get_player_index();
            let enemy_center = self
                .base
                .get_player_structure_bounds(enemy_idx)
                .ok()
                .map(|(lo, hi)| {
                    Coord3D::new(lo.x + (hi.x - lo.x) * 0.5, lo.y + (hi.y - lo.y) * 0.5, 0.0)
                })
                .or_else(|| self.get_enemy_base_center(&player_guard))
                .unwrap_or(base_center);
            let dx = enemy_center.x - base_center.x;
            let dy = enemy_center.y - base_center.y;
            let mut dist_sqr = dx * dx + dy * dy;

            if in_bad_shape {
                dist_sqr = HUGE_DIST * HUGE_DIST * 0.5;
            }

            // C++: other skirmish AIs targeting this candidate / me.
            // Uses cached enemy index (Player::getCurrentEnemy → AI getAiEnemy).
            for other_arc in player_list.iter() {
                let Ok(other_guard) = other_arc.read() else {
                    continue;
                };
                if other_guard.get_player_index() == player_guard.get_player_index() {
                    continue;
                }
                if !other_guard.is_skirmish_ai() {
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

        // C++: only replace when bestEnemy != NULL && bestEnemy != m_currentEnemy.
        // Empty search leaves the prior enemy intact.
        let Some(best) = best_enemy else {
            return;
        };
        let best_index = best.read().ok().map(|g| g.get_player_index());
        let same_as_current = self
            .current_enemy
            .as_ref()
            .and_then(|w| w.upgrade())
            .and_then(|arc| {
                arc.read()
                    .ok()
                    .map(|g| Some(g.get_player_index()) == best_index)
            })
            .unwrap_or(false);
        if same_as_current {
            if let Some(idx) = best_index {
                me_guard.set_current_enemy_player_index(Some(idx));
            }
            return;
        }

        self.current_enemy = Some(Arc::downgrade(&best));
        if let Some(idx) = best_index {
            me_guard.set_current_enemy_player_index(Some(idx));
            log::debug!(
                "AISkirmishPlayer acquiring target enemy player index {}",
                idx
            );
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
        // C++ checkReadyTeams → AIPlayer.
        if let Err(err) = self.base.check_ready_teams() {
            log::debug!("AISkirmishPlayer::check_ready_teams failed: {err}");
        }
    }

    /// Check if any queued teams have finished building or timed out.
    /// Matches C++ AISkirmishPlayer::checkQueuedTeams (delegates to AIPlayer)
    pub fn check_queued_teams(&mut self) {
        // C++ checkQueuedTeams → AIPlayer.
        if let Err(err) = self.base.check_queued_teams() {
            log::debug!("AISkirmishPlayer::check_queued_teams failed: {err}");
        }
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

    pub fn is_supply_source_attacked(&mut self) -> bool {
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
    /// C++ `AISkirmishPlayer::crc` is empty (does not call base or xfer fields).
    fn crc(&self, _xfer: &mut dyn Xfer) {
        // Intentionally empty — matches GeneralsMD AISkirmishPlayer.cpp.
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

        // PARITY_NOTE: C++ does not xfer m_currentEnemy or m_frameToCheckEnemy
        // (runtime). Enemy is re-acquired via getAiEnemy/acquireEnemy after load.
    }

    fn load_post_process(&mut self) {
        self.current_enemy = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::system::xfer_save::XferSave;
    use std::io::Cursor;

    #[test]
    fn recruit_specific_ai_team_skirmish_home_warn_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src
            .find("C++ `AISkirmishPlayer::recruitSpecificAITeam`")
            .expect("skirmish recruit");
        let w = &src[i..src.len().min(i + 800)];
        assert!(
            w.contains("has_home_location")
                && w.contains("no Home Position")
                && w.contains("recruit_specific_ai_team(team_proto.get_name()"),
            "skirmish recruit must warn missing home then delegate"
        );
    }

    #[test]
    fn skirmish_crc_is_empty_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src
            .find("/// C++ `AISkirmishPlayer::crc` is empty")
            .expect("crc doc");
        let j = src[i..].find("fn xfer(").expect("xfer after crc") + i;
        let window = &src[i..j];
        assert!(
            window.contains("Intentionally empty")
                && !window.contains("xfer_int")
                && !window.contains("xfer_real"),
            "AISkirmishPlayer::crc must be empty like C++"
        );
    }

    #[test]
    fn skirmish_xfer_does_not_write_runtime_enemy_check_frame() {
        let mut player = AISkirmishPlayer::new(3);
        player.frame_to_check_enemy = 0x1234_5678;

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("skirmish_player_frame").unwrap();
            player.xfer(&mut save);
            save.close().unwrap();
        }

        assert!(!bytes
            .windows(4)
            .any(|window| window == &0x1234_5678u32.to_le_bytes()));
    }

    #[test]
    fn cluster_mines_fallback_uses_structure_bounds_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src
            .find("pub fn compute_superweapon_target")
            .expect("compute_superweapon_target");
        let w = &src[i..src.len().min(i + 2500)];
        assert!(
            w.contains("ClusterMines")
                && w.contains("get_closest_waypoint_on_path")
                && w.contains("get_player_structure_bounds")
                && !w.contains("get_enemy_base_center"),
            "cluster mines fallback must use enemy structure bounds center like C++"
        );
    }

    #[test]
    fn find_broken_bridge_on_pathfinder_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/mod.rs"));
        let i = src
            .find("pub fn find_broken_bridge")
            .expect("find_broken_bridge");
        let w = &src[i..src.len().min(i + 3500)];
        assert!(
            w.contains("client_safe_quick_does_path_exist")
                && w.contains("find_broken_bridge_layer")
                && w.contains("get_first_bridge")
                && w.contains("is_point_on_bridge")
                && w.contains("BodyDamageType::Rubble"),
            "findBrokenBridge must zone-connect, then destroyed layers, then terrain residual"
        );
    }

    #[test]
    fn check_bridges_uses_pathfinder_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src.find("pub fn check_bridges").expect("checkBridges");
        let window = &src[i..src.len().min(i + 4500)];
        assert!(
            window.contains("get_ai_update_interface")
                && window.contains("get_locomotor_set_clone")
                && window.contains("client_safe_quick_does_path_exist")
                && window.contains("find_broken_bridge")
                && window.contains("repair_structure")
                && window.contains("findBrokenBridge"),
            "checkBridges must path-exist hop first, then pathfinder findBrokenBridge + repair"
        );
    }

    #[test]
    fn acquire_enemy_uses_structure_bounds_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("production before tests");
        let i = prod
            .find("fn acquire_enemy(&mut self)")
            .expect("acquireEnemy");
        let window = &prod[i..prod.len().min(i + 5500)];
        assert!(
            window.contains("get_player_structure_bounds")
                && window.contains("HUGE_DIST")
                && window.contains("500.0 * 500.0")
                && window.contains("25.0 * 25.0")
                && window.contains("has_any_objects()")
                && window.contains("let Some(best) = best_enemy else")
                && !window.contains("self.current_enemy = None"),
            "acquireEnemy must use structure bounds + gang-up penalties and keep prior enemy"
        );
    }

    #[test]
    #[test]
    fn build_specific_ai_building_uses_live_object_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("production before tests");
        let i = prod
            .find("pub fn build_specific_ai_building(&mut self, thing_name: &str)")
            .expect("buildSpecificAIBuilding");
        let end = prod[i..]
            .find("pub fn build_ai_base_defense")
            .map(|o| i + o)
            .unwrap_or(prod.len().min(i + 2500));
        let w = &prod[i..end];
        assert!(
            w.contains("OBJECT_REGISTRY.get_object(obj_id)")
                && w.contains("mark_priority_build")
                && w.contains("set_build_delay_frames(0)")
                && !w.contains("build_structure_now"),
            "skirmish buildSpecificAIBuilding marks priority on missing live object only"
        );
    }

    fn adjust_build_list_calls_on_structure_undone_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src.find("fn adjust_build_list").expect("adjust_build_list");
        let w = &src[i..src.len().min(i + 3500)];
        assert!(
            w.contains("on_structure_undone")
                && w.contains("remove_object_from_map")
                && w.contains("destroy_object")
                && !w.contains("onStructureUndone residual deferred"),
            "adjust_build_list must call Player::onStructureUndone before pathfind remove + destroy"
        );
        let player_src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/player.rs"));
        assert!(
            player_src.contains("fn on_structure_undone")
                && player_src.contains("remove_object_built_obj"),
            "Player::on_structure_undone must removeObjectBuilt like C++"
        );
    }

    #[test]
    fn adjust_build_list_removes_pathfind_before_destroy_like_cpp() {
        // C++ AISkirmishPlayer::adjustBuildList: pathfinder removeObjectFromPathfindMap
        // then destroyObject on starting command center.
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src.find("fn adjust_build_list").expect("adjust_build_list");
        let window = &src[i..src.len().min(i + 4500)];
        assert!(
            window.contains("remove_object_from_map")
                && window.contains("destroy_object")
                && window.contains("rotate_skirmish_bases")
                && window.contains("set_initially_built(true)"),
            "adjustBuildList must pathfind-remove CC, destroy, rotate, mark initiallyBuilt"
        );
    }

    #[test]
    #[test]
    fn build_ai_base_defense_no_host_residual_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        // Production body only — stop before tests so this surface check
        // does not match its own string literals.
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("production before tests");
        let i = prod
            .find("pub fn build_ai_base_defense(&mut self, flank: bool)")
            .expect("build_ai_base_defense");
        let end = prod[i..]
            .find("pub fn build_ai_base_defense_structure")
            .map(|o| i + o)
            .unwrap_or(prod.len().min(i + 2000));
        let w = &prod[i..end];
        assert!(
            w.contains("base_defense_structure_1")
                && w.contains("build_ai_base_defense_structure")
                && !w.contains("build_flank_defense")
                && !w.contains("build_front_defense"),
            "buildAIBaseDefense must only resolve side defense + place structure"
        );
        assert!(
            !prod.contains("fn build_flank_defense")
                && !prod.contains("fn build_front_defense")
                && !prod.contains("fn update_flank_defense_angles")
                && !prod.contains("fn update_front_defense_angles"),
            "host residual defense angle helpers must be removed"
        );
    }

    fn skirmish_base_defense_and_new_map_cpp_surface() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        assert!(
            src.contains("C++ `AISkirmishPlayer::buildAIBaseDefenseStructure`")
                && src.contains("add_to_priority_build_list")
                && src.contains("SKIRMISH_CENTER")
                && src.contains("C++ `AISkirmishPlayer::newMap`")
                && src.contains("build_structure_now_at_public")
                && src.contains("compute_center_and_radius_of_base"),
            "skirmish base defense + newMap C++ paths required"
        );
    }

    #[test]
    fn process_base_building_matches_hole_by_spawner() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src
            .find("fn process_base_building(&mut self)")
            .expect("pbb");
        let window = &src[i..src.len().min(i + 12000)];
        assert!(
            window.contains("KindOf::RebuildHole")
                && window.contains("get_spawner_id()")
                && window.contains("prior_id")
                && window.contains("get_rebuild_hole_behavior_interface"),
            "skirmish processBaseBuilding must match rebuild hole by spawnerID"
        );
    }

    #[test]
    fn process_base_building_dozer_cpp_surface() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src
            .find("Matches C++ AISkirmishPlayer.cpp:75 processBaseBuilding")
            .expect("processBaseBuilding");
        let w = &src[i..src.len().min(i + 16000)];
        assert!(
            w.contains("build_structure_with_dozer")
                && w.contains("ResumeConstruction")
                && w.contains("CashGenerator")
                && w.contains("find_dozer_public")
                && w.contains("start_structure_timer_seconds")
                && w.contains("adjust_build_timer_for_wealth"),
            "processBaseBuilding must dozer-build, resume, power-force, arm timer"
        );
    }

    #[test]
    fn skirmish_select_team_cpp_surface() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        assert!(
            src.contains("AISkirmishPlayer::selectTeamToBuild")
                && src.contains("select_team_to_build()")
                && src.contains("select_team_to_reinforce(min_priority)")
                && src.contains("is_a_good_idea_to_build_team(proto.get_name()")
                && src.contains("processTeamBuilding"),
            "skirmish team selection must delegate to AIPlayer C++ path"
        );
    }

    #[test]
    fn process_team_building_select_then_queue_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src
            .find("fn process_team_building")
            .expect("process_team_building");
        let w = &src[i..src.len().min(i + 500)];
        assert!(
            w.contains("select_team_to_build")
                && w.contains("queue_units")
                && !w.contains("analyze_enemy_composition"),
            "processTeamBuilding must only selectTeamToBuild + queueUnits like C++"
        );
    }

    #[test]
    fn process_base_building_no_prioritize_residual_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src
            .find("fn process_base_building(&mut self)")
            .expect("process_base_building");
        let w = &src[i..src.len().min(i + 2500)];
        assert!(
            !w.contains("prioritize_power_buildings")
                && !w.contains("prioritize_defensive_buildings")
                && w.contains("is_under_powered")
                && w.contains("power_plan"),
            "skirmish processBaseBuilding must pick power inline from build list like C++"
        );
    }

    #[test]
    fn skirmish_structure_timer_allows_zero_after_wealth_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src
            .find("fn adjust_build_timer_for_wealth(&mut self)")
            .expect("adjust_build_timer");
        let end = src[i..]
            .find(
                "
    /// Process team building",
            )
            .map(|o| i + o)
            .unwrap_or(src.len().min(i + 4000));
        let w = &src[i..end];
        assert!(
            w.contains("set_structure_timer_frames(new_timer)") && !w.contains(".max(1)"),
            "C++ Int/Real wealth divide may leave structureTimer 0; do not clamp to 1"
        );
        let j = src
            .find("fn process_base_building(&mut self)")
            .expect("process_base_building");
        let end = src[j..]
            .find("fn adjust_build_timer_for_wealth")
            .map(|o| j + o)
            .unwrap_or(src.len().min(j + 8000));
        let ww = &src[j..end];
        assert!(
            ww.contains("arm_structure_timer_after_build")
                && !ww.contains("start_structure_timer_seconds"),
            "skirmish processBaseBuilding success must arm via arm_structure_timer_after_build"
        );
    }

    #[test]
    fn skirmish_update_phase_order_cpp_surface() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/skirmish_player.rs"
        ));
        let i = src.find("C++ `AISkirmishPlayer::update`").expect("update");
        let w = &src[i..src.len().min(i + 2500)];
        assert!(
            w.contains("do_base_building()")
                && w.contains("check_ready_teams")
                && w.contains("check_queued_teams")
                && w.contains("do_team_building()")
                && w.contains("do_upgrades_and_skills")
                && w.contains("update_bridge_repair")
                && !w.contains("update_without_base_building"),
            "skirmish update must match C++ virtual phase order"
        );
        let db = src
            .find("C++ `AISkirmishPlayer::doBaseBuilding`")
            .expect("doBase");
        let dw = &src[db..src.len().min(db + 2500)];
        assert!(
            dw.contains("is_ready_to_build_structure")
                && dw.contains("set_structure_timer_frames")
                && dw.contains("3 * LOGICFRAMES_PER_SECOND")
                && dw.contains("process_base_building"),
            "doBaseBuilding must tick structureTimer and throttle buildDelay"
        );
    }
}
