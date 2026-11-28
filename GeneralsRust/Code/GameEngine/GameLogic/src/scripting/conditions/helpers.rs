//! Shared helpers for script condition evaluation.

mod snapshot_queries;

use crate::scripting::ScriptValue;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::{Player, player_list};
use crate::scripting::engine::{get_named_object_tracker, get_script_engine};
use crate::scripting::events::GameEventType;
use crate::{GameLogicError, GameLogicResult};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// Host-path script query snapshot (IDs/poses only — no crate `Object`).
#[derive(Debug, Clone, Default)]
pub struct HostScriptQuerySnapshot {
    pub named: HashMap<String, u32>,
    pub team_ids: HashMap<u32, Vec<u32>>,
    pub objects: Vec<HostScriptQueryObject>,
    /// Named trigger-area AABBs (min_x, min_z, max_x, max_z). Circular/script
    /// pads only — map polygons are tested with `point_in_trigger_int`.
    pub areas: HashMap<String, (f32, f32, f32, f32)>,
    /// Script team-instance name → host object ids (C++ Team member list).
    pub team_instance_ids: HashMap<String, Vec<u32>>,
    /// Live host Player money/power/object census keyed by lowercase SIDE name.
    pub player_census: HashMap<String, HostScriptPlayerCensus>,
    /// World tech-building census for SKIRMISH_TECH_BUILDING_WITHIN_DISTANCE.
    pub tech_buildings: Vec<HostTechBuildingCensus>,
}

/// C++ KINDOF_TECH_BUILDING row for host leftover eval (pose + owner only).
#[derive(Debug, Clone, Default)]
pub struct HostTechBuildingCensus {
    pub x: f32,
    pub z: f32,
    pub owner_player: String,
    pub team: u32,
    pub off_map: bool,
}

pub use generals_sim::scripting::world::{HostScriptPlayerCensus, HostScriptQueryObject};

/// C++ the evaluating engine's TheScriptEngine host facts: the snapshot of the
/// world whose scripts are being evaluated. A query of no world has no facts.
#[derive(Clone)]
pub struct HostScriptQuery {
    world: Option<Arc<crate::system::engine_stores::WorldServices>>,
}

impl HostScriptQuery {
    /// The world whose facts these are (none for a worldless query).
    pub fn world(&self) -> Option<&Arc<crate::system::engine_stores::WorldServices>> {
        self.world.as_ref()
    }

    /// The host facts of `world`.
    pub fn of(world: &Arc<crate::system::engine_stores::WorldServices>) -> Self {
        Self { world: Some(Arc::clone(world)) }
    }

    /// No owning world: every query answers as an empty snapshot.
    pub fn none() -> Self {
        Self { world: None }
    }

    pub fn from_world(world: Option<Arc<crate::system::engine_stores::WorldServices>>) -> Self {
        Self { world }
    }

