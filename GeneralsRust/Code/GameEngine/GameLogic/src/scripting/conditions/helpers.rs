//! Shared helpers for script condition evaluation.

use super::ScriptValue;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::{Player, player_list};
use crate::scripting::engine::{get_named_object_tracker, get_script_engine};
use crate::scripting::events::GameEventType;
use crate::{GameLogicError, GameLogicResult};
use std::cell::RefCell;
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
    /// Live AIPlayer::isSupplySourceAttacked keyed by player name.
    pub supply_source_attacked: HashMap<String, bool>,
    /// Cash at the preferred warehouse (or -1 if none).
    pub supply_center_cash: HashMap<String, i32>,
    /// isLocationSafe of that warehouse.
    pub supply_center_location_safe: HashMap<String, bool>,
    /// Live host Player money/power/object census keyed by lowercase SIDE name.
    pub player_census: HashMap<String, HostScriptPlayerCensus>,
    /// World tech-building census for SKIRMISH_TECH_BUILDING_WITHIN_DISTANCE.
    pub tech_buildings: Vec<HostTechBuildingCensus>,

    /// C++ TerrainLogic::anyBridgesDamageStatesChanged one-frame latch.
    pub any_bridges_damage_states_changed: bool,
    /// Named bridge isBridgeBroken keyed by script unit name.
    pub named_bridge_broken: HashMap<String, bool>,
    /// Named bridge isBridgeRepaired keyed by script unit name.
    pub named_bridge_repaired: HashMap<String, bool>,
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

#[derive(Debug, Clone, Default)]
pub struct HostScriptQueryObject {
    pub id: u32,
    pub name: String,
    pub team: u32,
    pub x: f32,
    /// Host Y-up height. C++ AudioEventRTS Z-up uses this as `z`.
    pub y: f32,
    pub z: f32,

    pub alive: bool,
    /// C++ Object::isEffectivelyDead while the pointer still exists.
    pub effectively_dead: bool,
    pub health: f32,
    pub initial_health: f32,
    pub owner_player: String,
    pub template_name: String,
    pub has_contain: bool,
    pub contain_count: u32,
    pub contain_max: u32,
    pub last_damage_source_id: u32,
    pub last_damage_template: String,
    pub last_damage_player: String,
    pub kind_structure: bool,
    pub kind_projectile: bool,
    pub kind_inert: bool,
    pub kind_mine: bool,
    pub held: bool,
    pub stealthed_hidden: bool,
    pub discovered_by: Vec<String>,
    pub waypoint_labels: Vec<String>,
    /// C++ Drawable selected / Object::isSelected for NAMED_SELECTED.
    pub selected: bool,
    /// C++ AIUpdateInterface::isIdle for sequential UNIT/TEAM progress.
    pub idle: bool,
    /// C++ Object::getVisionRange for ENEMY/TYPE_SIGHTED.
    pub vision_range: f32,
    /// Host KindOf Debug names (Infantry, Vehicle, Structure, …).
    pub kind_names: Vec<String>,
    /// C++ SpecialPowerModule::getPercentReady() == 1 for host isReady.
    pub special_power_ready: bool,
    /// Canonical SpecialPowerTemplate names this object owns.
    pub special_power_templates: Vec<String>,
    /// C++ LocomotorSet::getValidSurfaces; 0 means no AI → GROUND.
    pub locomotor_surfaces: u32,
    /// C++ Object::isCaptured (private CAPTURED status).
    pub captured: bool,
    /// C++ DISABLED_UNMANNED (unowned faction vehicle).
    pub unmanned: bool,
    /// C++ ContainModuleInterface::isGarrisonable.
    pub garrisonable: bool,
    /// C++ ThingTemplate::friend_getBuildCost.
    pub build_cost: i32,
    /// C++ Object::getStatusBits packed mask (OBJECT_STATUS_*).
    pub status_bits: u64,
    /// C++ OpenContain::getPlayerWhoEntered — last enterer's SIDE name.
    pub player_who_entered: String,
    /// C++ findUpdateModule("SupplyWarehouseDockUpdate") present.
    pub is_supply_warehouse: bool,
    /// C++ SupplyWarehouseDockUpdate::getBoxesStored.
    pub warehouse_boxes: i32,
    /// C++ Object::isOffMap — PartitionFilterOnMap reject.
    pub off_map: bool,
    /// C++ Object::getContainedBy id (0 = none) for TEAM_WAIT_FOR_NOT_CONTAINED.
    pub contained_by: u32,
    /// C++ AI_EXIT pretend-contained while evacuating (leftover Exit state).
    pub ai_exiting: bool,
}

/// Live host Player::getMoney / getEnergy / hasAnyObjects census.
/// C++ ScriptConditions player conditions read these from the same Player.
#[derive(Debug, Clone, Default)]
pub struct HostScriptPlayerCensus {
    /// C++ Player::getMoney()->countMoney().
    pub money: i32,
    /// C++ Energy production (0 while sabotaged).
    pub energy_production: i32,
    /// C++ Energy consumption.
    pub energy_consumption: i32,
    /// C++ Energy::m_powerSabotagedTillFrame still active.
    pub power_sabotaged: bool,
    /// C++ Player::hasAnyObjects().
    pub has_any_objects: bool,
    /// C++ Player::hasAnyBuildFacility().
    pub has_any_build_facility: bool,
    /// C++ Player::countBuildings().
    pub building_count: i32,
    /// C++ Player::countObjects(STRUCTURE|MP_COUNT_FOR_VICTORY).
    pub faction_building_count: i32,
    /// C++ Player::getSciencePurchasePoints().
    pub science_purchase_points: i32,
    /// C++ Player science vec names (SCIENCE_RankN / purchased).
    pub unlocked_sciences: Vec<String>,
    /// C++ countObjectsByThingTemplate(..., ignoreDead=false, ignoreUnderConstruction=true).
    /// Keyed by lowercase ThingTemplate name.
    pub template_counts: HashMap<String, i32>,
    /// C++ countObjectsByThingTemplate(..., ignoreDead=true, ignoreUnderConstruction=true).
    pub template_counts_ignore_dead: HashMap<String, i32>,
    /// C++ Player::getSupplyBoxValue (GlobalData m_baseValuePerSupplyBox).
    pub supply_box_value: i32,
}

impl HostScriptPlayerCensus {
    /// C++ Energy::hasSufficientPower.
    pub fn has_sufficient_power(&self) -> bool {
        if self.power_sabotaged {
            false
        } else {
            self.energy_production >= self.energy_consumption
        }
    }

    /// C++ Energy::getEnergySupplyRatio.
    pub fn supply_ratio(&self) -> f32 {
        if self.power_sabotaged {
            return 0.0;
        }
        if self.energy_consumption <= 0 {
            self.energy_production as f32
        } else {
            self.energy_production as f32 / self.energy_consumption as f32
        }
    }

    /// C++ production - consumption (sabotage zeros production).
    pub fn excess_power(&self) -> i32 {
        if self.power_sabotaged {
            -self.energy_consumption
        } else {
            self.energy_production - self.energy_consumption
        }
    }

    /// Sum census counts for the given template / ObjectTypes names.
    pub fn count_templates(
        &self,
        type_names: impl IntoIterator<Item = impl AsRef<str>>,
        ignore_dead: bool,
    ) -> i32 {
        let map = if ignore_dead {
            &self.template_counts_ignore_dead
        } else {
            &self.template_counts
        };
        let mut seen = HashSet::new();
        let mut sum = 0;
        for name in type_names {
            let key = name.as_ref().trim().to_ascii_lowercase();
            if key.is_empty() || !seen.insert(key.clone()) {
                continue;
            }
            sum += map.get(&key).copied().unwrap_or(0);
        }
        sum
    }
}

thread_local! {
    static HOST_SCRIPT_QUERY: RefCell<HostScriptQuerySnapshot> =
        RefCell::new(HostScriptQuerySnapshot::default());
}

/// Merge additional host-query rows into the current snapshot.
pub fn merge_host_script_query_snapshot(f: impl FnOnce(&mut HostScriptQuerySnapshot)) {
    HOST_SCRIPT_QUERY.with(|slot| f(&mut *slot.borrow_mut()));
}

/// Inject a read-only host name/team/area query map for crate conditions.
pub fn set_host_script_query_snapshot(snap: HostScriptQuerySnapshot) {
    HOST_SCRIPT_QUERY.with(|slot| *slot.borrow_mut() = snap);
}

pub fn clear_host_script_query_snapshot() {
    HOST_SCRIPT_QUERY.with(|slot| *slot.borrow_mut() = HostScriptQuerySnapshot::default());
    clear_host_trigger_flags();
}

pub fn host_bridge_broken(bridge_name: &str) -> bool {
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
        snap.any_bridges_damage_states_changed
            && snap
                .named_bridge_broken
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(bridge_name))
                .map(|(_, v)| *v)
                .unwrap_or(false)
    })
}

pub fn host_bridge_repaired(bridge_name: &str) -> bool {
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
        snap.any_bridges_damage_states_changed
            && snap
                .named_bridge_repaired
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(bridge_name))
                .map(|(_, v)| *v)
                .unwrap_or(false)
    })
}

pub fn host_script_named_unit_id(name: &str) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| slot.borrow().named.get(name).copied())
}

/// True when a host snapshot was injected (any named/team/object/area/tech row).
pub fn host_script_query_has_any() -> bool {
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
        !snap.named.is_empty()
            || !snap.objects.is_empty()
            || !snap.team_ids.is_empty()
            || !snap.tech_buildings.is_empty()
    })
}

/// Host named-unit aliveness from the snapshot (no crate Object).
pub fn host_script_named_unit_alive(name: &str) -> Option<bool> {
    if name.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
        snap.objects
            .iter()
            .find(|o| o.name == name)
            .map(|o| o.alive)
            .or_else(|| snap.named.contains_key(name).then_some(true))
    })
}

/// True when the host snapshot lists this named unit (alive or dying).
pub fn host_script_named_unit_present(name: &str) -> bool {
    host_script_named_unit_alive(name).is_some() || host_script_named_unit_id(name).is_some()
}

/// C++ evaluateNamedSelected: Object::getName() match on selected drawables.
pub fn host_script_named_unit_selected(name: &str) -> Option<bool> {
    if name.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .objects
            .iter()
            .find(|o| o.name == name)
            .map(|o| o.selected)
    })
}

/// C++ AIGroup::isIdle / isGroupAiDead over host snapshot members.
/// Empty group is idle and dead (vacuous C++ loops).
pub fn host_team_sequential_status(team_name: &str) -> (bool, bool) {
    let ids = host_script_team_member_ids(team_name);
    if ids.is_empty() {
        return (true, true);
    }
    let mut idle = true;
    let mut all_dead = true;
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
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

pub fn host_script_query_object(name: &str) -> Option<HostScriptQueryObject> {
    if name.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
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

pub fn host_script_query_object_by_id(id: u32) -> Option<HostScriptQueryObject> {
    HOST_SCRIPT_QUERY.with(|slot| slot.borrow().objects.iter().find(|o| o.id == id).cloned())
}

/// C++ evaluateBuildingEntered on the host snapshot (no leftover contain).
/// None = named building missing from snapshot.
pub fn host_building_entered_by_player(building_name: &str, player_name: &str) -> Option<bool> {
    let obj = host_script_query_object(building_name)?;
    if !obj.has_contain {
        return Some(false);
    }
    if obj.player_who_entered.is_empty() {
        return Some(false);
    }
    Some(obj.player_who_entered.eq_ignore_ascii_case(player_name))
}

/// C++ Team::hasAnyObjects over host snapshot members.
pub fn host_team_has_any_live_objects(team_name: &str) -> bool {
    let ids: HashSet<u32> = host_script_team_member_ids(team_name).into_iter().collect();
    if ids.is_empty() {
        return false;
    }
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow().objects.iter().any(|o| {
            ids.contains(&o.id) && o.alive && !o.kind_projectile && !o.kind_inert && !o.kind_mine
        })
    })
}

/// C++ Team::hasAnyUnits over host snapshot members.
pub fn host_team_has_any_live_units(team_name: &str) -> bool {
    let ids: HashSet<u32> = host_script_team_member_ids(team_name).into_iter().collect();
    if ids.is_empty() {
        return false;
    }
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow().objects.iter().any(|o| {
            ids.contains(&o.id)
                && o.alive
                && !o.kind_structure
                && !o.kind_projectile
                && !o.kind_mine
        })
    })
}

/// True when the host snapshot lists any member ids for this team instance.
pub fn host_team_was_fielded(team_name: &str) -> bool {
    !host_script_team_member_ids(team_name).is_empty()
}

pub fn host_script_team_unit_ids(team: u32) -> Vec<u32> {
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .team_ids
            .get(&team)
            .cloned()
            .unwrap_or_default()
    })
}

fn host_player_query_key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

/// Live `AIPlayer::isSupplySourceAttacked` for leftover conditions.
pub fn host_query_supply_source_attacked(player_name: &str) -> Option<bool> {
    let key = host_player_query_key(player_name);
    if key.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| slot.borrow().supply_source_attacked.get(&key).copied())
}

/// Live `AIPlayer::isSupplySourceSafe(min)` for leftover conditions.
pub fn host_query_supply_source_safe(player_name: &str, min_supplies: i32) -> Option<bool> {
    let key = host_player_query_key(player_name);
    if key.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
        let cash = *snap.supply_center_cash.get(&key)?;
        if cash < min_supplies {
            return Some(true);
        }
        Some(
            snap.supply_center_location_safe
                .get(&key)
                .copied()
                .unwrap_or(true),
        )
    })
}

/// Live host Player census for leftover/live script player conditions.
pub fn host_query_player_census(player_name: &str) -> Option<HostScriptPlayerCensus> {
    let key = host_player_query_key(player_name);
    if key.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| slot.borrow().player_census.get(&key).cloned())
}

/// C++ Player::countObjectsByThingTemplate from the live host census.
/// `ignore_dead` matches C++ ignoreDead; under-construction is already excluded.
pub fn host_query_player_template_count(
    player_name: &str,
    type_names: &[String],
    ignore_dead: bool,
) -> Option<i32> {
    let census = host_query_player_census(player_name)?;
    Some(census.count_templates(type_names, ignore_dead))
}

/// C++ Player::getSciencePurchasePoints from the live host census.
pub fn host_query_player_science_purchase_points(player_name: &str) -> Option<i32> {
    host_query_player_census(player_name).map(|c| c.science_purchase_points)
}