    /// Tests: the query of the world they published on this thread.
    #[cfg(test)]
    pub(crate) fn published_for_test() -> Self {
        Self::from_world(crate::system::engine_stores::try_active_services())
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut HostScriptQuerySnapshot) -> R) -> R {
        match self.world.as_deref() {
            Some(world) => {
                let mut slot = world
                    .host_script_query()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                f(&mut *slot)
            }
            None => f(&mut HostScriptQuerySnapshot::default()),
        }
    }

    /// Merge additional host-query rows into the current snapshot.
    pub fn merge_host_script_query_snapshot(&self, f: impl FnOnce(&mut HostScriptQuerySnapshot)) {
        self.with_mut(|slot| f(slot));
    }

    /// Inject a read-only host name/team/area query map for crate conditions.
    pub fn set_host_script_query_snapshot(&self, snap: HostScriptQuerySnapshot) {
        self.with_mut(|slot| *slot = snap);
    }

    pub fn clear_host_script_query_snapshot(&self) {
        self.with_mut(|slot| *slot = HostScriptQuerySnapshot::default());
    }

    pub fn host_script_named_unit_id(&self, name: &str) -> Option<u32> {
        self.with_mut(|slot| slot.named_unit_id(name))
    }

    /// True when a host snapshot was injected (any named/team/object/area/tech row).
    pub fn host_script_query_has_any(&self) -> bool {
        self.with_mut(|slot| slot.has_any())
    }

    /// Host named-unit aliveness from the snapshot (no crate Object).
    pub fn host_script_named_unit_alive(&self, name: &str) -> Option<bool> {
        self.with_mut(|slot| slot.named_unit_alive(name))
    }

    /// True when the host snapshot lists this named unit (alive or dying).
    pub fn host_script_named_unit_present(&self, name: &str) -> bool {
        self.host_script_named_unit_alive(name).is_some() || self.host_script_named_unit_id(name).is_some()
    }

    /// C++ evaluateNamedSelected: Object::getName() match on selected drawables.
    pub fn host_script_named_unit_selected(&self, name: &str) -> Option<bool> {
        if name.is_empty() {
            return None;
        }
        self.with_mut(|slot| {
            slot
                .objects
                .iter()
                .find(|o| o.name == name)
                .map(|o| o.selected)
        })
    }

    /// C++ AIGroup::isIdle / isGroupAiDead over host snapshot members.
    /// Empty group is idle and dead (vacuous C++ loops).
    pub fn host_team_sequential_status(&self, team_name: &str) -> (bool, bool) {
        let ids = self.host_script_team_member_ids(team_name);
        if ids.is_empty() {
            return (true, true);
        }
        let mut idle = true;
        let mut all_dead = true;
        self.with_mut(|slot| {
            let snap = slot;
            for id in ids {
                let Some(obj) = snap.objects.iter().find(|o| o.id == id) else {
                    continue;
                };
                let dead = obj.effectively_dead || !obj.alive;
                if !dead {
                    all_dead = false;
                }
                if !(obj.idle || dead) {
                    idle = false;
                }
            }
        });
        (idle, all_dead)
    }

    pub fn host_script_query_object(&self, name: &str) -> Option<HostScriptQueryObject> {
        if name.is_empty() {
            return None;
        }
        self.with_mut(|slot| {
            let snap = slot;
            if let Some(obj) = snap
                .objects
                .iter()
                .find(|o| o.name.eq_ignore_ascii_case(name))
            {
                return Some(obj.clone());
            }
            let id = snap
                .named
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, id)| *id)?;
            snap.objects.iter().find(|o| o.id == id).cloned()
        })
    }

    pub fn host_script_query_object_by_id(&self, id: u32) -> Option<HostScriptQueryObject> {
        self.with_mut(|slot| slot.object_by_id(id).cloned())
    }

    /// C++ evaluateBuildingEntered on the host snapshot (no leftover contain).
    /// None = named building missing from snapshot.
    pub fn host_building_entered_by_player(&self, building_name: &str, player_name: &str) -> Option<bool> {
        let obj = self.host_script_query_object(building_name)?;
        if !obj.has_contain {
            return Some(false);
        }
        if obj.player_who_entered.is_empty() {
            return Some(false);
        }
        Some(obj.player_who_entered.eq_ignore_ascii_case(player_name))
    }

    /// C++ Team::hasAnyObjects over host snapshot members.
    pub fn host_team_has_any_live_objects(&self, team_name: &str) -> bool {
        let ids: HashSet<u32> = self.host_script_team_member_ids(team_name).into_iter().collect();
        if ids.is_empty() {
            return false;
        }
        self.with_mut(|slot| {
            slot.objects.iter().any(|o| {
                ids.contains(&o.id) && o.alive && !o.kind_projectile && !o.kind_inert && !o.kind_mine
            })
        })
    }

    /// C++ Team::hasAnyUnits over host snapshot members.
    pub fn host_team_has_any_live_units(&self, team_name: &str) -> bool {
        let ids: HashSet<u32> = self.host_script_team_member_ids(team_name).into_iter().collect();
        if ids.is_empty() {
            return false;
        }
        self.with_mut(|slot| {
            slot.objects.iter().any(|o| {
                ids.contains(&o.id)
                    && o.alive
                    && !o.kind_structure
                    && !o.kind_projectile
                    && !o.kind_mine
            })
        })
    }

    /// True when the host snapshot lists any member ids for this team instance.
    pub fn host_team_was_fielded(&self, team_name: &str) -> bool {
        !self.host_script_team_member_ids(team_name).is_empty()
    }

    pub fn host_script_team_unit_ids(&self, team: u32) -> Vec<u32> {
        self.with_mut(|slot| slot.team_unit_ids(team).to_vec())
    }

    /// Live host Player census for leftover/live script player conditions.
    pub fn host_query_player_census(&self, player_name: &str) -> Option<HostScriptPlayerCensus> {
        // PlayerList::findPlayerWithNameKey ultimately compares the canonical
        // authored name key; unlike other string-backed query maps, this key is
        // intentionally exact and case-sensitive.
        if player_name.is_empty() {
            return None;
        }
        self.with_mut(|slot| slot.player_census.get(player_name).cloned())
    }

    /// C++ Player::countObjectsByThingTemplate from the live host census.
    /// `ignore_dead` matches C++ ignoreDead; under-construction is already excluded.
    pub fn host_query_player_template_count(&self, player_name: &str,
        type_names: &[String],
        ignore_dead: bool,) -> Option<i32> {
        let census = self.host_query_player_census(player_name)?;
        Some(census.count_templates(type_names, ignore_dead))
    }

    /// C++ Player::getSciencePurchasePoints from the live host census.
    pub fn host_query_player_science_purchase_points(&self, player_name: &str) -> Option<i32> {
        self.host_query_player_census(player_name).map(|c| c.science_purchase_points)
    }

    /// C++ Player::hasScience against host unlocked names.
    pub fn host_query_player_has_science(&self, player_name: &str, science_name: &str) -> Option<bool> {
        let census = self.host_query_player_census(player_name)?;
        let want = science_name.trim();
        if want.is_empty() {
            return Some(false);
        }
        Some(census.unlocked_sciences.iter().any(|owned| {
            owned.eq_ignore_ascii_case(want)
                || owned.eq_ignore_ascii_case(&format!("SCIENCE_{want}"))
                || format!("SCIENCE_{owned}").eq_ignore_ascii_case(want)
        }))
    }

    fn host_object_in_named_area(&self, obj: &HostScriptQueryObject, area_name: &str) -> bool {
        if let Some(trigger) = host_script_lookup_polygon_trigger(area_name) {
            return trigger.point_in_trigger_int(&host_xz_to_trigger_point(obj.x, obj.z));
        }
        if let Some((min_x, min_z, max_x, max_z)) = self.host_script_area_bounds(area_name) {
            return obj.x >= min_x && obj.x <= max_x && obj.z >= min_z && obj.z <= max_z;
        }
        false
    }

    /// C++ evaluatePlayerHasUnitTypeInArea count over host objects.
    pub fn host_count_player_type_in_area(&self, player_name: &str,
        area_name: &str,
        type_names: &[String],) -> Option<i32> {
        if !self.host_script_query_has_any() {
            return None;
        }
        if area_name.is_empty()
            && host_script_lookup_polygon_trigger(area_name).is_none()
            && self.host_script_area_bounds(area_name).is_none()
        {
            return None;
        }
        if host_script_lookup_polygon_trigger(area_name).is_none()
            && self.host_script_area_bounds(area_name).is_none()
        {
            return None;
        }
        Some(self.with_mut(|slot| {
            slot
                .objects
                .iter()
                .filter(|o| {
                    host_owner_matches(o, player_name)
                        && type_names
                            .iter()
                            .any(|t| o.template_name.eq_ignore_ascii_case(t))
                        && self.host_object_in_named_area(o, area_name)
                        && host_unit_counts_in_area(o, true)
                })
                .count() as i32
        }))
    }

    /// C++ evaluatePlayerHasUnitKindInArea count over host objects.
    pub fn host_count_player_kind_in_area(&self, player_name: &str,
        area_name: &str,
        kind: crate::common::KindOf,) -> Option<i32> {
        if !self.host_script_query_has_any() {
            return None;
        }
        if host_script_lookup_polygon_trigger(area_name).is_none()
            && self.host_script_area_bounds(area_name).is_none()
        {
            return None;
        }
        Some(self.with_mut(|slot| {
            slot
                .objects
                .iter()
                .filter(|o| {
                    host_owner_matches(o, player_name)
                        && host_object_has_kind(o, kind)
                        && self.host_object_in_named_area(o, area_name)
                        && host_unit_counts_in_area(o, false)
                })
                .count() as i32
        }))
    }

    /// C++ evaluateEnemySighted over host snapshot objects.
    pub fn host_enemy_sighted(&self, unit_name: &str, alliance: i32, player_name: &str) -> Option<bool> {
        if !self.host_script_query_has_any() {
            return None;
        }
        let looker = self.host_script_query_object(unit_name)?;
        if !looker.alive || looker.effectively_dead {
            return Some(false);
        }
        Some(self.with_mut(|slot| {
            slot.objects.iter().any(|cand| {
                if !host_sighted_candidate_ok(&looker, cand) {
                    return false;
                }
                if !host_owner_matches(cand, player_name) {
                    return false;
                }
                let rel = host_relationship(&looker, cand);
                match alliance {
                    0 => rel == crate::common::Relationship::Enemies,
                    1 => rel == crate::common::Relationship::Neutral,
                    2 => rel == crate::common::Relationship::Allies,
                    _ => false,
                }
            })
        }))
    }

    /// C++ evaluateTypeSighted over host snapshot objects.
    pub fn host_type_sighted(&self, unit_name: &str,
        type_names: &[String],
        player_name: &str,) -> Option<bool> {
        if !self.host_script_query_has_any() {
            return None;
        }
        let looker = self.host_script_query_object(unit_name)?;
        if !looker.alive || looker.effectively_dead {
            return Some(false);
        }
        Some(self.with_mut(|slot| {
            slot.objects.iter().any(|cand| {
                host_sighted_candidate_ok(&looker, cand)
                    && host_owner_matches(cand, player_name)
                    && type_names
                        .iter()
                        .any(|t| cand.template_name.eq_ignore_ascii_case(t))
            })
        }))
    }

    pub fn host_script_area_unit_ids(&self, min_x: f32, min_z: f32, max_x: f32, max_z: f32) -> Vec<u32> {
        self.with_mut(|slot| {
            slot
                .objects
                .iter()
                .filter(|o| o.alive && o.x >= min_x && o.x <= max_x && o.z >= min_z && o.z <= max_z)
                .map(|o| o.id)
                .collect()
        })
    }

    pub fn host_script_named_unit_in_area(&self, name: &str,
        min_x: f32,
        min_z: f32,
        max_x: f32,
        max_z: f32,) -> bool {
        self.with_mut(|slot| {
            slot
                .named_unit_in_area(name, (min_x, min_z, max_x, max_z))
        })
    }

    pub fn host_script_area_bounds(&self, area_name: &str) -> Option<(f32, f32, f32, f32)> {
        if area_name.is_empty() {
            return None;
        }
        self.with_mut(|slot| slot.areas.get(area_name).copied())
    }

    pub fn host_named_unit_point_in_trigger(&self, unit_name: &str,
        trigger: &crate::polygon_trigger::PolygonTrigger,) -> bool {
        self.with_mut(|slot| {
            slot
                .named_unit_point_in_trigger(unit_name, trigger)
        })
    }

    /// `Some(true/false)` when leftover polygon or host AABB geometry exists.
    /// C++ `evaluateNamedInsideArea` uses `pointInTrigger` on current position.
    pub fn host_script_named_unit_in_named_area(&self, unit_name: &str, area_name: &str) -> Option<bool> {
        if let Some(trigger) = host_script_lookup_polygon_trigger(area_name) {
            return Some(self.host_named_unit_point_in_trigger(unit_name, &trigger));
        }
        let (min_x, min_z, max_x, max_z) = self.host_script_area_bounds(area_name)?;
        Some(self.host_script_named_unit_in_area(
            unit_name, min_x, min_z, max_x, max_z,
        ))
    }

    /// Sync this world's host-object trigger mirror from its current query census.
    pub fn sync_host_trigger_flags_from_snapshot(&self, world: &mut HostTriggerWorld, frame: u32) {
        let snap = self.with_mut(|slot| slot.clone());
        world.set_current_frame(frame);
        for obj in &snap.objects {
            let team = snap
                .team_instance_ids
                .iter()
                .find_map(|(name, ids)| ids.contains(&obj.id).then_some(name.as_str()));
            world.update_object_flags(obj.id, obj.x, obj.z, frame, false, team);
        }
    }

    pub fn host_script_team_member_ids(&self, team_name: &str) -> Vec<u32> {
        if team_name.is_empty() {
            return Vec::new();
        }
        let mut ids = self.with_mut(|slot| {
            let snap = slot;
            snap.team_instance_ids
                .get(team_name)
                .cloned()
                .or_else(|| {
                    snap.team_instance_ids.iter().find_map(|(name, listed)| {
                        name.eq_ignore_ascii_case(team_name).then(|| listed.clone())
                    })
                })
                .unwrap_or_default()
        });
        if ids.is_empty() {
            if let Ok(factory) = crate::team::get_team_factory().lock() {
                for team in factory.find_team_instances(team_name) {
                    if let Ok(team_guard) = team.read() {
                        ids.extend(team_guard.get_members().iter().copied());
                    }
                }
            }
        }
        if ids.is_empty() {
            let ord = match team_name.to_ascii_lowercase().as_str() {
                "gla" => 0,
                "usa" | "america" => 1,
                "china" => 2,
                "neutral" => 3,
                _ => team_name.parse::<u32>().unwrap_or(u32::MAX),
            };
            ids = self.host_script_team_unit_ids(ord);
        }
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// C++ `ScriptConditions::evaluateTeamIsContained` over host snapshot members.
    /// `getContainedBy() != NULL`, plus leftover Exit-state pretend-contained.
    pub fn host_eval_team_is_contained(&self, team_name: &str, all_contained: bool) -> bool {
        let members = self.host_script_team_member_ids(team_name);
        if members.is_empty() {
            return false;
        }
        let mut any_considered = false;
        for id in members {
            let Some(obj) = self.host_script_query_object_by_id(id) else {
                continue;
            };
            let is_contained = obj.contained_by != 0 || obj.ai_exiting;
            if is_contained {
                if !all_contained {
                    return true;
                }
            } else if all_contained {
                return false;
            }
            any_considered = true;
        }
        if any_considered { all_contained } else { false }
    }

    /// C++ ScriptConditions::evaluateSkirmishCommandButtonIsReady over the host
    /// snapshot. `None` when no snapshot is injected (leftover OBJECT_REGISTRY path).
    pub fn host_eval_skirmish_command_button_ready(&self, team_name: &str,
        command_button_name: &str,
        all_ready: bool,
        control_bar: Option<&crate::control_bar::ControlBarBridge>,) -> Option<bool> {
        if !self.host_script_query_has_any() {
            return None;
        }
        if team_name.is_empty() {
            return Some(false);
        }

        match leftover_command_button_kind(command_button_name, control_bar) {
            LeftoverCommandButtonKind::Missing => return Some(false),
            LeftoverCommandButtonKind::Unknown
            | LeftoverCommandButtonKind::SpecialPower(_)
            | LeftoverCommandButtonKind::Upgrade
            | LeftoverCommandButtonKind::Other => {}
        }

        let ids = self.host_script_team_member_ids(team_name);
        if ids.is_empty() && !self.host_team_name_known(team_name) {
            return Some(false);
        }

        for id in ids {
            let Some(obj) = self.host_script_query_object_by_id(id) else {
                continue;
            };
            let Some(is_ready) = host_command_button_ready_for_object(&obj, command_button_name, control_bar) else {
                continue;
            };
            if is_ready {
                if !all_ready {
                    return Some(true);
                }
            } else if all_ready {
                return Some(false);
            }
        }
        Some(all_ready)
    }

    /// C++ ScriptConditions::evaluateSkirmishSpecialPowerIsReady over the host snapshot.
    pub fn host_eval_skirmish_special_power_ready(&self, player_name: &str, power_name: &str) -> Option<bool> {
        if !self.host_script_query_has_any() {
            return None;
        }
        if power_name.is_empty() {
            return Some(false);
        }
        Some(self.with_mut(|slot| {
            slot.objects.iter().any(|obj| {
                host_owner_matches(obj, player_name)
                    && host_object_has_special_power(obj, power_name)
                    && obj.special_power_ready
            })
        }))
    }

    /// C++ ScriptConditions::evaluateSkirmishValueInArea over the host snapshot.
    pub fn host_eval_skirmish_value_in_area(&self, player_name: &str,
        comparison: i32,
        money: i32,
        area_name: &str,) -> Option<bool> {
        if !self.host_script_query_has_any() {
            return None;
        }
        if host_script_lookup_polygon_trigger(area_name).is_none()
            && self.host_script_area_bounds(area_name).is_none()
        {
            return Some(false);
        }
        let total_cost = self.with_mut(|slot| {
            slot
                .objects
                .iter()
                .filter(|obj| {
                    host_owner_matches(obj, player_name)
                        && !obj.kind_inert
                        && obj.alive
                        && !obj.effectively_dead
                        && self.host_object_in_named_area(obj, area_name)
                })
                .map(|obj| obj.build_cost)
                .sum::<i32>()
        });
        Some(host_compare_i32(comparison, total_cost, money))
    }

    /// C++ evaluateSkirmishUnownedFactionUnitComparison count over host snapshot.
    pub fn host_eval_skirmish_unowned_faction_unit_count(&self) -> Option<i32> {
        if !self.host_script_query_has_any() {
            return None;
        }
        Some(self.with_mut(|slot| {
            slot
                .objects
                .iter()
                .filter(|obj| obj.unmanned)
                .count() as i32
        }))
    }

    /// C++ evaluateSkirmishPlayerHasDiscoveredPlayer over host discovered_by (shroud CLEAR|PARTIAL_CLEAR).
    pub fn host_eval_skirmish_player_has_discovered_player(&self, player_name: &str,
        discovered_by_name: &str,) -> Option<bool> {
        if !self.host_script_query_has_any() {
            return None;
        }
        Some(self.with_mut(|slot| {
            slot.objects.iter().any(|obj| {
                host_owner_matches(obj, player_name)
                    && obj
                        .discovered_by
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(discovered_by_name))
            })
        }))
    }

    /// C++ evaluateSkirmishPlayerHasPrereqsToBuild / ObjectTypes::canBuildAny using host census.
    pub fn host_eval_skirmish_player_has_prerequisite_to_build(&self, player_name: &str,
        type_name: &str,) -> Option<bool> {
        let census = self.host_query_player_census(player_name)?;
        let type_name = type_name.trim();
        if type_name.is_empty() {
            return Some(false);
        }
        let mut names = Vec::new();
        if let Some(found) = crate::scripting::engine::with_script_engine_ref(|engine| {
            engine.get_object_types(type_name)
        })
        .flatten()
        {
            names.extend(found.iter().map(|s| s.as_str().to_string()));
        }
        if names.is_empty() {
            names.push(type_name.to_string());
        }
        Some(
            names
                .iter()
                .any(|name| host_census_can_build_type(player_name, &census, name)),
        )
    }

    /// C++ evaluateSkirmishPlayerHasComparisonGarrisoned count over host snapshot.
    pub fn host_eval_skirmish_garrisoned_count(&self, player_name: &str) -> Option<i32> {
        if !self.host_script_query_has_any() {
            return None;
        }
        Some(self.with_mut(|slot| {
            slot
                .objects
                .iter()
                .filter(|obj| {
                    host_owner_matches(obj, player_name) && obj.garrisonable && obj.contain_count > 0
                })
                .count() as i32
        }))
    }

    /// C++ evaluateSkirmishPlayerHasComparisonCapturedUnits count over host snapshot.
    pub fn host_eval_skirmish_captured_count(&self, player_name: &str) -> Option<i32> {
        if !self.host_script_query_has_any() {
            return None;
        }
        Some(self.with_mut(|slot| {
            slot
                .objects
                .iter()
                .filter(|obj| host_owner_matches(obj, player_name) && obj.captured)
                .count() as i32
        }))
    }

    /// C++ ScriptConditions::evaluateSkirmishPlayerHasUnitsInArea over the host snapshot.
    pub fn host_eval_skirmish_player_has_units_in_area(&self, player_name: &str,
        area_name: &str,) -> Option<bool> {
        if !self.host_script_query_has_any() {
            return None;
        }
        if host_script_lookup_polygon_trigger(area_name).is_none()
            && self.host_script_area_bounds(area_name).is_none()
        {
            return Some(false);
        }
        Some(self.with_mut(|slot| {
            slot.objects.iter().any(|obj| {
                host_owner_matches(obj, player_name)
                    && self.host_object_in_named_area(obj, area_name)
                    && obj.alive
                    && !obj.effectively_dead
                    && !obj.kind_inert
                    && !obj.kind_projectile
            })
        }))
    }

    /// C++ ScriptConditions::evaluateSkirmishSuppliesWithinDistancePerimeter over
    /// the host warehouse census. `None` when no snapshot is injected.
    pub fn host_eval_skirmish_supplies_value_within_distance(&self, player_name: &str,
        extra_distance: f32,
        area_name: &str,
        compare_value: f32,) -> Option<bool> {
        if !self.host_script_query_has_any() {
            return None;
        }
        if player_name.trim().is_empty() {
            return Some(false);
        }
        let Some((center_x, center_y, trigger_radius)) = self.host_trigger_center_radius(area_name) else {
            return Some(false);
        };
        let radius = trigger_radius + extra_distance;
        let radius_sq = radius * radius;
        let supply_box_value = self.host_query_player_census(player_name)
            .map(|c| c.supply_box_value)
            .filter(|&v| v > 0)
            .unwrap_or(75) as f32;
        let mut max_value = 0.0f32;
        self.with_mut(|slot| {
            for obj in slot.objects.iter() {
                if !obj.is_supply_warehouse || !obj.kind_structure || obj.off_map {
                    continue;
                }
                if !host_allow_neutral_affiliation(player_name, obj) {
                    continue;
                }
                let dx = obj.x - center_x;
                let dy = obj.z - center_y;
                if dx * dx + dy * dy > radius_sq {
                    continue;
                }
                let value = supply_box_value * obj.warehouse_boxes as f32;
                if value > max_value {
                    max_value = value;
                }
            }
        });
        Some(max_value > compare_value)
    }

    /// C++ PolygonTrigger::getCenterPoint + getRadius, else host AABB circumradius.
    fn host_trigger_center_radius(&self, area_name: &str) -> Option<(f32, f32, f32)> {
        if let Some(trigger) = host_script_lookup_polygon_trigger(area_name) {
            let center = crate::polygon_trigger::terrain_center(&trigger);
            return Some((center.x, center.y, trigger.get_radius()));
        }
        let (min_x, min_z, max_x, max_z) = self.host_script_area_bounds(area_name)?;
        let cx = (min_x + max_x) * 0.5;
        let cz = (min_z + max_z) * 0.5;
        let hx = (max_x - min_x) * 0.5;
        let hz = (max_z - min_z) * 0.5;
        Some((cx, cz, (hx * hx + hz * hz).sqrt()))
    }

    /// C++ evaluateSkirmishPlayerTechBuildingWithinDistancePerimeter over the host
    /// tech census. `None` when no snapshot or trigger geometry (do not latch).
    pub fn host_eval_skirmish_tech_building_within_distance(&self, player_name: &str,
        extra_distance: f32,
        area_name: &str,) -> Option<bool> {
        if !self.host_script_query_has_any() {
            return None;
        }
        let (center_x, center_y, trigger_radius) =
            self.host_tech_building_search_circle(area_name, player_name)?;
        let radius = trigger_radius + extra_distance;
        let radius_sq = radius * radius;
        self.with_mut(|slot| {
            let snap = slot;
            if snap.tech_buildings.is_empty() && snap.objects.is_empty() {
                return None;
            }
            if !snap.tech_buildings.is_empty() {
                return Some(snap.tech_buildings.iter().any(|tb| {
                    if tb.off_map {
                        return false;
                    }
                    if !host_allow_non_ally_tech_affiliation(player_name, &tb.owner_player) {
                        return false;
                    }
                    let dx = tb.x - center_x;
                    let dy = tb.z - center_y;
                    dx * dx + dy * dy <= radius_sq
                }));
            }
            Some(snap.objects.iter().any(|obj| {
                if obj.off_map || !obj.alive || obj.effectively_dead {
                    return false;
                }
                if !host_object_has_kind(obj, crate::common::KindOf::TechBuilding) {
                    return false;
                }
                if !host_allow_non_ally_tech_affiliation(player_name, &obj.owner_player) {
                    return false;
                }
                let dx = obj.x - center_x;
                let dy = obj.z - center_y;
                dx * dx + dy * dy <= radius_sq
            }))
        })
    }

    /// C++ getQualifiedTriggerAreaByName + getCenterPoint/getRadius.
    /// Qualifies [Skirmish]MyInnerPerimeter with the SIDE player, then script engine.
    fn host_tech_building_search_circle(&self, area_name: &str, player_name: &str) -> Option<(f32, f32, f32)> {
        let resolved = crate::scripting::engine::qualify_trigger_area_name(
            area_name,
            (!player_name.is_empty()).then_some(player_name),
        )
        .unwrap_or_else(|| area_name.to_string());
        let from_terrain = |name: &str| {
            crate::terrain::get_terrain_logic()
                .read()
                .ok()
                .and_then(|terrain| terrain.get_trigger_area_by_name(name).cloned())
        };
        if let Some(trigger) = from_terrain(&resolved).or_else(|| from_terrain(area_name)) {
            let center = crate::polygon_trigger::terrain_center(&trigger);
            return Some((center.x, center.y, trigger.get_radius()));
        }
        if let Some(trigger) = crate::scripting::engine::with_script_engine_ref(|engine| {
            engine.get_qualified_trigger_area_by_name(area_name)
        })
        .flatten()
        {
            let center = crate::polygon_trigger::terrain_center(&trigger);
            return Some((center.x, center.y, trigger.get_radius()));
        }
        let (min_x, min_z, max_x, max_z) =
            self.host_script_area_bounds(&resolved).or_else(|| self.host_script_area_bounds(area_name))?;
        let cx = (min_x + max_x) * 0.5;
        let cz = (min_z + max_z) * 0.5;
        let hx = (max_x - min_x) * 0.5;
        let hz = (max_z - min_z) * 0.5;
        Some((cx, cz, (hx * hx + hz * hz).sqrt()))
    }

    /// C++ ScriptConditions::evaluateUnitHasObjectStatus over the host snapshot.
    /// `None` when no snapshot is injected (leftover OBJECT_REGISTRY path).
    pub fn host_eval_unit_has_object_status(&self, unit_name: &str, status_mask: u64) -> Option<bool> {
        if !self.host_script_query_has_any() {
            return None;
        }
        let Some(obj) = self.host_script_query_object(unit_name) else {
            return Some(false);
        };
        Some(host_object_has_status_bits(&obj, status_mask))
    }

    fn host_team_name_known(&self, team_name: &str) -> bool {
        self.with_mut(|slot| {
            slot
                .team_instance_ids
                .keys()
                .any(|name| name.eq_ignore_ascii_case(team_name))
        })
    }

    fn host_team_area_members(&self, team_name: &str, which_to_consider: u32) -> Vec<(u32, f32, f32)> {
        let ids: HashSet<u32> = self.host_script_team_member_ids(team_name).into_iter().collect();
        self.with_mut(|slot| {
            slot
                .objects
                .iter()
                .filter(|obj| {
                    ids.contains(&obj.id) && host_object_counts_for_team_area(obj, which_to_consider)
                })
                .map(|obj| (obj.id, obj.x, obj.z))
                .collect()
        })
    }

    pub fn host_team_all_inside(&self, _world: &HostTriggerWorld,
        team_name: &str,
        trigger: &crate::polygon_trigger::PolygonTrigger,
        which_to_consider: u32,) -> bool {
        let members = self.host_team_area_members(team_name, which_to_consider);
        !members.is_empty()
            && members
                .iter()
                .all(|(_, x, z)| trigger.point_in_trigger_int(&host_xz_to_trigger_point(*x, *z)))
    }

    pub fn host_team_some_inside_some_outside(&self, _world: &HostTriggerWorld,
        team_name: &str,
        trigger: &crate::polygon_trigger::PolygonTrigger,
        which_to_consider: u32,) -> bool {
        let members = self.host_team_area_members(team_name, which_to_consider);
        let mut any_inside = false;
        let mut any_outside = false;
        for (_, x, z) in members {
            if trigger.point_in_trigger_int(&host_xz_to_trigger_point(x, z)) {
                any_inside = true;
            } else {
                any_outside = true;
            }
        }
        any_inside && any_outside
    }

    pub fn host_team_did_all_enter(&self, world: &HostTriggerWorld,
        team_name: &str,
        trigger: &crate::polygon_trigger::PolygonTrigger,
        which_to_consider: u32,
        now: u32,) -> bool {
        if !host_team_did_enter_or_exit(self, world, team_name, now) {
            return false;
        }
        let members = self.host_team_area_members(team_name, which_to_consider);
        let mut entered = false;
        let mut outside = false;
        for (id, x, z) in members {
            if host_object_did_enter(world, id, trigger, now) {
                entered = true;
            } else if !trigger.point_in_trigger_int(&host_xz_to_trigger_point(x, z)) {
                outside = true;
            }
        }
        entered && !outside
    }

    pub fn host_team_did_partial_enter(&self, world: &HostTriggerWorld,
        team_name: &str,
        trigger: &crate::polygon_trigger::PolygonTrigger,
        which_to_consider: u32,
        now: u32,) -> bool {
        if !host_team_did_enter_or_exit(self, world, team_name, now) {
            return false;
        }
        self.host_team_area_members(team_name, which_to_consider)
            .into_iter()
            .any(|(id, _, _)| host_object_did_enter(world, id, trigger, now))
    }

    pub fn host_team_did_all_exit(&self, world: &HostTriggerWorld,
        team_name: &str,
        trigger: &crate::polygon_trigger::PolygonTrigger,
        which_to_consider: u32,
        now: u32,) -> bool {
        if !host_team_did_enter_or_exit(self, world, team_name, now) {
            return false;
        }
        let members = self.host_team_area_members(team_name, which_to_consider);
        let mut exited = false;
        let mut inside = false;
        let mut any = false;
        for (id, x, z) in members {
            any = true;
            if host_object_did_exit(world, id, trigger, now) {
                exited = true;
            } else if trigger.point_in_trigger_int(&host_xz_to_trigger_point(x, z)) {
                inside = true;
            }
        }
        any && exited && !inside
    }

    pub fn host_team_did_partial_exit(&self, world: &HostTriggerWorld,
        team_name: &str,
        trigger: &crate::polygon_trigger::PolygonTrigger,
        which_to_consider: u32,
        now: u32,) -> bool {
        if !host_team_did_enter_or_exit(self, world, team_name, now) {
            return false;
        }
        self.host_team_area_members(team_name, which_to_consider)
            .into_iter()
            .any(|(id, _, _)| host_object_did_exit(world, id, trigger, now))
    }
}