/// C++ Player::hasScience against host unlocked names.
pub fn host_query_player_has_science(player_name: &str, science_name: &str) -> Option<bool> {
    let census = host_query_player_census(player_name)?;
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

fn host_object_in_named_area(obj: &HostScriptQueryObject, area_name: &str) -> bool {
    if let Some(trigger) = host_script_lookup_polygon_trigger(area_name) {
        return trigger.point_in_trigger_int(&host_xz_to_trigger_point(obj.x, obj.z));
    }
    if let Some((min_x, min_z, max_x, max_z)) = host_script_area_bounds(area_name) {
        return obj.x >= min_x && obj.x <= max_x && obj.z >= min_z && obj.z <= max_z;
    }
    false
}

fn host_unit_counts_in_area(obj: &HostScriptQueryObject, include_crate: bool) -> bool {
    let dead_or_inert = obj.effectively_dead || !obj.alive || obj.kind_inert;
    if include_crate && host_object_has_kind(obj, crate::common::KindOf::Crate) {
        return true;
    }
    !dead_or_inert
}

/// C++ evaluatePlayerHasUnitTypeInArea count over host objects.
pub fn host_count_player_type_in_area(
    player_name: &str,
    area_name: &str,
    type_names: &[String],
) -> Option<i32> {
    if !host_script_query_has_any() {
        return None;
    }
    if area_name.is_empty()
        && host_script_lookup_polygon_trigger(area_name).is_none()
        && host_script_area_bounds(area_name).is_none()
    {
        return None;
    }
    if host_script_lookup_polygon_trigger(area_name).is_none()
        && host_script_area_bounds(area_name).is_none()
    {
        return None;
    }
    Some(HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .objects
            .iter()
            .filter(|o| {
                host_owner_matches(o, player_name)
                    && type_names
                        .iter()
                        .any(|t| o.template_name.eq_ignore_ascii_case(t))
                    && host_object_in_named_area(o, area_name)
                    && host_unit_counts_in_area(o, true)
            })
            .count() as i32
    }))
}

/// C++ evaluatePlayerHasUnitKindInArea count over host objects.
pub fn host_count_player_kind_in_area(
    player_name: &str,
    area_name: &str,
    kind: crate::common::KindOf,
) -> Option<i32> {
    if !host_script_query_has_any() {
        return None;
    }
    if host_script_lookup_polygon_trigger(area_name).is_none()
        && host_script_area_bounds(area_name).is_none()
    {
        return None;
    }
    Some(HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .objects
            .iter()
            .filter(|o| {
                host_owner_matches(o, player_name)
                    && host_object_has_kind(o, kind)
                    && host_object_in_named_area(o, area_name)
                    && host_unit_counts_in_area(o, false)
            })
            .count() as i32
    }))
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

/// C++ evaluateEnemySighted over host snapshot objects.
pub fn host_enemy_sighted(unit_name: &str, alliance: i32, player_name: &str) -> Option<bool> {
    if !host_script_query_has_any() {
        return None;
    }
    let looker = host_script_query_object(unit_name)?;
    if !looker.alive || looker.effectively_dead {
        return Some(false);
    }
    Some(HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow().objects.iter().any(|cand| {
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
pub fn host_type_sighted(
    unit_name: &str,
    type_names: &[String],
    player_name: &str,
) -> Option<bool> {
    if !host_script_query_has_any() {
        return None;
    }
    let looker = host_script_query_object(unit_name)?;
    if !looker.alive || looker.effectively_dead {
        return Some(false);
    }
    Some(HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow().objects.iter().any(|cand| {
            host_sighted_candidate_ok(&looker, cand)
                && host_owner_matches(cand, player_name)
                && type_names
                    .iter()
                    .any(|t| cand.template_name.eq_ignore_ascii_case(t))
        })
    }))
}

pub fn host_script_area_unit_ids(min_x: f32, min_z: f32, max_x: f32, max_z: f32) -> Vec<u32> {
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .objects
            .iter()
            .filter(|o| o.alive && o.x >= min_x && o.x <= max_x && o.z >= min_z && o.z <= max_z)
            .map(|o| o.id)
            .collect()
    })
}

pub fn host_script_named_unit_in_area(
    name: &str,
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
) -> bool {
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
        snap.objects.iter().any(|o| {
            o.alive
                && o.name == name
                && o.x >= min_x
                && o.x <= max_x
                && o.z >= min_z
                && o.z <= max_z
        })
    })
}

pub fn host_script_area_bounds(area_name: &str) -> Option<(f32, f32, f32, f32)> {
    if area_name.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| slot.borrow().areas.get(area_name).copied())
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
    let terrain = crate::terrain::get_terrain_logic().read().ok()?;
    terrain
        .get_trigger_area_by_name(&resolved)
        .or_else(|| terrain.get_trigger_area_by_name(area_name))
        .cloned()
}

fn host_named_unit_point_in_trigger(
    unit_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
) -> bool {
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow().objects.iter().any(|o| {
            o.name == unit_name && trigger.point_in_trigger_int(&host_xz_to_trigger_point(o.x, o.z))
        })
    })
}

/// `Some(true/false)` when leftover polygon or host AABB geometry exists.
/// C++ `evaluateNamedInsideArea` uses `pointInTrigger` on current position.
pub fn host_script_named_unit_in_named_area(unit_name: &str, area_name: &str) -> Option<bool> {
    if let Some(trigger) = host_script_lookup_polygon_trigger(area_name) {
        return Some(host_named_unit_point_in_trigger(unit_name, &trigger));
    }
    let (min_x, min_z, max_x, max_z) = host_script_area_bounds(area_name)?;
    Some(host_script_named_unit_in_area(
        unit_name, min_x, min_z, max_x, max_z,
    ))
}

const HOST_MAX_TRIGGER_INFOS: usize = 5;

#[derive(Clone)]
struct HostTriggerSlot {
    trigger_id: i32,
    is_inside: bool,
    entered: bool,
    exited: bool,
}

struct HostObjectTriggerState {
    i_x: i32,
    i_y: i32,
    entered_or_exited_frame: u32,
    slots: Vec<HostTriggerSlot>,
}

#[derive(Default)]
struct HostTriggerWorld {
    objects: HashMap<u32, HostObjectTriggerState>,
    team_entered_or_exited: HashMap<String, u32>,
}

thread_local! {
    static HOST_TRIGGER_WORLD: RefCell<HostTriggerWorld> =
        RefCell::new(HostTriggerWorld::default());
}

fn leftover_polygon_triggers() -> Vec<crate::polygon_trigger::PolygonTrigger> {
    crate::terrain::get_terrain_logic()
        .read()
        .ok()
        .map(|terrain| terrain.get_trigger_areas().get_triggers().to_vec())
        .unwrap_or_default()
}

fn host_flag_window(flag_frame: u32, now: u32) -> bool {
    flag_frame == now || (now > 0 && flag_frame == now - 1)
}

fn current_logic_frame() -> u32 {
    crate::system::game_logic::current_frame()
}