fn host_kind_token(name: &str) -> String {
    name.trim()
        .trim_start_matches("KINDOF_")
        .trim_start_matches("KindOf_")
        .replace('_', "")
        .to_ascii_lowercase()
}

/// Match a leftover KindOf against host KindOf Debug names.
pub fn host_object_has_kind(obj: &HostScriptQueryObject, kind: crate::common::KindOf) -> bool {
    let want = host_kind_token(&format!("{kind:?}"));
    obj.kind_names.iter().any(|n| host_kind_token(n) == want)
        || match kind {
            crate::common::KindOf::Structure => obj.kind_structure,
            crate::common::KindOf::Projectile => obj.kind_projectile,
            crate::common::KindOf::Inert => obj.kind_inert,
            crate::common::KindOf::Mine => obj.kind_mine,
            crate::common::KindOf::Crate => {
                obj.kind_names.iter().any(|n| host_kind_token(n) == "crate")
            }
            _ => false,
        }
}
fn host_owner_matches(obj: &HostScriptQueryObject, player_name: &str) -> bool {
    let want = player_name.trim();
    if want.is_empty() || obj.owner_player.is_empty() {
        return false;
    }
    if obj.owner_player.eq_ignore_ascii_case(want) {
        return true;
    }
    let norm = |s: &str| {
        s.trim()
            .trim_start_matches("plyr")
            .trim_start_matches("player_")
            .trim_start_matches("player")
            .replace([' ', '_', '-'], "")
            .to_ascii_lowercase()
    };
    let a = norm(&obj.owner_player);
    let b = norm(want);
    !a.is_empty() && a == b
}