/// C++ `Object::setTriggerAreaFlagsForChangeInPosition` for host units.
pub fn update_host_object_trigger_flags(
    object_id: u32,
    x: f32,
    z: f32,
    frame: u32,
    skip: bool,
    team_name: Option<&str>,
) {
    if skip {
        return;
    }
    let new_x = x as i32;
    let new_y = z as i32;
    let triggers = leftover_polygon_triggers();
    HOST_TRIGGER_WORLD.with(|world| {
        let mut world = world.borrow_mut();
        let state = world
            .objects
            .entry(object_id)
            .or_insert_with(|| HostObjectTriggerState {
                i_x: 0,
                i_y: 0,
                entered_or_exited_frame: 0,
                slots: Vec::new(),
            });
        let pos_changed = state.i_x != new_x || state.i_y != new_y;
        // C++ Object.cpp:2575-2578: unchanged integer XY returns even with
        // zero active areas. Required so load can restore m_iPos and skip
        // a fresh ENTERED_AREA edge.
        if !pos_changed {
            return;
        }
        if state.entered_or_exited_frame != 0 && state.entered_or_exited_frame != frame {
            state.slots.retain(|slot| slot.is_inside);
            for slot in &mut state.slots {
                slot.entered = false;
                slot.exited = false;
            }
        }
        if pos_changed {
            let old = crate::common::ICoord3D::new(state.i_x, state.i_y, 0);
            for slot in &mut state.slots {
                let Some(trigger) = triggers.iter().find(|t| t.get_id() == slot.trigger_id) else {
                    continue;
                };
                if !trigger.point_in_trigger_int(&old) {
                    slot.is_inside = false;
                    slot.exited = true;
                    state.entered_or_exited_frame = frame;
                }
            }
            state.i_x = new_x;
            state.i_y = new_y;
        }
        let now_pt = crate::common::ICoord3D::new(state.i_x, state.i_y, 0);
        for trigger in &triggers {
            if state
                .slots
                .iter()
                .any(|slot| slot.trigger_id == trigger.get_id())
            {
                continue;
            }
            if !trigger.point_in_trigger_int(&now_pt) {
                continue;
            }
            if state.slots.len() >= HOST_MAX_TRIGGER_INFOS {
                break;
            }
            state.slots.push(HostTriggerSlot {
                trigger_id: trigger.get_id(),
                is_inside: true,
                entered: true,
                exited: false,
            });
            state.entered_or_exited_frame = frame;
        }
        if state.entered_or_exited_frame == frame {
            if let Some(name) = team_name.filter(|name| !name.is_empty()) {
                world.team_entered_or_exited.insert(name.to_string(), frame);
            }
        }
    });
}

pub fn clear_host_trigger_flags() {
    HOST_TRIGGER_WORLD.with(|world| *world.borrow_mut() = HostTriggerWorld::default());
}

/// C++ `Object::xfer` (`Object.cpp:4218-4246`) per-area slot.
#[derive(Clone, Debug, Default)]
pub struct HostTriggerSlotPersist {
    pub trigger_id: i32,
    pub trigger_name: String,
    pub is_inside: bool,
    pub entered: bool,
    pub exited: bool,
}

/// C++ `Object::xfer` trigger housekeeping: `m_iPos`, `m_enteredOrExitedFrame`,
/// `m_numTriggerAreasActive` + per-area entered/exited/isInside.
#[derive(Clone, Debug, Default)]
pub struct HostObjectTriggerPersist {
    pub object_id: u32,
    pub i_x: i32,
    pub i_y: i32,
    pub entered_or_exited_frame: u32,
    pub slots: Vec<HostTriggerSlotPersist>,
}

/// Capture live `HOST_TRIGGER_WORLD` slots for WorldSnapshot persist.
pub fn capture_host_object_trigger_persists() -> Vec<HostObjectTriggerPersist> {
    let triggers = leftover_polygon_triggers();
    HOST_TRIGGER_WORLD.with(|world| {
        let world = world.borrow();
        let mut entries: Vec<HostObjectTriggerPersist> = world
            .objects
            .iter()
            .map(|(object_id, state)| HostObjectTriggerPersist {
                object_id: *object_id,
                i_x: state.i_x,
                i_y: state.i_y,
                entered_or_exited_frame: state.entered_or_exited_frame,
                slots: state
                    .slots
                    .iter()
                    .map(|slot| {
                        let trigger_name = triggers
                            .iter()
                            .find(|trigger| trigger.get_id() == slot.trigger_id)
                            .map(|trigger| trigger.get_trigger_name().to_string())
                            .unwrap_or_default();
                        HostTriggerSlotPersist {
                            trigger_id: slot.trigger_id,
                            trigger_name,
                            is_inside: slot.is_inside,
                            entered: slot.entered,
                            exited: slot.exited,
                        }
                    })
                    .collect(),
            })
            .collect();
        entries.sort_by_key(|entry| entry.object_id);
        entries
    })
}

/// Restore slots and integer pose before the first post-load position update.
pub fn restore_host_object_trigger_persists(entries: &[HostObjectTriggerPersist]) {
    let triggers = leftover_polygon_triggers();
    HOST_TRIGGER_WORLD.with(|world| {
        let mut world = world.borrow_mut();
        *world = HostTriggerWorld::default();
        for entry in entries {
            let slots = entry
                .slots
                .iter()
                .map(|slot| {
                    let trigger_id = if slot.trigger_name.is_empty() {
                        slot.trigger_id
                    } else {
                        triggers
                            .iter()
                            .find(|trigger| {
                                trigger.get_trigger_name().to_string() == slot.trigger_name
                            })
                            .map(|trigger| trigger.get_id())
                            .unwrap_or(slot.trigger_id)
                    };
                    HostTriggerSlot {
                        trigger_id,
                        is_inside: slot.is_inside,
                        entered: slot.entered,
                        exited: slot.exited,
                    }
                })
                .collect();
            world.objects.insert(
                entry.object_id,
                HostObjectTriggerState {
                    i_x: entry.i_x,
                    i_y: entry.i_y,
                    entered_or_exited_frame: entry.entered_or_exited_frame,
                    slots,
                },
            );
        }
    });
}

pub fn sync_host_trigger_flags_from_snapshot(frame: u32) {
    let snap = HOST_SCRIPT_QUERY.with(|slot| slot.borrow().clone());
    for obj in &snap.objects {
        let team = snap
            .team_instance_ids
            .iter()
            .find_map(|(name, ids)| ids.contains(&obj.id).then_some(name.as_str()));
        update_host_object_trigger_flags(obj.id, obj.x, obj.z, frame, false, team);
    }
}

pub fn host_object_did_enter_or_exit(object_id: u32) -> bool {
    let now = current_logic_frame();
    HOST_TRIGGER_WORLD.with(|world| {
        world
            .borrow()
            .objects
            .get(&object_id)
            .is_some_and(|state| host_flag_window(state.entered_or_exited_frame, now))
    })
}

pub fn host_object_did_enter(
    object_id: u32,
    trigger: &crate::polygon_trigger::PolygonTrigger,
) -> bool {
    let now = current_logic_frame();
    HOST_TRIGGER_WORLD.with(|world| {
        let world = world.borrow();
        let Some(state) = world.objects.get(&object_id) else {
            return false;
        };
        host_flag_window(state.entered_or_exited_frame, now)
            && state
                .slots
                .iter()
                .any(|slot| slot.entered && slot.trigger_id == trigger.get_id())
    })
}