fn host_unit_counts_in_area(obj: &HostScriptQueryObject, include_crate: bool) -> bool {
    let dead_or_inert = obj.effectively_dead || !obj.alive || obj.kind_inert;
    if include_crate && host_object_has_kind(obj, crate::common::KindOf::Crate) {
        return true;
    }
    !dead_or_inert
}

fn host_relationship(
    looker: &HostScriptQueryObject,
    candidate: &HostScriptQueryObject,
) -> crate::common::Relationship {
    if let (Ok(players), true, true) = (
        player_list().read(),
        !looker.owner_player.is_empty(),
        !candidate.owner_player.is_empty(),
    ) {
        if let (Some(a), Some(b)) = (
            players.find_player_by_name(&looker.owner_player),
            players.find_player_by_name(&candidate.owner_player),
        ) {
            if let (Ok(look), Ok(them)) = (a.read(), b.read()) {
                return look.get_relationship(&them);
            }
        }
    }
    if looker
        .owner_player
        .eq_ignore_ascii_case(&candidate.owner_player)
        && !looker.owner_player.is_empty()
    {
        return crate::common::Relationship::Allies;
    }
    const HOST_NEUTRAL_TEAM: u32 = 3;
    if looker.team == HOST_NEUTRAL_TEAM || candidate.team == HOST_NEUTRAL_TEAM {
        return crate::common::Relationship::Neutral;
    }
    if looker.team == candidate.team {
        crate::common::Relationship::Allies
    } else {
        crate::common::Relationship::Enemies
    }
}