pub fn host_object_did_exit(
    object_id: u32,
    trigger: &crate::polygon_trigger::PolygonTrigger,
) -> bool {
    let now = current_logic_frame();
    HOST_TRIGGER_WORLD.with(|world| {
        let world = world.borrow();
        let Some(state) = world.objects.get(&object_id) else {
            return false;
        };
        host_flag_window(state.entered_or_exited_frame, now)
            && state
                .slots
                .iter()
                .any(|slot| slot.exited && slot.trigger_id == trigger.get_id())
    })
}

/// C++ Team.cpp:142-145 `locoSetMatches`.
/// Script bit0 stays GROUND; bit1 remaps to locomotor AIR (`<< 2`).
pub fn host_script_loco_set_matches(lstm: u32, which_to_consider: u32) -> bool {
    let remapped = (which_to_consider & 0x01) | ((which_to_consider & 0x02) << 2);
    (remapped & lstm) != 0
}

/// C++ no-AI members consider themselves GROUND.
pub fn host_script_loco_matches_ground(which_to_consider: u32) -> bool {
    host_script_loco_set_matches(crate::path::SURFACE_GROUND, which_to_consider)
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
    if !host_script_loco_set_matches(surfaces, which_to_consider) {
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

pub fn host_script_team_member_ids(team_name: &str) -> Vec<u32> {
    if team_name.is_empty() {
        return Vec::new();
    }
    let mut ids = HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
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
        ids = host_script_team_unit_ids(ord);
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// C++ `ScriptConditions::evaluateTeamIsContained` over host snapshot members.
/// `getContainedBy() != NULL`, plus leftover Exit-state pretend-contained.
pub fn host_eval_team_is_contained(team_name: &str, all_contained: bool) -> bool {
    let members = host_script_team_member_ids(team_name);
    if members.is_empty() {
        return false;
    }
    let mut any_considered = false;
    for id in members {
        let Some(obj) = host_script_query_object_by_id(id) else {
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
pub fn host_eval_skirmish_command_button_ready(
    team_name: &str,
    command_button_name: &str,
    all_ready: bool,
) -> Option<bool> {
    if !host_script_query_has_any() {
        return None;
    }
    if team_name.is_empty() {
        return Some(false);
    }

    match leftover_command_button_kind(command_button_name) {
        LeftoverCommandButtonKind::Missing => return Some(false),
        LeftoverCommandButtonKind::Unknown
        | LeftoverCommandButtonKind::SpecialPower(_)
        | LeftoverCommandButtonKind::Upgrade
        | LeftoverCommandButtonKind::Other => {}
    }

    let ids = host_script_team_member_ids(team_name);
    if ids.is_empty() && !host_team_name_known(team_name) {
        return Some(false);
    }

    for id in ids {
        let Some(obj) = host_script_query_object_by_id(id) else {
            continue;
        };
        let Some(is_ready) = host_command_button_ready_for_object(&obj, command_button_name) else {
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

/// C++ ScriptConditions::evaluateSkirmishSpecialPowerIsReady over the host snapshot.
pub fn host_eval_skirmish_special_power_ready(player_name: &str, power_name: &str) -> Option<bool> {
    if !host_script_query_has_any() {
        return None;
    }
    if power_name.is_empty() {
        return Some(false);
    }
    Some(HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow().objects.iter().any(|obj| {
            host_owner_matches(obj, player_name)
                && host_object_has_special_power(obj, power_name)
                && obj.special_power_ready
        })
    }))
}

/// C++ ScriptConditions::evaluateSkirmishValueInArea over the host snapshot.
pub fn host_eval_skirmish_value_in_area(
    player_name: &str,
    comparison: i32,
    money: i32,
    area_name: &str,
) -> Option<bool> {
    if !host_script_query_has_any() {
        return None;
    }
    if host_script_lookup_polygon_trigger(area_name).is_none()
        && host_script_area_bounds(area_name).is_none()
    {
        return Some(false);
    }
    let total_cost = HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .objects
            .iter()
            .filter(|obj| {
                host_owner_matches(obj, player_name)
                    && !obj.kind_inert
                    && obj.alive
                    && !obj.effectively_dead
                    && host_object_in_named_area(obj, area_name)
            })
            .map(|obj| obj.build_cost)
            .sum::<i32>()
    });
    Some(host_compare_i32(comparison, total_cost, money))
}

/// C++ evaluateSkirmishUnownedFactionUnitComparison count over host snapshot.
pub fn host_eval_skirmish_unowned_faction_unit_count() -> Option<i32> {
    if !host_script_query_has_any() {
        return None;
    }
    Some(HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .objects
            .iter()
            .filter(|obj| obj.unmanned)
            .count() as i32
    }))
}

/// C++ evaluateSkirmishPlayerHasDiscoveredPlayer over host discovered_by (shroud CLEAR|PARTIAL_CLEAR).
pub fn host_eval_skirmish_player_has_discovered_player(
    player_name: &str,
    discovered_by_name: &str,
) -> Option<bool> {
    if !host_script_query_has_any() {
        return None;
    }
    Some(HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow().objects.iter().any(|obj| {
            host_owner_matches(obj, player_name)
                && obj
                    .discovered_by
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(discovered_by_name))
        })
    }))
}

/// C++ evaluateSkirmishPlayerHasPrereqsToBuild / ObjectTypes::canBuildAny using host census.
pub fn host_eval_skirmish_player_has_prerequisite_to_build(
    player_name: &str,
    type_name: &str,
) -> Option<bool> {
    let census = host_query_player_census(player_name)?;
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
    if let Ok(list) = player_list().read() {
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

/// C++ evaluateSkirmishPlayerHasComparisonGarrisoned count over host snapshot.
pub fn host_eval_skirmish_garrisoned_count(player_name: &str) -> Option<i32> {
    if !host_script_query_has_any() {
        return None;
    }
    Some(HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .objects
            .iter()
            .filter(|obj| {
                host_owner_matches(obj, player_name) && obj.garrisonable && obj.contain_count > 0
            })
            .count() as i32
    }))
}

/// C++ evaluateSkirmishPlayerHasComparisonCapturedUnits count over host snapshot.
pub fn host_eval_skirmish_captured_count(player_name: &str) -> Option<i32> {
    if !host_script_query_has_any() {
        return None;
    }
    Some(HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .objects
            .iter()
            .filter(|obj| host_owner_matches(obj, player_name) && obj.captured)
            .count() as i32
    }))
}

/// C++ ScriptConditions::evaluateSkirmishPlayerHasUnitsInArea over the host snapshot.
pub fn host_eval_skirmish_player_has_units_in_area(
    player_name: &str,
    area_name: &str,
) -> Option<bool> {
    if !host_script_query_has_any() {
        return None;
    }
    if host_script_lookup_polygon_trigger(area_name).is_none()
        && host_script_area_bounds(area_name).is_none()
    {
        return Some(false);
    }
    Some(HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow().objects.iter().any(|obj| {
            host_owner_matches(obj, player_name)
                && host_object_in_named_area(obj, area_name)
                && obj.alive
                && !obj.effectively_dead
                && !obj.kind_inert
                && !obj.kind_projectile
        })
    }))
}