fn host_sighted_candidate_ok(
    looker: &HostScriptQueryObject,
    candidate: &HostScriptQueryObject,
) -> bool {
    if candidate.id == looker.id {
        return false;
    }
    if !candidate.alive || candidate.effectively_dead {
        return false;
    }
    if candidate.stealthed_hidden {
        return false;
    }
    let dx = candidate.x - looker.x;
    let dz = candidate.z - looker.z;
    dx * dx + dz * dz <= looker.vision_range * looker.vision_range
}

/// Host XZ → C++ trigger XY (`Object.cpp:2572-2574`, host Y-up).
pub fn host_xz_to_trigger_point(x: f32, z: f32) -> crate::common::ICoord3D {
    crate::common::ICoord3D::new(x as i32, z as i32, 0)
}

/// Leftover `TerrainLogic` polygon by qualified name.
pub fn host_script_lookup_polygon_trigger(
    area_name: &str,
) -> Option<crate::polygon_trigger::PolygonTrigger> {
    if area_name.is_empty() {
        return None;
    }
    let resolved = crate::scripting::engine::qualify_trigger_area_name(area_name, None)
        .unwrap_or_else(|| area_name.to_string());
    let terrain_owner_handle = crate::terrain::get_terrain_logic();
    let terrain = terrain_owner_handle.read().ok()?;
    terrain
        .get_trigger_area_by_name(&resolved)
        .or_else(|| terrain.get_trigger_area_by_name(area_name))
        .cloned()
}

pub use generals_sim::scripting::{HostObjectTriggerPersist, HostTriggerSlotPersist};
pub use generals_sim::scripting::context::ScriptTriggerWorld as HostTriggerWorld;

pub fn host_object_did_enter(
    world: &HostTriggerWorld,
    object_id: u32,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    now: u32,
) -> bool {
    world.did_enter(object_id, trigger, now)
}

pub fn host_object_did_exit(
    world: &HostTriggerWorld,
    object_id: u32,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    now: u32,
) -> bool {
    world.did_exit(object_id, trigger, now)
}

bitflags::bitflags! {
    /// Ground and air bits authored by World Builder in a SurfacesAllowed parameter.
    #[derive(Clone, Copy)]
    pub struct ScriptSurfacesAllowed: u32 {
        const GROUND = 0x01;
        const AIR = 0x02;
    }
}

impl ScriptSurfacesAllowed {
    /// Convert the authored bits to the locomotor mask (Team.cpp:142–145).
    pub const fn locomotor_mask(self) -> crate::locomotor::LocomotorSurfaceTypeMask {
        use crate::locomotor::{SURFACE_AIR, SURFACE_GROUND};

        let mut mask = 0;
        if self.contains(Self::GROUND) {
            mask |= SURFACE_GROUND;
        }
        if self.contains(Self::AIR) {
            mask |= SURFACE_AIR;
        }
        mask
    }
}

fn leftover_template_is_inert(template_name: &str) -> bool {
    if template_name.is_empty() {
        return false;
    }
    game_engine::common::thing::thing_factory::try_get_thing_factory()
        .and_then(|guard| {
            guard
                .as_ref()
                .and_then(|factory| factory.find_template(template_name, false))
        })
        .is_some_and(|template| {
            template.is_kind_of_mask(game_engine::common::system::kind_of::KindOfMask::INERT.bits())
        })
}

/// C++ Team::didAllEnter member filter: loco surfaces, dead, KINDOF_INERT.
fn host_object_counts_for_team_area(obj: &HostScriptQueryObject, which_to_consider: u32) -> bool {
    let surfaces = if obj.locomotor_surfaces != 0 {
        obj.locomotor_surfaces
    } else {
        crate::path::SURFACE_GROUND
    };
    let considered = ScriptSurfacesAllowed::from_bits_truncate(which_to_consider).locomotor_mask();
    if surfaces & considered == 0 {
        return false;
    }
    if obj.effectively_dead || !obj.alive {
        return false;
    }
    if obj.kind_inert
        || host_object_has_kind(obj, crate::common::KindOf::Inert)
        || leftover_template_is_inert(&obj.template_name)
    {
        return false;
    }
    true
}

fn host_compare_i32(comparison: i32, actual: i32, target: i32) -> bool {
    match comparison {
        0 => actual < target,
        1 => actual <= target,
        2 => actual == target,
        3 => actual >= target,
        4 => actual > target,
        5 => actual != target,
        _ => false,
    }
}

fn host_object_has_special_power(obj: &HostScriptQueryObject, power_name: &str) -> bool {
    obj.special_power_templates.iter().any(|owned| {
        owned.eq_ignore_ascii_case(power_name)
            || host_special_power_matches_button(owned, power_name)
    })
}

fn host_census_can_build_type(
    player_name: &str,
    census: &HostScriptPlayerCensus,
    type_name: &str,
) -> bool {
    let Some(template) = crate::helpers::TheThingFactory::find_template(type_name) else {
        return false;
    };
    if let Some(status) = template.get_buildable_status() {
        use game_engine::common::thing::BuildableStatus;
        match status {
            BuildableStatus::No => return false,
            BuildableStatus::IgnorePrerequisites => return true,
            BuildableStatus::OnlyByAi | BuildableStatus::Yes => {}
        }
    }
    for prereq in template.get_production_prerequisites() {
        if !prereq.is_satisfied_with_counter(
            |science| leftover_census_or_player_has_science(player_name, census, science),
            |handles, ignore_dead, counts| {
                for (i, handle) in handles.iter().enumerate() {
                    if i >= counts.len() {
                        break;
                    }
                    counts[i] =
                        crate::helpers::TheThingFactory::find_template_by_id(handle.value())
                            .map(|tpl| {
                                census.count_templates([tpl.get_name().as_str()], ignore_dead)
                            })
                            .unwrap_or(0);
                }
            },
        ) {
            return false;
        }
    }
    true
}

fn leftover_census_or_player_has_science(
    player_name: &str,
    census: &HostScriptPlayerCensus,
    science: game_engine::common::rts::ScienceType,
) -> bool {
    if science == game_engine::common::rts::SCIENCE_INVALID {
        return true;
    }
    let __player_list_arc = player_list();
        if let Ok(list) = __player_list_arc.read() {
        if let Some(arc) = list.find_player_by_name(player_name) {
            if let Ok(player) = arc.read() {
                return player.has_science(science);
            }
        }
    }
    if let Some(store) = game_engine::common::rts::science::get_science_store() {
        for (stored, info) in store.iter() {
            if *stored == science {
                let display = info.name.to_string();
                if census.unlocked_sciences.iter().any(|owned| {
                    owned.eq_ignore_ascii_case(&display)
                        || owned.eq_ignore_ascii_case(&format!("SCIENCE_{display}"))
                }) {
                    return true;
                }
            }
        }
        if store.is_empty() {
            return true;
        }
    }
    !census.unlocked_sciences.is_empty()
}