/// C++ ScriptConditions::evaluateSkirmishSuppliesWithinDistancePerimeter over
/// the host warehouse census. `None` when no snapshot is injected.
pub fn host_eval_skirmish_supplies_value_within_distance(
    player_name: &str,
    extra_distance: f32,
    area_name: &str,
    compare_value: f32,
) -> Option<bool> {
    if !host_script_query_has_any() {
        return None;
    }
    if player_name.trim().is_empty() {
        return Some(false);
    }
    let Some((center_x, center_y, trigger_radius)) = host_trigger_center_radius(area_name) else {
        return Some(false);
    };
    let radius = trigger_radius + extra_distance;
    let radius_sq = radius * radius;
    let supply_box_value = host_query_player_census(player_name)
        .map(|c| c.supply_box_value)
        .filter(|&v| v > 0)
        .unwrap_or(75) as f32;
    let mut max_value = 0.0f32;
    HOST_SCRIPT_QUERY.with(|slot| {
        for obj in slot.borrow().objects.iter() {
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
fn host_trigger_center_radius(area_name: &str) -> Option<(f32, f32, f32)> {
    if let Some(trigger) = host_script_lookup_polygon_trigger(area_name) {
        let center = trigger.get_center_point();
        return Some((center.x, center.y, trigger.get_radius()));
    }
    let (min_x, min_z, max_x, max_z) = host_script_area_bounds(area_name)?;
    let cx = (min_x + max_x) * 0.5;
    let cz = (min_z + max_z) * 0.5;
    let hx = (max_x - min_x) * 0.5;
    let hz = (max_z - min_z) * 0.5;
    Some((cx, cz, (hx * hx + hz * hz).sqrt()))
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
    if let Ok(players) = player_list().read() {
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
    if let Ok(players) = player_list().read() {
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

/// C++ evaluateSkirmishPlayerTechBuildingWithinDistancePerimeter over the host
/// tech census. `None` when no snapshot or trigger geometry (do not latch).
pub fn host_eval_skirmish_tech_building_within_distance(
    player_name: &str,
    extra_distance: f32,
    area_name: &str,
) -> Option<bool> {
    if !host_script_query_has_any() {
        return None;
    }
    let (center_x, center_y, trigger_radius) =
        host_tech_building_search_circle(area_name, player_name)?;
    let radius = trigger_radius + extra_distance;
    let radius_sq = radius * radius;
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
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
fn host_tech_building_search_circle(area_name: &str, player_name: &str) -> Option<(f32, f32, f32)> {
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
        let center = trigger.get_center_point();
        return Some((center.x, center.y, trigger.get_radius()));
    }
    if let Some(trigger) = crate::scripting::engine::with_script_engine_ref(|engine| {
        engine.get_qualified_trigger_area_by_name(area_name)
    })
    .flatten()
    {
        let center = trigger.get_center_point();
        return Some((center.x, center.y, trigger.get_radius()));
    }
    let (min_x, min_z, max_x, max_z) =
        host_script_area_bounds(&resolved).or_else(|| host_script_area_bounds(area_name))?;
    let cx = (min_x + max_x) * 0.5;
    let cz = (min_z + max_z) * 0.5;
    let hx = (max_x - min_x) * 0.5;
    let hz = (max_z - min_z) * 0.5;
    Some((cx, cz, (hx * hx + hz * hz).sqrt()))
}

fn host_object_has_status_bits(obj: &HostScriptQueryObject, mask: u64) -> bool {
    mask != 0 && (obj.status_bits & mask) != 0
}

/// C++ ScriptConditions::evaluateUnitHasObjectStatus over the host snapshot.
/// `None` when no snapshot is injected (leftover OBJECT_REGISTRY path).
pub fn host_eval_unit_has_object_status(unit_name: &str, status_mask: u64) -> Option<bool> {
    if !host_script_query_has_any() {
        return None;
    }
    let Some(obj) = host_script_query_object(unit_name) else {
        return Some(false);
    };
    Some(host_object_has_status_bits(&obj, status_mask))
}

/// C++ ScriptConditions::evaluateTeamHasObjectStatus over the host snapshot.
/// `None` when no snapshot is injected (leftover OBJECT_REGISTRY path).
pub fn host_eval_team_has_object_status(
    team_name: &str,
    status_mask: u64,
    entire_team: bool,
) -> Option<bool> {
    if !host_script_query_has_any() {
        return None;
    }
    if team_name.is_empty() {
        return Some(false);
    }
    let ids = host_script_team_member_ids(team_name);
    if ids.is_empty() {
        if host_team_name_known(team_name) || leftover_named_team_exists(team_name) {
            return Some(entire_team);
        }
        return Some(false);
    }
    for id in ids {
        let Some(obj) = host_script_query_object_by_id(id) else {
            return Some(false);
        };
        let has = host_object_has_status_bits(&obj, status_mask);
        if entire_team && !has {
            return Some(false);
        }
        if !entire_team && has {
            return Some(true);
        }
    }
    Some(entire_team)
}

fn leftover_named_team_exists(team_name: &str) -> bool {
    crate::team::get_team_factory()
        .lock()
        .ok()
        .and_then(|mut factory| factory.find_team(team_name))
        .is_some()
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

fn leftover_command_button_kind(command_button_name: &str) -> LeftoverCommandButtonKind {
    if let Some(bridge) = crate::control_bar::get_control_bar_bridge() {
        if let Some(button) = bridge.find_command_button_by_name(command_button_name) {
            if let Some(template) = button.get_special_power_template() {
                return LeftoverCommandButtonKind::SpecialPower(template.get_name().to_string());
            }
            if button.get_upgrade_template().is_some() {
                return LeftoverCommandButtonKind::Upgrade;
            }
            return LeftoverCommandButtonKind::Other;
        }
    }
    if let Some(bar) = game_engine::common::ini::ini_command_button::get_control_bar() {
        if let Some(button) = bar.find_command_button_resolved(command_button_name) {
            if let Some(template) = button.get_special_power_template() {
                return LeftoverCommandButtonKind::SpecialPower(template.clone());
            }
            if !button.upgrade.is_empty() {
                return LeftoverCommandButtonKind::Upgrade;
            }
            return LeftoverCommandButtonKind::Other;
        }
        if !bar.get_button_names().is_empty() {
            return LeftoverCommandButtonKind::Missing;
        }
    }
    LeftoverCommandButtonKind::Unknown
}

fn host_team_name_known(team_name: &str) -> bool {
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .team_instance_ids
            .keys()
            .any(|name| name.eq_ignore_ascii_case(team_name))
    })
}

fn host_command_button_ready_for_object(
    obj: &HostScriptQueryObject,
    command_button_name: &str,
) -> Option<bool> {
    match leftover_command_button_kind(command_button_name) {
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

pub fn host_team_did_enter_or_exit(team_name: &str) -> bool {
    let now = current_logic_frame();
    let flagged = HOST_TRIGGER_WORLD.with(|world| {
        world
            .borrow()
            .team_entered_or_exited
            .get(team_name)
            .copied()
            .is_some_and(|frame| host_flag_window(frame, now))
    });
    flagged
        || host_script_team_member_ids(team_name)
            .into_iter()
            .any(host_object_did_enter_or_exit)
}

fn host_team_area_members(team_name: &str, which_to_consider: u32) -> Vec<(u32, f32, f32)> {
    let ids: HashSet<u32> = host_script_team_member_ids(team_name).into_iter().collect();
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .objects
            .iter()
            .filter(|obj| {
                ids.contains(&obj.id) && host_object_counts_for_team_area(obj, which_to_consider)
            })
            .map(|obj| (obj.id, obj.x, obj.z))
            .collect()
    })
}

pub fn host_team_all_inside(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    let members = host_team_area_members(team_name, which_to_consider);
    !members.is_empty()
        && members
            .iter()
            .all(|(_, x, z)| trigger.point_in_trigger_int(&host_xz_to_trigger_point(*x, *z)))
}

pub fn host_team_some_inside_some_outside(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    let members = host_team_area_members(team_name, which_to_consider);
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

pub fn host_team_did_all_enter(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    if !host_team_did_enter_or_exit(team_name) {
        return false;
    }
    let members = host_team_area_members(team_name, which_to_consider);
    let mut entered = false;
    let mut outside = false;
    for (id, x, z) in members {
        if host_object_did_enter(id, trigger) {
            entered = true;
        } else if !trigger.point_in_trigger_int(&host_xz_to_trigger_point(x, z)) {
            outside = true;
        }
    }
    entered && !outside
}

pub fn host_team_did_partial_enter(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    if !host_team_did_enter_or_exit(team_name) {
        return false;
    }
    host_team_area_members(team_name, which_to_consider)
        .into_iter()
        .any(|(id, _, _)| host_object_did_enter(id, trigger))
}

pub fn host_team_did_all_exit(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    if !host_team_did_enter_or_exit(team_name) {
        return false;
    }
    let members = host_team_area_members(team_name, which_to_consider);
    let mut exited = false;
    let mut inside = false;
    let mut any = false;
    for (id, x, z) in members {
        any = true;
        if host_object_did_exit(id, trigger) {
            exited = true;
        } else if trigger.point_in_trigger_int(&host_xz_to_trigger_point(x, z)) {
            inside = true;
        }
    }
    any && exited && !inside
}

pub fn host_team_did_partial_exit(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    if !host_team_did_enter_or_exit(team_name) {
        return false;
    }
    host_team_area_members(team_name, which_to_consider)
        .into_iter()
        .any(|(id, _, _)| host_object_did_exit(id, trigger))
}

/// Wave 271: host-only path has no dual-world factory objects.
#[inline]
pub(super) fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

pub(super) fn normalize_event_name(name: &str) -> String {
    name.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase()
}

pub(super) fn event_type_from_name(name: &str) -> GameEventType {
    let normalized = normalize_event_name(name);
    match normalized.as_str() {
        "unitcreated" | "unit_created" => GameEventType::UnitCreated,
        "unitdestroyed" | "unit_destroyed" => GameEventType::UnitDestroyed,
        "unitdamaged" | "unit_damaged" => GameEventType::UnitDamaged,
        "unitmoved" | "unit_moved" => GameEventType::UnitMoved,
        "unitattacked" | "unit_attacked" => GameEventType::UnitAttacked,
        "weaponfired" | "weapon_fired" => GameEventType::WeaponFired,
        "combats_started" | "combatstarted" | "combat_started" => GameEventType::CombatStarted,
        "combatended" | "combat_ended" => GameEventType::CombatEnded,
        "playerdefeated" | "player_defeated" => GameEventType::PlayerDefeated,
        "playervictorious" | "player_victorious" => GameEventType::PlayerVictorious,
        "timerexpired" | "timer_expired" => GameEventType::TimerExpired,
        _ => GameEventType::Custom(name.to_string()),
    }
}

pub(super) fn compare_i64(actual: i64, comparison: &str, expected: i64) -> GameLogicResult<bool> {
    Ok(match comparison {
        "greater" => actual > expected,
        "less" => actual < expected,
        "equal" => actual == expected,
        "greater_equal" => actual >= expected,
        "less_equal" => actual <= expected,
        _ => {
            return Err(GameLogicError::Configuration(format!(
                "Invalid comparison operator: {}",
                comparison
            )));
        }
    })
}

pub(super) fn compare_f64(actual: f64, comparison: &str, expected: f64) -> GameLogicResult<bool> {
    Ok(match comparison {
        "greater" => actual > expected,
        "less" => actual < expected,
        "equal" => (actual - expected).abs() < 0.01,
        "greater_equal" => actual >= expected,
        "less_equal" => actual <= expected,
        _ => {
            return Err(GameLogicError::Configuration(format!(
                "Invalid comparison operator: {}",
                comparison
            )));
        }
    })
}

/// Helper: get string parameter from condition parameters
pub(crate) fn get_str_param(
    parameters: &HashMap<String, ScriptValue>,
    key: &str,
) -> GameLogicResult<String> {
    match parameters.get(key) {
        Some(ScriptValue::String(s)) => Ok(s.clone()),
        Some(v) => Err(GameLogicError::Configuration(format!(
            "Expected string for '{}', got {:?}",
            key, v
        ))),
        None => Err(GameLogicError::Configuration(format!(
            "Missing parameter '{}'",
            key
        ))),
    }
}

/// Helper: get optional string parameter
pub(super) fn get_str_param_optional(
    parameters: &HashMap<String, ScriptValue>,
    key: &str,
) -> Option<String> {
    match parameters.get(key) {
        Some(ScriptValue::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Helper: get player arc from parameter value
pub(crate) fn get_player_arc(
    parameters: &HashMap<String, ScriptValue>,
    key: &str,
) -> GameLogicResult<Option<Arc<RwLock<Player>>>> {
    let val = parameters
        .get(key)
        .ok_or_else(|| GameLogicError::Configuration(format!("Missing parameter '{}'", key)))?;
    match val {
        ScriptValue::PlayerId(id) => {
            let list = player_list();
            let guard = list.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read player list: {}", e))
            })?;
            Ok(guard.get_player(*id as i32).cloned())
        }
        ScriptValue::String(name) => {
            let list = player_list();
            let guard = list.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read player list: {}", e))
            })?;
            for i in 0..guard.get_player_count() {
                if let Some(player_arc) = guard.get_player(i as i32) {
                    if let Ok(player) = player_arc.read() {
                        if player.get_general_name() == name.as_str() {
                            return Ok(Some(player_arc.clone()));
                        }
                    }
                }
            }
            Ok(None)
        }
        ScriptValue::Int(id) => {
            let list = player_list();
            let guard = list.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read player list: {}", e))
            })?;
            Ok(guard.get_player(*id as i32).cloned())
        }
        _ => Err(GameLogicError::Configuration(format!(
            "Expected player id/name for '{}', got {:?}",
            key, val
        ))),
    }
}

/// Helper: look up a named object from the script engine's named object tracker.
/// Returns the ObjectID if found.
pub(crate) fn lookup_named_object_id(name: &str) -> GameLogicResult<Option<u32>> {
    let tracker = get_named_object_tracker();
    tracker.get_object_id(name)
}

/// Helper: perform C++-style comparison (less_than, less_equal, equal, etc.)
pub(crate) fn perform_comparison(actual: i64, comparison: &str, expected: i64) -> bool {
    match comparison.to_lowercase().as_str() {
        "less_than" | "<" => actual < expected,
        "less_equal" | "<=" => actual <= expected,
        "equal" | "==" | "=" => actual == expected,
        "greater_equal" | ">=" => actual >= expected,
        "greater" | ">" => actual > expected,
        "not_equal" | "!=" => actual != expected,
        _ => false,
    }
}

pub(super) fn with_script_engine_mut<R>(
    f: impl FnOnce(&mut crate::scripting::engine::ScriptEngine) -> R,
) -> Option<R> {
    let engine = get_script_engine();
    let mut engine_guard = engine.write().ok()?;
    engine_guard.as_mut().map(f)
}

pub(super) fn parse_nested_condition(
    value: &ScriptValue,
) -> GameLogicResult<(String, HashMap<String, ScriptValue>)> {
    match value {
        ScriptValue::Object(map) => {
            let name_value = map
                .get("name")
                .or_else(|| map.get("condition"))
                .or_else(|| map.get("type"))
                .ok_or_else(|| {
                    GameLogicError::Configuration(
                        "Nested condition object missing 'name'".to_string(),
                    )
                })?;
            let ScriptValue::String(name) = name_value else {
                return Err(GameLogicError::Configuration(
                    "Nested condition 'name' must be a string".to_string(),
                ));
            };

            let params = match map.get("parameters") {
                Some(ScriptValue::Object(params)) => params.clone(),
                Some(_) => {
                    return Err(GameLogicError::Configuration(
                        "Nested condition 'parameters' must be an object".to_string(),
                    ));
                }
                None => HashMap::new(),
            };

            Ok((name.clone(), params))
        }
        _ => Err(GameLogicError::Configuration(
            "Nested condition must be an object".to_string(),
        )),
    }
}

//-------------------------------------------------------------------------------------------------
// Helper: parse object status mask from string name
//-------------------------------------------------------------------------------------------------
pub(super) fn parse_object_status_mask(status_str: &str) -> crate::common::ObjectStatusMaskType {
    use crate::common::ObjectStatusMaskType as OSM;
    match status_str.to_lowercase().as_str() {
        "destroyed" => OSM::DESTROYED,
        "can_attack" => OSM::CAN_ATTACK,
        "under_construction" => OSM::UNDER_CONSTRUCTION,
        "unselectable" => OSM::UNSELECTABLE,
        "no_collisions" => OSM::NO_COLLISIONS,
        "no_attack" => OSM::NO_ATTACK,
        "airborne_target" => OSM::AIRBORNE_TARGET,
        "parachuting" => OSM::PARACHUTING,
        "hijacked" => OSM::HIJACKED,
        "aflame" => OSM::AFLAME,
        "burned" => OSM::BURNED,
        "stealthed" | "cloaked" => OSM::STEALTHED,
        "detected" => OSM::DETECTED,
        "can_stealth" => OSM::CAN_STEALTH,
        "sold" => OSM::SOLD,
        "undergoing_repair" => OSM::UNDERGOING_REPAIR,
        "reconstructing" => OSM::RECONSTRUCTING,
        "masked" => OSM::MASKED,
        "is_attacking" => OSM::IS_ATTACKING,
        "is_using_ability" => OSM::IS_USING_ABILITY,
        "is_aiming_weapon" => OSM::IS_AIMING_WEAPON,
        "no_attack_from_ai" => OSM::NO_ATTACK_FROM_AI,
        "ignoring_stealth" => OSM::IGNORING_STEALTH,
        "is_car_bomb" => OSM::IS_CAR_BOMB,
        "is_firing_weapon" => OSM::IS_FIRING_WEAPON,
        "braking" => OSM::BRAKING,
        "wet" => OSM::WET,
        "repulsor" => OSM::REPULSOR,
        "rider1" => OSM::RIDER1,
        "rider2" => OSM::RIDER2,
        "rider3" => OSM::RIDER3,
        "rider4" => OSM::RIDER4,
        "rider5" => OSM::RIDER5,
        "rider6" => OSM::RIDER6,
        "rider7" => OSM::RIDER7,
        "rider8" => OSM::RIDER8,
        _ => {
            log::warn!("Unknown object status: {}", status_str);
            OSM::NONE
        }
    }
}

#[cfg(test)]
mod tech_building_census_tests {
    use super::*;

    #[test]
    fn host_census_tech_building_none_without_snapshot() {
        clear_host_script_query_snapshot();
        assert_eq!(
            host_eval_skirmish_tech_building_within_distance("PlyrAmerica", 100.0, "HomeBase"),
            None
        );
    }

    #[test]
    fn host_census_tech_building_finds_neutral_derrick() {
        clear_host_script_query_snapshot();
        let mut snap = HostScriptQuerySnapshot::default();
        snap.areas.insert("HomeBase".into(), (0.0, 0.0, 20.0, 20.0));
        snap.tech_buildings.push(HostTechBuildingCensus {
            x: 10.0,
            z: 10.0,
            owner_player: String::new(),
            team: 3,
            off_map: false,
        });
        set_host_script_query_snapshot(snap);
        assert_eq!(
            host_eval_skirmish_tech_building_within_distance("PlyrAmerica", 100.0, "HomeBase"),
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
        set_host_script_query_snapshot(own);
        assert_eq!(
            host_eval_skirmish_tech_building_within_distance("PlyrAmerica", 100.0, "HomeBase"),
            Some(false)
        );
        clear_host_script_query_snapshot();
    }

    #[test]
    fn host_census_tech_building_missing_area_is_none() {
        clear_host_script_query_snapshot();
        let mut snap = HostScriptQuerySnapshot::default();
        snap.tech_buildings.push(HostTechBuildingCensus {
            x: 10.0,
            z: 10.0,
            ..Default::default()
        });
        set_host_script_query_snapshot(snap);
        assert_eq!(
            host_eval_skirmish_tech_building_within_distance("PlyrAmerica", 100.0, "MissingPad"),
            None
        );
        clear_host_script_query_snapshot();
    }
}

#[cfg(test)]
mod host_skirmish_discovered_prereq_tests {
    use super::*;

    #[test]
    fn host_skirmish_discovered_none_without_snapshot() {
        clear_host_script_query_snapshot();
        assert_eq!(
            host_eval_skirmish_player_has_discovered_player("PlyrChina", "PlyrAmerica"),
            None
        );
    }

    #[test]
    fn host_skirmish_discovered_uses_discovered_by() {
        clear_host_script_query_snapshot();
        let mut snap = HostScriptQuerySnapshot::default();
        snap.objects.push(HostScriptQueryObject {
            id: 7,
            owner_player: "PlyrChina".into(),
            discovered_by: vec!["PlyrAmerica".into()],
            ..Default::default()
        });
        set_host_script_query_snapshot(snap);
        assert_eq!(
            host_eval_skirmish_player_has_discovered_player("PlyrChina", "PlyrAmerica"),
            Some(true)
        );
        assert_eq!(
            host_eval_skirmish_player_has_discovered_player("PlyrChina", "PlyrGLA"),
            Some(false)
        );
        clear_host_script_query_snapshot();
    }

    #[test]
    fn host_skirmish_prereq_none_without_census() {
        clear_host_script_query_snapshot();
        assert_eq!(
            host_eval_skirmish_player_has_prerequisite_to_build("PlyrAmerica", "AmericaWarFactory"),
            None
        );
    }

    #[test]
    fn host_skirmish_prereq_false_without_leftover_template() {
        clear_host_script_query_snapshot();
        let mut snap = HostScriptQuerySnapshot::default();
        let mut census = HostScriptPlayerCensus::default();
        census
            .template_counts
            .insert("americacommandcenter".into(), 1);
        snap.player_census.insert("plyramerica".into(), census);
        set_host_script_query_snapshot(snap);
        assert_eq!(
            host_eval_skirmish_player_has_prerequisite_to_build("PlyrAmerica", "AmericaWarFactory"),
            Some(false)
        );
        clear_host_script_query_snapshot();
    }
}