/// C++ PartitionFilterPlayerAffiliation(player, ALLOW_NEUTRAL, true).
fn host_allow_neutral_affiliation(player_name: &str, obj: &HostScriptQueryObject) -> bool {
    if host_owner_matches(obj, player_name) {
        return true;
    }
    const HOST_NEUTRAL_TEAM: u32 = 3;
    if obj.team == HOST_NEUTRAL_TEAM {
        return true;
    }
    if obj.owner_player.is_empty() {
        return true;
    }
    let __player_list_arc = player_list();
        if let Ok(players) = __player_list_arc.read() {
        if let (Some(a), Some(b)) = (
            players.find_player_by_name(player_name),
            players.find_player_by_name(&obj.owner_player),
        ) {
            if let (Ok(look), Ok(them)) = (a.read(), b.read()) {
                return look.get_relationship(&them) == crate::common::Relationship::Neutral;
            }
        }
    }
    false
}

/// C++ PartitionFilterPlayerAffiliation(ALLOW_ALLIES, false) + PartitionFilterPlayer(false).
fn host_allow_non_ally_tech_affiliation(player_name: &str, owner_player: &str) -> bool {
    if owner_player.is_empty() {
        return true;
    }
    let owner = HostScriptQueryObject {
        owner_player: owner_player.to_string(),
        ..Default::default()
    };
    if host_owner_matches(&owner, player_name) {
        return false;
    }
    let __player_list_arc = player_list();
        if let Ok(players) = __player_list_arc.read() {
        if let (Some(a), Some(b)) = (
            players.find_player_by_name(player_name),
            players.find_player_by_name(owner_player),
        ) {
            if let (Ok(look), Ok(them)) = (a.read(), b.read()) {
                return !matches!(
                    look.get_relationship(&them),
                    crate::common::Relationship::Allies
                );
            }
        }
    }
    true
}

fn host_object_has_status_bits(obj: &HostScriptQueryObject, mask: u64) -> bool {
    mask != 0 && (obj.status_bits & mask) != 0
}

enum LeftoverCommandButtonKind {
    /// Leftover ControlBar is populated and the name is absent. C++ false.
    Missing,
    /// No leftover catalog — live host matches snapshot special-power names.
    Unknown,
    SpecialPower(String),
    Upgrade,
    Other,
}

/// `control_bar` is the evaluating engine's ControlBar (C++ TheControlBar of
/// the engine running the script), never the published world's.
fn leftover_command_button_kind(
    command_button_name: &str,
    control_bar: Option<&crate::control_bar::ControlBarBridge>,
) -> LeftoverCommandButtonKind {
    if let Some(bridge) = control_bar {
        if let Some(button) = bridge.find_command_button_by_name(command_button_name) {
            if let Some(template) = button.get_special_power_template() {
                return LeftoverCommandButtonKind::SpecialPower(template.get_name().to_string());
            }
            if button.get_upgrade_template().is_some() {
                return LeftoverCommandButtonKind::Upgrade;
            }
            return LeftoverCommandButtonKind::Other;
        }
        // C++ findCommandButton over a populated ControlBar: absent is false.
        if bridge.has_command_buttons() {
            return LeftoverCommandButtonKind::Missing;
        }
    }
    // No ControlBar of the evaluating engine: nothing to resolve against.
    LeftoverCommandButtonKind::Unknown
}



fn host_command_button_ready_for_object(
    obj: &HostScriptQueryObject,
    command_button_name: &str,
    control_bar: Option<&crate::control_bar::ControlBarBridge>,
) -> Option<bool> {
    match leftover_command_button_kind(command_button_name, control_bar) {
        LeftoverCommandButtonKind::Missing => None,
        LeftoverCommandButtonKind::Upgrade | LeftoverCommandButtonKind::Other => {
            // C++ does not skip upgrade members; isReady needs leftover Object.
            Some(false)
        }
        LeftoverCommandButtonKind::SpecialPower(template) => {
            if !obj
                .special_power_templates
                .iter()
                .any(|owned| owned.eq_ignore_ascii_case(&template))
            {
                return None;
            }
            Some(obj.special_power_ready)
        }
        LeftoverCommandButtonKind::Unknown => {
            if obj.special_power_templates.is_empty() {
                return None;
            }
            if !obj
                .special_power_templates
                .iter()
                .any(|owned| host_special_power_matches_button(owned, command_button_name))
            {
                return None;
            }
            Some(obj.special_power_ready)
        }
    }
}

fn host_special_power_matches_button(template: &str, button_name: &str) -> bool {
    let template_key = host_command_identity_token(template);
    let button_key = host_command_identity_token(button_name);
    !template_key.is_empty()
        && !button_key.is_empty()
        && (template_key == button_key
            || template_key.contains(&button_key)
            || button_key.contains(&template_key))
}

fn host_command_identity_token(name: &str) -> String {
    name.trim()
        .trim_start_matches("Command_")
        .trim_start_matches("COMMAND_")
        .trim_start_matches("Superweapon")
        .trim_start_matches("SpecialPower")
        .replace('_', "")
        .to_ascii_lowercase()
}

pub fn host_team_did_enter_or_exit(
    q: &HostScriptQuery,
    world: &HostTriggerWorld,
    team_name: &str,
    now: u32,
) -> bool {
    world.team_did_enter_or_exit(team_name, now, || q.host_script_team_member_ids(team_name))
}



//-------------------------------------------------------------------------------------------------
// Helper: parse object status mask from string name
//-------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tech_building_census_tests {
    use super::*;

    #[test]
    fn host_census_tech_building_none_without_snapshot() {
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
        assert_eq!(
            crate::scripting::HostScriptQuery::published_for_test().host_eval_skirmish_tech_building_within_distance("PlyrAmerica", 100.0, "HomeBase"),
            None
        );
    }

    #[test]
    fn host_census_tech_building_finds_neutral_derrick() {
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
        let mut snap = HostScriptQuerySnapshot::default();
        snap.areas.insert("HomeBase".into(), (0.0, 0.0, 20.0, 20.0));
        snap.tech_buildings.push(HostTechBuildingCensus {
            x: 10.0,
            z: 10.0,
            owner_player: String::new(),
            team: 3,
            off_map: false,
        });
        crate::scripting::HostScriptQuery::published_for_test().set_host_script_query_snapshot(snap);
        assert_eq!(
            crate::scripting::HostScriptQuery::published_for_test().host_eval_skirmish_tech_building_within_distance("PlyrAmerica", 100.0, "HomeBase"),
            Some(true)
        );

        let mut own = HostScriptQuerySnapshot::default();
        own.areas.insert("HomeBase".into(), (0.0, 0.0, 20.0, 20.0));
        own.tech_buildings.push(HostTechBuildingCensus {
            x: 10.0,
            z: 10.0,
            owner_player: "PlyrAmerica".into(),
            team: 0,
            off_map: false,
        });
        crate::scripting::HostScriptQuery::published_for_test().set_host_script_query_snapshot(own);
        assert_eq!(
            crate::scripting::HostScriptQuery::published_for_test().host_eval_skirmish_tech_building_within_distance("PlyrAmerica", 100.0, "HomeBase"),
            Some(false)
        );
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
    }

    #[test]
    fn host_census_tech_building_missing_area_is_none() {
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
        let mut snap = HostScriptQuerySnapshot::default();
        snap.tech_buildings.push(HostTechBuildingCensus {
            x: 10.0,
            z: 10.0,
            ..Default::default()
        });
        crate::scripting::HostScriptQuery::published_for_test().set_host_script_query_snapshot(snap);
        assert_eq!(
            crate::scripting::HostScriptQuery::published_for_test().host_eval_skirmish_tech_building_within_distance("PlyrAmerica", 100.0, "MissingPad"),
            None
        );
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
    }
}

#[cfg(test)]
mod host_skirmish_discovered_prereq_tests {
    use super::*;

    #[test]
    fn host_skirmish_discovered_none_without_snapshot() {
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
        assert_eq!(
            crate::scripting::HostScriptQuery::published_for_test().host_eval_skirmish_player_has_discovered_player("PlyrChina", "PlyrAmerica"),
            None
        );
    }

    #[test]
    fn host_player_census_uses_exact_case_sensitive_player_name_key() {
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
        let mut snap = HostScriptQuerySnapshot::default();
        snap.player_census.insert(
            "AuthoredSide".into(),
            HostScriptPlayerCensus {
                money: 731,
                ..Default::default()
            },
        );
        crate::scripting::HostScriptQuery::published_for_test().set_host_script_query_snapshot(snap);
        assert_eq!(crate::scripting::HostScriptQuery::published_for_test().host_query_player_census("AuthoredSide").unwrap().money, 731);
        assert!(crate::scripting::HostScriptQuery::published_for_test().host_query_player_census("authoredside").is_none());
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
    }

    #[test]
    fn host_skirmish_discovered_uses_discovered_by() {
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
        let mut snap = HostScriptQuerySnapshot::default();
        snap.objects.push(HostScriptQueryObject {
            id: 7,
            owner_player: "PlyrChina".into(),
            discovered_by: vec!["PlyrAmerica".into()],
            ..Default::default()
        });
        crate::scripting::HostScriptQuery::published_for_test().set_host_script_query_snapshot(snap);
        assert_eq!(
            crate::scripting::HostScriptQuery::published_for_test().host_eval_skirmish_player_has_discovered_player("PlyrChina", "PlyrAmerica"),
            Some(true)
        );
        assert_eq!(
            crate::scripting::HostScriptQuery::published_for_test().host_eval_skirmish_player_has_discovered_player("PlyrChina", "PlyrGLA"),
            Some(false)
        );
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
    }

    #[test]
    fn host_skirmish_prereq_none_without_census() {
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
        assert_eq!(
            crate::scripting::HostScriptQuery::published_for_test().host_eval_skirmish_player_has_prerequisite_to_build("PlyrAmerica", "AmericaWarFactory"),
            None
        );
    }

    #[test]
    fn host_skirmish_prereq_false_without_leftover_template() {
        // Retail names may already exist in the loaded catalog. This case
        // exercises ObjectTypes::canBuildAny's genuinely missing-template path.
        const MISSING_TEMPLATE: &str = "NeverAuthoredPrerequisiteFixture";
        assert!(crate::helpers::TheThingFactory::find_template(MISSING_TEMPLATE).is_none());
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
        let mut snap = HostScriptQuerySnapshot::default();
        let mut census = HostScriptPlayerCensus::default();
        census
            .template_counts
            .insert("americacommandcenter".into(), 1);
        snap.player_census.insert("PlyrAmerica".into(), census);
        crate::scripting::HostScriptQuery::published_for_test().set_host_script_query_snapshot(snap);
        assert_eq!(
            crate::scripting::HostScriptQuery::published_for_test().host_eval_skirmish_player_has_prerequisite_to_build("PlyrAmerica", MISSING_TEMPLATE),
            Some(false)
        );
        crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
    }
}

#[cfg(test)]
mod command_button_kind_world_tests {
    use super::*;
    use crate::system::engine_stores::{new_world_services, with_world_services};

    #[test]
    fn command_button_kind_reads_only_the_evaluating_engines_control_bar() {
        let a = new_world_services();
        let b = new_world_services();
        with_world_services(&b, || {
            let mut parsed = game_engine::common::ini::ini_command_button::CommandButton::new(
                "Command_OnlyInB".to_string(),
            );
            parsed.command = "SPECIAL_POWER".to_string();
            let button = crate::command_button::CommandButton::from_common(7, &parsed);
            crate::control_bar::install_test_command_button(button, "Set_B", 0).unwrap();
        });
        let bridge_b = crate::control_bar::control_bar_bridge_in(&b).expect("b's ControlBar");
        assert!(crate::control_bar::control_bar_bridge_in(&a).is_none());
        // Evaluated for world `a` while `b` is published: `a` has no ControlBar.
        with_world_services(&b, || {
            assert!(matches!(
                leftover_command_button_kind("Command_OnlyInB", None),
                LeftoverCommandButtonKind::Unknown
            ));
            assert!(!matches!(
                leftover_command_button_kind("Command_OnlyInB", Some(&bridge_b)),
                LeftoverCommandButtonKind::Unknown | LeftoverCommandButtonKind::Missing
            ));
            assert!(matches!(
                leftover_command_button_kind("Command_Absent", Some(&bridge_b)),
                LeftoverCommandButtonKind::Missing
            ));
        });
    }
}

#[cfg(test)]
mod host_query_world_tests {
    use super::*;
    use crate::system::engine_stores::{new_world_services, with_world_services};

    #[test]
    fn host_query_reads_only_the_evaluating_worlds_snapshot() {
        let a = new_world_services();
        let b = new_world_services();
        let mut snap = HostScriptQuerySnapshot::default();
        snap.named.insert("Hero".to_string(), 7);
        HostScriptQuery::of(&a).set_host_script_query_snapshot(snap);
        // Another world published on this thread never answers for `a`.
        with_world_services(&b, || {
            assert_eq!(HostScriptQuery::of(&a).host_script_named_unit_id("Hero"), Some(7));
            assert_eq!(HostScriptQuery::of(&b).host_script_named_unit_id("Hero"), None);
            assert!(!HostScriptQuery::of(&b).host_script_query_has_any());
            assert_eq!(HostScriptQuery::none().host_script_named_unit_id("Hero"), None);
        });
        HostScriptQuery::of(&a).clear_host_script_query_snapshot();
        assert_eq!(HostScriptQuery::of(&a).host_script_named_unit_id("Hero"), None);
    }

    #[test]
    fn trigger_team_edge_reads_members_of_the_querying_world_only() {
        let a = new_world_services();
        let b = new_world_services();
        let mut snap = HostScriptQuerySnapshot::default();
        snap.team_instance_ids
            .insert("TriggerTeamTwoWorld".to_string(), vec![42]);
        HostScriptQuery::of(&a).set_host_script_query_snapshot(snap);
        let mut trig = HostTriggerWorld::default();
        trig.restore(&[HostObjectTriggerPersist {
            object_id: 42,
            entered_or_exited_frame: 9,
            ..Default::default()
        }]);
        with_world_services(&b, || {
            assert!(host_team_did_enter_or_exit(&HostScriptQuery::of(&a), &trig, "TriggerTeamTwoWorld", 9));
            assert!(!host_team_did_enter_or_exit(&HostScriptQuery::of(&b), &trig, "TriggerTeamTwoWorld", 9));
        });
    }
}

impl std::fmt::Debug for HostScriptQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostScriptQuery")
            .field("bound", &self.world.is_some())
            .finish()
    }
}
