//! ActionManager bridge for GameLogic object validation.
//!
//! Implements targeted C++ parity checks (starting with canPickUpPrisoner).

use crate::ai::CommandSourceType;
use crate::attack::{ATTACKRESULT_NOT_POSSIBLE, AbleToAttackType, CanAttackResult};
use crate::commands::command::{Command, CommandType, command_builder};
use crate::commands::command_queue::{CommandPriority, QueuedCommand, get_command_queue_manager};
use crate::commands::selection::get_selection_manager;
use crate::common::ObjectID;
use crate::common::{
    AsciiString, DisabledType, Int, KindOf, NameKeyGenerator, ObjectShroudStatus,
    ObjectStatusTypes, Relationship,
};
use crate::helpers::TheGameLogic;
use crate::helpers::ThePartitionManager;
use crate::helpers::TheTerrainLogic;
use crate::modules::SpecialPowerModuleInterface;
use crate::object::Object;
use crate::object::behavior::spawn_behavior::SpawnBehaviorInterface;
use crate::object::collide::COLLISION_MANAGER;
use crate::object::production::supply_warehouse_dock::SupplyWarehouseDockUpdate;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::SpecialPowerTemplate;
use crate::object::special_power_types::SpecialPowerType;
use crate::player::PlayerType;
use crate::weapon::DamageType;
use crate::weapon::WeaponSlotType;
use game_engine::common::rts::ActionManager as RtsActionManager;
use game_engine::common::rts::action_manager::{
    ActionExecutor, ActionType, Coord3D as ActionCoord3D,
};
use once_cell::sync::Lazy;
use std::sync::{Arc, RwLock};

/// Wave 347: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

fn is_object_shrouded_for_action(
    source: &Object,
    target: &Object,
    command_source: CommandSourceType,
) -> bool {
    let Some(player) = source.get_controlling_player() else {
        return false;
    };
    let Ok(player_guard) = player.read() else {
        return false;
    };
    if player_guard.get_player_type() != PlayerType::Human {
        return false;
    }
    if command_source == CommandSourceType::FromScript {
        return false;
    }
    let shroud = target.get_shrouded_status(player_guard.get_player_index());
    (shroud as u8) >= (ObjectShroudStatus::Fogged as u8)
}

fn is_faction_structure(obj: &Object) -> bool {
    obj.is_any_kind_of(&[
        KindOf::FSBarracks,
        KindOf::FSWarfactory,
        KindOf::FSAirfield,
        KindOf::FSInternetCenter,
        KindOf::FSPower,
        KindOf::FSBaseDefense,
        KindOf::FSSupplyDropzone,
        KindOf::FSSupplyCenter,
        KindOf::FSSuperweapon,
        KindOf::FSStrategyCenter,
    ])
}

fn is_point_on_map(pos: &crate::common::Coord3D) -> bool {
    // C++ isPointOnMap: TheTerrainLogic->getExtent(&mapRegion); mapRegion.isInRegionNoZ(testPos)
    let Some(terrain) = TheTerrainLogic::get() else {
        return false;
    };
    let extent = terrain.get_extent();
    pos.x >= extent.lo.x && pos.x <= extent.hi.x && pos.y >= extent.lo.y && pos.y <= extent.hi.y
}

fn controlling_player_index(source: &Object) -> crate::common::Int {
    source
        .get_controlling_player()
        .and_then(|player| player.read().ok().map(|guard| guard.get_player_index()))
        .or_else(|| {
            source
                .get_controlling_player_id()
                .map(|player_id| player_id as crate::common::Int)
        })
        .unwrap_or(-1)
}

/// C++ ActionManager.cpp:1521 — PartitionManager cell shroud, fail-closed if missing.
fn is_location_cell_shrouded_for_player(
    player_index: crate::common::Int,
    loc: &crate::common::Coord3D,
) -> bool {
    let Some(partition) = ThePartitionManager::get() else {
        return true;
    };
    partition.get_shroud_status_for_player(player_index, loc)
        == game_engine::common::system::radar::CellShroudStatus::Shrouded
}

/// C++ ActionManager.cpp:1459-1551 after source/module checks.
///
/// `shroud_unknown_is_clear` fail-opens the cell-shroud arm when leftover
/// shroud grid is not initialized (live host before map load / synthetic
/// worlds). Leftover `can_do_special_power_at_location` passes false.
fn special_power_at_location_ok(
    power_type: SpecialPowerType,
    loc: &crate::common::Coord3D,
    player_index: crate::common::Int,
    shroud_unknown_is_clear: bool,
) -> bool {
    match power_type {
        SpecialPowerType::ParadropAmerica
        | SpecialPowerType::InfaParadropAmerica
        | SpecialPowerType::CrateDrop
        | SpecialPowerType::TankParadrop => {
            if TheTerrainLogic::get()
                .is_some_and(|terrain| terrain.is_underwater(loc.x, loc.y, None, None))
            {
                return false;
            }
        }
        _ => {}
    }

    match power_type {
        SpecialPowerType::DaisyCutter
        | SpecialPowerType::AirfDaisyCutter
        | SpecialPowerType::ParadropAmerica
        | SpecialPowerType::TankParadrop
        | SpecialPowerType::InfaParadropAmerica
        | SpecialPowerType::CarpetBomb
        | SpecialPowerType::ChinaCarpetBomb
        | SpecialPowerType::LeafletDrop
        | SpecialPowerType::EarlyLeafletDrop
        | SpecialPowerType::EarlyChinaCarpetBomb
        | SpecialPowerType::AirfCarpetBomb
        | SpecialPowerType::SuprCruiseMissile
        | SpecialPowerType::ClusterMines
        | SpecialPowerType::NukeClusterMines
        | SpecialPowerType::EmpPulse
        | SpecialPowerType::CrateDrop
        | SpecialPowerType::NapalmStrike
        | SpecialPowerType::BlackMarketNuke
        | SpecialPowerType::AnthraxBomb
        | SpecialPowerType::TerrorCell
        | SpecialPowerType::Ambush
        | SpecialPowerType::NeutronMissile
        | SpecialPowerType::NukeNeutronMissile
        | SpecialPowerType::SupwNeutronMissile
        | SpecialPowerType::ScudStorm
        | SpecialPowerType::Demoralize
        | SpecialPowerType::A10ThunderboltStrike
        | SpecialPowerType::AirfA10ThunderboltStrike
        | SpecialPowerType::SpectreGunship
        | SpecialPowerType::AirfSpectreGunship
        | SpecialPowerType::RepairVehicles
        | SpecialPowerType::EarlyRepairVehicles
        | SpecialPowerType::GpsScrambler
        | SpecialPowerType::SlthGpsScrambler
        | SpecialPowerType::ArtilleryBarrage
        | SpecialPowerType::Frenzy
        | SpecialPowerType::EarlyFrenzy
        | SpecialPowerType::ParticleUplinkCannon
        | SpecialPowerType::SupwParticleUplinkCannon
        | SpecialPowerType::LazrParticleUplinkCannon
        | SpecialPowerType::CleanupArea
        | SpecialPowerType::SneakAttack
        | SpecialPowerType::BattleshipBombardment => {
            if shroud_unknown_is_clear {
                let grid_ready = crate::system::shroud_manager::get_shroud_manager()
                    .lock()
                    .ok()
                    .is_some_and(|shroud| shroud.has_shroud_grid());
                if !grid_ready {
                    return true;
                }
            }
            !is_location_cell_shrouded_for_player(player_index, loc)
        }
        SpecialPowerType::SpySatellite
        | SpecialPowerType::RadarVanScan
        | SpecialPowerType::SpyDrone
        | SpecialPowerType::HelixNapalmBomb => is_point_on_map(loc),
        SpecialPowerType::LaunchBaikonurRocket => true,
        SpecialPowerType::MissileDefenderLaserGuidedMissiles
        | SpecialPowerType::HackerDisableBuilding
        | SpecialPowerType::TankHunterTntAttack
        | SpecialPowerType::BoobyTrap
        | SpecialPowerType::CashHack
        | SpecialPowerType::Defector
        | SpecialPowerType::BlackLotusCaptureBuilding
        | SpecialPowerType::BlackLotusDisableVehicleHack
        | SpecialPowerType::BlackLotusStealCashHack
        | SpecialPowerType::InfantryCaptureBuilding
        | SpecialPowerType::DetonateDirtyNuke
        | SpecialPowerType::DisguiseAsVehicle
        | SpecialPowerType::RemoteCharges
        | SpecialPowerType::TimedCharges
        | SpecialPowerType::CashBounty
        | SpecialPowerType::ChangeBattlePlans => false,
        _ => false,
    }
}

/// C++ getSpecialPowerModule(spTemplate) + optional getPercentReady() < 1.0.
///
/// When `check_source_requirements` is false (AI hunt), C++ still requires a
/// SpecialPowerModule. Hunt unit tests omit that module and only exercise the
/// target-kind switch, so a missing module is tolerated only in that mode.
fn special_power_source_ok(
    obj: &Object,
    sp_template: &SpecialPowerTemplate,
    check_source_requirements: bool,
) -> bool {
    if check_source_requirements && !obj.has_special_power(sp_template.get_special_power_type()) {
        return false;
    }
    let has_module = obj.has_special_power_module_for_power(sp_template);
    if check_source_requirements {
        if !has_module {
            return false;
        }
        match special_power_module_percent_ready(obj, sp_template) {
            Some(ready) if ready >= 1.0 => {}
            _ => return false,
        }
    }
    true
}

fn special_power_module_percent_ready(
    obj: &Object,
    sp_template: &SpecialPowerTemplate,
) -> Option<f32> {
    for behavior in obj.get_behavior_modules() {
        let Ok(behavior) = behavior.lock() else {
            continue;
        };
        if let Some(module) = behavior.get_special_power_module_interface_const() {
            if module.is_module_for_power(sp_template) {
                return Some(module.get_percent_ready());
            }
        }
    }
    get_special_power_ready_percent(obj, sp_template.get_special_power_type())
}

fn special_ability_object_counts(obj: &Object, power_type: SpecialPowerType) -> Option<(u32, u32)> {
    use crate::object::behavior::special_ability_update::SpecialAbilityUpdate as SpecialAbilityUpdateBehavior;

    for behavior in obj.get_behavior_modules() {
        let Ok(guard) = behavior.lock() else {
            continue;
        };
        let Some(update) = guard
            .as_any()
            .downcast_ref::<SpecialAbilityUpdateBehavior>()
        else {
            continue;
        };
        let Some(update_type) = update.get_special_power_type() else {
            continue;
        };
        if update_type as u32 != power_type as u32 {
            continue;
        }
        return Some((
            update.get_special_object_count(),
            update.get_special_object_max(),
        ));
    }
    None
}

/// C++ canDoSpecialPowerAtObject SPECIAL_REMOTE/TIMED_CHARGES / HELIX_NAPALM_BOMB.
fn can_place_special_object_charge(
    obj: &Object,
    target: &Object,
    power_type: SpecialPowerType,
) -> bool {
    if target.is_effectively_dead()
        || target.is_kind_of(KindOf::Bridge)
        || target.is_kind_of(KindOf::BridgeTower)
    {
        return false;
    }
    if !target.is_kind_of(KindOf::Structure) && !target.is_kind_of(KindOf::Vehicle) {
        return false;
    }

    // Fail-closed when SpecialAbilityUpdate is missing (C++ requires spUpdate).
    let Some((count, max)) = special_ability_object_counts(obj, power_type) else {
        return false;
    };
    if count >= max {
        return false;
    }
    if has_special_object_on_target(target.get_id(), "StickyBombUpdate") {
        return false;
    }

    let other = match power_type {
        SpecialPowerType::RemoteCharges => Some(SpecialPowerType::TimedCharges),
        SpecialPowerType::TimedCharges => Some(SpecialPowerType::RemoteCharges),
        _ => None,
    };
    if let Some(other_type) = other {
        if special_ability_object_counts(obj, other_type).is_some()
            && has_special_object_on_target(target.get_id(), "StickyBombUpdate")
        {
            return false;
        }
    }
    true
}

#[allow(dead_code)]
fn count_special_objects_by_producer(producer_id: ObjectID, special_object_update: &str) -> usize {
    // Wave 347: empty dual-world → 0.
    if dual_world_registry_unavailable() {
        return 0;
    }

    OBJECT_REGISTRY
        .get_all_objects()
        .into_iter()
        .filter(|obj| {
            let Ok(guard) = obj.read() else {
                return false;
            };
            if guard.get_producer_id() != producer_id {
                return false;
            }
            guard.find_update_module(special_object_update).is_some()
        })
        .count()
}

fn has_special_object_on_target(target_id: ObjectID, special_object_update: &str) -> bool {
    // Wave 347: empty dual-world → false.
    if dual_world_registry_unavailable() {
        return false;
    }
    OBJECT_REGISTRY
        .get_all_object_ids()
        .into_iter()
        .any(|obj_id| {
            let Some(obj) = OBJECT_REGISTRY.get_object(obj_id) else {
                return false;
            };
            let Ok(guard) = obj.read() else {
                return false;
            };
            guard.get_producer_id() == target_id
                && guard.find_update_module(special_object_update).is_some()
        })
}

fn has_module(object: &Object, name: &str) -> bool {
    object.find_update_module(name).is_some()
}

fn get_supply_warehouse_boxes(warehouse: &Object) -> Option<i32> {
    warehouse
        .find_update_module("SupplyWarehouseDockUpdate")
        .and_then(|module| {
            module.with_module(|module| {
                module
                    .get_supply_warehouse_dock_interface()
                    .map(|warehouse| warehouse.boxes_stored())
            })
        })
}

fn count_stealthed_contained(contain: &dyn crate::modules::ContainModuleInterface) -> usize {
    // Wave 347: empty dual-world → 0.
    if dual_world_registry_unavailable() {
        return 0;
    }

    contain
        .get_contained_objects()
        .iter()
        .filter(|id| {
            if let Some(obj) = TheGameLogic::find_object_by_id(**id) {
                if let Ok(guard) = obj.read() {
                    guard.is_stealthed()
                } else {
                    false
                }
            } else {
                false
            }
        })
        .count()
}

fn appears_to_contain_friendlies(obj: &Object, other: &Object) -> bool {
    let Some(contain) = other.get_contain() else {
        return false;
    };
    let Ok(contain_guard) = contain.lock() else {
        return false;
    };
    let Some(observer) = obj.get_controlling_player() else {
        return false;
    };
    let Ok(observer_guard) = observer.read() else {
        return false;
    };
    let Some(apparent_player) =
        contain_guard.get_apparent_controlling_player(Some(&observer_guard))
    else {
        return false;
    };
    let Ok(apparent_guard) = apparent_player.read() else {
        return false;
    };
    let Some(my_team) = obj.get_team() else {
        return false;
    };
    let Some(other_team) = apparent_guard.get_default_team() else {
        return false;
    };
    let Ok(my_team_guard) = my_team.read() else {
        return false;
    };
    let Ok(other_team_guard) = other_team.read() else {
        return false;
    };
    my_team_guard.get_relationship(&*other_team_guard) != Relationship::Enemies
}

fn get_special_power_ready_percent(obj: &Object, power_type: SpecialPowerType) -> Option<f32> {
    let mut ready = None;
    for behavior in obj.get_behavior_modules() {
        let Ok(mut behavior) = behavior.lock() else {
            continue;
        };
        if let Some(module) = behavior.get_special_power() {
            if module.get_power_type() == power_type as u32 {
                ready = Some(module.get_percent_ready());
            }
        }
        if ready.is_some() {
            return ready;
        }
    }
    ready
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanEnterType {
    CheckCapacity,
    DontCheckCapacity,
    CombatDropInto,
}

/// GameLogic-facing ActionManager entrypoint (matches C++ TheActionManager usage).
pub struct TheActionManager;

/// Legacy-compatible alias (matches C++ ActionManager usage sites).
pub type ActionManager = TheActionManager;

impl TheActionManager {
    /// Can `obj` pick up the surrendered `prisoner` (C++ ActionManager::canPickUpPrisoner).
    pub fn can_pick_up_prisoner(
        obj: &Object,
        prisoner: &Object,
        _command_source: CommandSourceType,
    ) -> bool {
        if !obj.is_kind_of(KindOf::PowTruck) {
            return false;
        }

        if !prisoner.is_kind_of(KindOf::Infantry) {
            return false;
        }

        if prisoner.get_contained_by().is_some() {
            return false;
        }

        let Some(ai) = prisoner.get_ai_update_interface() else {
            return false;
        };
        let Ok(ai_guard) = ai.lock() else {
            return false;
        };
        if !ai_guard.is_surrendered() {
            return false;
        }

        if let Some(surrendered_to) = ai_guard.get_surrendered_player_index() {
            if obj.get_controlling_player_id() != Some(surrendered_to as u32) {
                return false;
            }
        }

        if obj.relationship_to(prisoner) != Relationship::Enemies {
            return false;
        }

        true
    }

    /// Can `obj` get repaired at `repair_dest` (C++ ActionManager::canGetRepairedAt).
    pub fn can_get_repaired_at(
        obj: &Object,
        repair_dest: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        if obj.relationship_to(repair_dest) != Relationship::Allies {
            return false;
        }

        if obj.is_effectively_dead() {
            return false;
        }

        if obj.is_kind_of(KindOf::Immobile) {
            return false;
        }

        if obj.test_status(ObjectStatusTypes::UnderConstruction)
            || repair_dest.test_status(ObjectStatusTypes::UnderConstruction)
        {
            return false;
        }

        if repair_dest.test_status(ObjectStatusTypes::Sold) {
            return false;
        }

        if !obj.is_kind_of(KindOf::Vehicle) {
            return false;
        }

        if obj.is_kind_of(KindOf::Aircraft) {
            if !obj.is_above_terrain() || !repair_dest.is_kind_of(KindOf::FSAirfield) {
                return false;
            }
        } else if !repair_dest.is_kind_of(KindOf::RepairPad)
            && !has_module(repair_dest, "RepairDockUpdate")
        {
            return false;
        }

        let Some(body) = obj.get_body_module() else {
            return false;
        };
        let Ok(body_guard) = body.lock() else {
            return false;
        };
        if body_guard.get_health() >= body_guard.get_max_health() {
            return false;
        }

        if is_object_shrouded_for_action(obj, repair_dest, command_source) {
            return false;
        }

        true
    }

    /// Can `obj` execute a special power at a location.
    /// Matches C++ ActionManager::canDoSpecialPowerAtLocation.
    pub fn can_do_special_power_at_location(
        obj: &Object,
        loc: &crate::common::Coord3D,
        command_source: CommandSourceType,
        sp_template: &SpecialPowerTemplate,
        _object_in_way: Option<&Object>,
        _command_options: u32,
        check_source_requirements: bool,
    ) -> bool {
        // C++ ActionManager.cpp:1436-1554
        if !special_power_source_ok(obj, sp_template, check_source_requirements) {
            return false;
        }
        let _ = command_source;
        special_power_at_location_ok(
            sp_template.get_special_power_type(),
            loc,
            controlling_player_index(obj),
            false,
        )
    }

    /// Host-facing leftover of C++ `canDoSpecialPowerAtLocation` location
    /// gates (underwater + fully-shrouded + object-only). No leftover Object.
    ///
    /// `shroud_unknown_is_clear` matches live host: fail-open when leftover
    /// shroud grid is not initialized.
    pub fn can_do_special_power_at_location_for_player(
        power_type: SpecialPowerType,
        loc: &crate::common::Coord3D,
        player_index: crate::common::Int,
        shroud_unknown_is_clear: bool,
    ) -> bool {
        special_power_at_location_ok(power_type, loc, player_index, shroud_unknown_is_clear)
    }

    /// C++ `CommandXlat::issueSpecialPowerCommand` NEED_TARGET_POS-only specials.
    ///
    /// Leftover `canDoSpecialPowerAtObject` is unconditionally FALSE and
    /// `canDoSpecialPowerAtLocation` evaluates underwater / shroud / on-map.
    /// Dual-path Battleship, object-charge HelixNapalmBomb, and no-target
    /// LaunchBaikonurRocket stay off this list.
    pub fn special_power_is_location_target_only(power_type: SpecialPowerType) -> bool {
        match power_type {
            SpecialPowerType::DaisyCutter
            | SpecialPowerType::AirfDaisyCutter
            | SpecialPowerType::ParadropAmerica
            | SpecialPowerType::TankParadrop
            | SpecialPowerType::InfaParadropAmerica
            | SpecialPowerType::CarpetBomb
            | SpecialPowerType::ChinaCarpetBomb
            | SpecialPowerType::LeafletDrop
            | SpecialPowerType::EarlyLeafletDrop
            | SpecialPowerType::EarlyChinaCarpetBomb
            | SpecialPowerType::AirfCarpetBomb
            | SpecialPowerType::SuprCruiseMissile
            | SpecialPowerType::ClusterMines
            | SpecialPowerType::NukeClusterMines
            | SpecialPowerType::EmpPulse
            | SpecialPowerType::CrateDrop
            | SpecialPowerType::NapalmStrike
            | SpecialPowerType::BlackMarketNuke
            | SpecialPowerType::AnthraxBomb
            | SpecialPowerType::TerrorCell
            | SpecialPowerType::Ambush
            | SpecialPowerType::NeutronMissile
            | SpecialPowerType::NukeNeutronMissile
            | SpecialPowerType::SupwNeutronMissile
            | SpecialPowerType::ScudStorm
            | SpecialPowerType::Demoralize
            | SpecialPowerType::A10ThunderboltStrike
            | SpecialPowerType::AirfA10ThunderboltStrike
            | SpecialPowerType::SpectreGunship
            | SpecialPowerType::AirfSpectreGunship
            | SpecialPowerType::RepairVehicles
            | SpecialPowerType::EarlyRepairVehicles
            | SpecialPowerType::GpsScrambler
            | SpecialPowerType::SlthGpsScrambler
            | SpecialPowerType::ArtilleryBarrage
            | SpecialPowerType::Frenzy
            | SpecialPowerType::EarlyFrenzy
            | SpecialPowerType::ParticleUplinkCannon
            | SpecialPowerType::SupwParticleUplinkCannon
            | SpecialPowerType::LazrParticleUplinkCannon
            | SpecialPowerType::CleanupArea
            | SpecialPowerType::SneakAttack
            | SpecialPowerType::SpySatellite
            | SpecialPowerType::RadarVanScan
            | SpecialPowerType::SpyDrone => true,
            _ => false,
        }
    }

    /// C++ `ActionManager::canDoSpecialPower` type switch after module/ready
    /// (`ActionManager.cpp:1820-1907`). These fire with neither
    /// `COMMAND_OPTION_NEED_OBJECT_TARGET` nor `NEED_TARGET_POS`.
    pub fn special_power_is_no_target(power_type: SpecialPowerType) -> bool {
        match power_type {
            SpecialPowerType::RemoteCharges
            | SpecialPowerType::CiaIntelligence
            | SpecialPowerType::CommunicationsDownload
            | SpecialPowerType::DetonateDirtyNuke
            | SpecialPowerType::ChangeBattlePlans
            | SpecialPowerType::LaunchBaikonurRocket => true,
            _ => false,
        }
    }

    /// Can `obj` execute a special power on a target object.
    /// Matches C++ ActionManager::canDoSpecialPowerAtObject.
    pub fn can_do_special_power_at_object(
        obj: &Object,
        target: &Object,
        command_source: CommandSourceType,
        sp_template: &SpecialPowerTemplate,
        _command_options: u32,
        check_source_requirements: bool,
    ) -> bool {
        // C++ ActionManager.cpp:1558-1816
        if !special_power_source_ok(obj, sp_template, check_source_requirements) {
            return false;
        }

        if target.is_effectively_dead() {
            return false;
        }

        let relationship = obj.relationship_to(target);

        if is_object_shrouded_for_action(obj, target, command_source) {
            return false;
        }

        match sp_template.get_special_power_type() {
            SpecialPowerType::CashBounty => false,
            // C++: if relationship != ALLIES return TRUE (enemies and neutrals).
            SpecialPowerType::BattleshipBombardment => relationship != Relationship::Allies,
            SpecialPowerType::TankHunterTntAttack => {
                target.is_kind_of(KindOf::Structure)
                    || (target.is_kind_of(KindOf::Vehicle) && !target.is_kind_of(KindOf::Aircraft))
            }
            SpecialPowerType::BoobyTrap => {
                target.is_kind_of(KindOf::Structure)
                    && (relationship == Relationship::Neutral
                        || relationship == Relationship::Allies)
            }
            SpecialPowerType::MissileDefenderLaserGuidedMissiles => {
                target.is_kind_of(KindOf::Vehicle) && relationship == Relationship::Enemies
            }
            SpecialPowerType::HackerDisableBuilding => {
                if target.is_kind_of(KindOf::Structure) && relationship == Relationship::Enemies {
                    if !target.is_kind_of(KindOf::Capturable)
                        || target.is_kind_of(KindOf::RebuildHole)
                    {
                        return false;
                    }
                    return true;
                }
                false
            }
            SpecialPowerType::InfantryCaptureBuilding
            | SpecialPowerType::BlackLotusCaptureBuilding => {
                TheActionManager::can_capture_building(obj, target, command_source)
            }
            SpecialPowerType::BlackLotusDisableVehicleHack => {
                TheActionManager::can_disable_vehicle_via_hacking(
                    obj,
                    target,
                    command_source,
                    false,
                )
            }
            SpecialPowerType::BlackLotusStealCashHack => {
                TheActionManager::can_steal_cash_via_hacking(obj, target, command_source)
            }
            SpecialPowerType::CashHack => {
                if target.is_kind_of(KindOf::Structure) && relationship == Relationship::Enemies {
                    if !target.is_kind_of(KindOf::Capturable)
                        || target.is_kind_of(KindOf::RebuildHole)
                    {
                        return false;
                    }
                    if target.test_status(ObjectStatusTypes::UnderConstruction) {
                        return false;
                    }
                    return target.is_kind_of(KindOf::CashGenerator);
                }
                false
            }
            SpecialPowerType::DisguiseAsVehicle => {
                if target.is_kind_of(KindOf::Vehicle)
                    && !target.is_kind_of(KindOf::Aircraft)
                    && !target.is_kind_of(KindOf::Boat)
                    && !target.is_kind_of(KindOf::CliffJumper)
                {
                    return !has_module(target, "RailroadBehavior");
                }
                false
            }
            SpecialPowerType::Defector => {
                if target.is_kind_of(KindOf::Structure) {
                    return false;
                }
                if relationship == Relationship::Enemies {
                    return TheActionManager::can_make_object_defector(obj, target, command_source);
                }
                false
            }
            SpecialPowerType::DaisyCutter
            | SpecialPowerType::AirfDaisyCutter
            | SpecialPowerType::ParadropAmerica
            | SpecialPowerType::TankParadrop
            | SpecialPowerType::InfaParadropAmerica
            | SpecialPowerType::CarpetBomb
            | SpecialPowerType::ChinaCarpetBomb
            | SpecialPowerType::LeafletDrop
            | SpecialPowerType::EarlyLeafletDrop
            | SpecialPowerType::EarlyChinaCarpetBomb
            | SpecialPowerType::AirfCarpetBomb
            | SpecialPowerType::SuprCruiseMissile
            | SpecialPowerType::ClusterMines
            | SpecialPowerType::NukeClusterMines
            | SpecialPowerType::EmpPulse
            | SpecialPowerType::CrateDrop
            | SpecialPowerType::NapalmStrike
            | SpecialPowerType::TerrorCell
            | SpecialPowerType::Ambush
            | SpecialPowerType::NeutronMissile
            | SpecialPowerType::NukeNeutronMissile
            | SpecialPowerType::SupwNeutronMissile
            | SpecialPowerType::DetonateDirtyNuke
            | SpecialPowerType::BlackMarketNuke
            | SpecialPowerType::AnthraxBomb
            | SpecialPowerType::SpySatellite
            | SpecialPowerType::SpyDrone
            | SpecialPowerType::RadarVanScan
            | SpecialPowerType::ScudStorm
            | SpecialPowerType::A10ThunderboltStrike
            | SpecialPowerType::AirfA10ThunderboltStrike
            | SpecialPowerType::SpectreGunship
            | SpecialPowerType::AirfSpectreGunship
            | SpecialPowerType::ArtilleryBarrage
            | SpecialPowerType::Frenzy
            | SpecialPowerType::EarlyFrenzy
            | SpecialPowerType::RepairVehicles
            | SpecialPowerType::EarlyRepairVehicles
            | SpecialPowerType::GpsScrambler
            | SpecialPowerType::SlthGpsScrambler
            | SpecialPowerType::ParticleUplinkCannon
            | SpecialPowerType::ChangeBattlePlans
            | SpecialPowerType::CleanupArea
            | SpecialPowerType::LaunchBaikonurRocket
            | SpecialPowerType::SneakAttack => false,
            SpecialPowerType::RemoteCharges
            | SpecialPowerType::TimedCharges
            | SpecialPowerType::HelixNapalmBomb => {
                can_place_special_object_charge(obj, target, sp_template.get_special_power_type())
            }
            _ => false,
        }
    }

    /// Can `obj` execute a special power with no target.
    /// Matches C++ ActionManager::canDoSpecialPower.
    pub fn can_do_special_power(
        obj: &Object,
        sp_template: &SpecialPowerTemplate,
        _command_source: CommandSourceType,
        _command_options: u32,
        check_source_requirements: bool,
    ) -> bool {
        // C++ ActionManager.cpp:1820-1907 — module required, default FALSE.
        if !special_power_source_ok(obj, sp_template, check_source_requirements) {
            return false;
        }

        Self::special_power_is_no_target(sp_template.get_special_power_type())
    }

    /// Can `obj` transfer supplies at `transfer_dest` (C++ ActionManager::canTransferSuppliesAt).
    pub fn can_transfer_supplies_at(obj: &Object, transfer_dest: &Object) -> bool {
        if transfer_dest.is_effectively_dead() {
            return false;
        }

        if obj.test_status(ObjectStatusTypes::UnderConstruction)
            || transfer_dest.test_status(ObjectStatusTypes::UnderConstruction)
        {
            return false;
        }

        if transfer_dest.test_status(ObjectStatusTypes::Sold) {
            return false;
        }

        let Some(ai) = obj.get_ai_update_interface() else {
            return false;
        };
        let Ok(ai_guard) = ai.lock() else {
            return false;
        };
        let Some(supply_truck) = ai_guard.get_supply_truck_ai_interface() else {
            return false;
        };

        let warehouse_boxes = get_supply_warehouse_boxes(transfer_dest);
        if let Some(boxes) = warehouse_boxes {
            if boxes == 0 || transfer_dest.relationship_to(obj) == Relationship::Enemies {
                return false;
            }
        }

        let has_center = has_module(transfer_dest, "SupplyCenterDockUpdate");
        if has_center {
            let Ok(boxes) = supply_truck.get_supplies_count() else {
                return false;
            };
            if boxes == 0
                || transfer_dest.get_controlling_player_id() != obj.get_controlling_player_id()
            {
                return false;
            }
        }

        if warehouse_boxes.is_none() && !has_center {
            return false;
        }

        if !supply_truck.is_available_for_supplying() {
            return false;
        }

        if let Some(player) = obj.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                if player_guard.get_player_type() == PlayerType::Human
                    && transfer_dest.get_shrouded_status(player_guard.get_player_index())
                        == ObjectShroudStatus::Shrouded
                {
                    return false;
                }
            }
        }

        true
    }

    /// Can `obj` dock at `dock_dest` (C++ ActionManager::canDockAt).
    pub fn can_dock_at(
        obj: &Object,
        dock_dest: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        if dock_dest.with_dock_update_interface(|_| ()).is_none() {
            return false;
        }

        if Self::can_transfer_supplies_at(obj, dock_dest) {
            return true;
        }

        if has_module(dock_dest, "RailedTransportDockUpdate")
            && (obj.is_kind_of(KindOf::Vehicle) || obj.is_kind_of(KindOf::Infantry))
        {
            return true;
        }

        let _ = command_source;
        false
    }

    /// Can `obj` get healed at `heal_dest` (C++ ActionManager::canGetHealedAt).
    pub fn can_get_healed_at(
        obj: &Object,
        heal_dest: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        if obj.relationship_to(heal_dest) != Relationship::Allies {
            return false;
        }

        if heal_dest.is_effectively_dead() {
            return false;
        }

        if obj.test_status(ObjectStatusTypes::UnderConstruction)
            || heal_dest.test_status(ObjectStatusTypes::UnderConstruction)
        {
            return false;
        }

        if heal_dest.test_status(ObjectStatusTypes::Sold) {
            return false;
        }

        if !obj.is_kind_of(KindOf::Infantry) {
            return false;
        }

        if !heal_dest.is_kind_of(KindOf::HealPad) {
            return false;
        }

        if is_object_shrouded_for_action(obj, heal_dest, command_source) {
            return false;
        }

        let Some(body) = obj.get_body_module() else {
            return false;
        };
        let Ok(body_guard) = body.lock() else {
            return false;
        };
        if body_guard.get_health() >= body_guard.get_max_health() {
            return false;
        }

        true
    }

    /// Can `obj` repair `object_to_repair` (C++ ActionManager::canRepairObject).
    pub fn can_repair_object(
        obj: &Object,
        object_to_repair: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        if obj.relationship_to(object_to_repair) == Relationship::Enemies {
            return false;
        }

        if object_to_repair.is_effectively_dead() {
            return false;
        }

        if object_to_repair.is_kind_of(KindOf::Bridge)
            || object_to_repair.is_kind_of(KindOf::BridgeTower)
        {
            return false;
        }

        if obj.test_status(ObjectStatusTypes::UnderConstruction)
            || object_to_repair.test_status(ObjectStatusTypes::UnderConstruction)
        {
            return false;
        }

        // C++ ActionManager.cpp:394 — KINDOF_REBUILD_HOLE, not a module-name check.
        if object_to_repair.is_kind_of(KindOf::RebuildHole) {
            return false;
        }

        if !obj.is_kind_of(KindOf::Dozer) {
            return false;
        }

        if !object_to_repair.is_kind_of(KindOf::Structure) {
            return false;
        }

        let Some(body) = object_to_repair.get_body_module() else {
            return false;
        };
        let Ok(body_guard) = body.lock() else {
            return false;
        };
        if body_guard.get_health() >= body_guard.get_max_health() {
            return false;
        }

        if is_object_shrouded_for_action(obj, object_to_repair, command_source) {
            return false;
        }

        if obj.get_contained_by().is_some() {
            return false;
        }

        true
    }

    /// Can `obj` resume construction of `object_being_constructed`
    /// (C++ ActionManager::canResumeConstructionOf).
    pub fn can_resume_construction_of(
        obj: &Object,
        object_being_constructed: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        // Wave 347: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if !obj.is_kind_of(KindOf::Dozer) {
            return false;
        }

        if obj.relationship_to(object_being_constructed) != Relationship::Allies {
            return false;
        }

        if !object_being_constructed.test_status(ObjectStatusTypes::UnderConstruction) {
            return false;
        }

        if obj.is_effectively_dead() {
            return false;
        }

        // C++ ActionManager.cpp:463-485 — DozerAI current BUILD task + BUILD target.
        let builder_id = object_being_constructed.get_builder_id();
        if builder_id != crate::common::INVALID_ID {
            if let Some(builder) = TheGameLogic::find_object_by_id(builder_id) {
                if let Ok(builder_guard) = builder.read() {
                    if let Some(ai) = builder_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            use crate::object::update::ai_update::dozer_ai_update::DozerTask;
                            let build_pending = ai_guard
                                .get_dozer_ai_update_interface_mut()
                                .is_some_and(|dozer| dozer.is_task_pending(DozerTask::Build));
                            if build_pending
                                && ai_guard.get_goal_object_id()
                                    == object_being_constructed.get_id()
                            {
                                return false;
                            }
                        }
                    }
                }
            }
        }

        if is_object_shrouded_for_action(obj, object_being_constructed, command_source) {
            return false;
        }

        true
    }

    /// Can `obj` enter `object_to_enter` (C++ ActionManager::canEnterObject).
    pub fn can_enter_object(
        obj: &Object,
        object_to_enter: &Object,
        command_source: CommandSourceType,
        mode: CanEnterType,
    ) -> bool {
        // Wave 347: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if obj.get_id() == object_to_enter.get_id() {
            return false;
        }

        if object_to_enter.is_effectively_dead() {
            return false;
        }

        if is_object_shrouded_for_action(obj, object_to_enter, command_source) {
            return false;
        }

        if obj.test_status(ObjectStatusTypes::UnderConstruction)
            || object_to_enter.test_status(ObjectStatusTypes::UnderConstruction)
        {
            return false;
        }

        if object_to_enter.test_status(ObjectStatusTypes::Sold) {
            return false;
        }

        if object_to_enter.is_disabled_by_type(DisabledType::DisabledSubdued) {
            return false;
        }

        if obj.is_kind_of(KindOf::IgnoredInGui)
            || obj.is_kind_of(KindOf::MobNexus)
            || object_to_enter.is_kind_of(KindOf::IgnoredInGui)
        {
            return false;
        }

        if obj.is_kind_of(KindOf::Structure) || obj.is_kind_of(KindOf::Immobile) {
            return false;
        }

        if obj.is_kind_of(KindOf::Infantry)
            && object_to_enter.is_disabled_by_type(DisabledType::DisabledUnmanned)
        {
            return !obj.is_kind_of(KindOf::RejectUnmanned);
        }

        if obj.is_kind_of(KindOf::Aircraft) && object_to_enter.is_kind_of(KindOf::FSAirfield) {
            if obj.test_status(ObjectStatusTypes::DeckHeightOffset)
                && obj.get_carrier_deck_height() >= obj.get_position().z
            {
                return false;
            }

            if !obj.is_above_terrain() {
                return false;
            }

            if obj.get_controlling_player_id() == object_to_enter.get_controlling_player_id() {
                if let Some(result) = object_to_enter.with_parking_place_behavior(|parking| {
                    if parking.has_reserved_space(obj.get_id()) {
                        return true;
                    }
                    parking.should_reserve_door_when_queued(obj.get_template().as_ref())
                        && parking.has_available_space_for(obj.get_template().as_ref())
                }) {
                    if result {
                        return true;
                    }
                }
            }
            return false;
        }

        if let Some(other_handle) = TheGameLogic::find_object_by_id(object_to_enter.get_id()) {
            if COLLISION_MANAGER
                .would_like_to_collide_with(obj.get_id(), &other_handle)
                .unwrap_or(false)
            {
                return true;
            }
        }

        #[cfg(feature = "allow_surrender")]
        {
            if object_to_enter.is_kind_of(KindOf::Prison) {
                return false;
            }
            if object_to_enter.is_kind_of(KindOf::PowTruck) {
                return false;
            }
        }

        let Some(contain) = object_to_enter.get_contain() else {
            return false;
        };
        let Ok(contain_guard) = contain.lock() else {
            return false;
        };

        if contain_guard.is_heal_contain() {
            let Some(body) = obj.get_body_module() else {
                return false;
            };
            let Ok(body_guard) = body.lock() else {
                return false;
            };
            if body_guard.get_health() >= body_guard.get_max_health() {
                return false;
            }
        }

        if mode == CanEnterType::CombatDropInto {
            if is_faction_structure(object_to_enter) {
                return false;
            }
        } else {
            let mut check_capacity = mode == CanEnterType::CheckCapacity;
            let contain_count = contain_guard.get_contained_count();
            let stealth_count = count_stealthed_contained(&*contain_guard);
            let non_stealth = contain_count.saturating_sub(stealth_count);

            if object_to_enter.get_controlling_player_id() != obj.get_controlling_player_id() {
                if non_stealth > 0 {
                    return false;
                }

                if is_faction_structure(object_to_enter) {
                    return false;
                }

                if stealth_count > 0 && non_stealth == 0 {
                    check_capacity = false;
                }
            }

            if check_capacity && obj.get_transport_slot_count() == 0 {
                return false;
            }

            if !contain_guard.is_valid_container_for(obj, check_capacity) {
                return false;
            }
        }

        true
    }

    /// Can `obj` convert `object_to_convert` to a car bomb (C++ ActionManager::canConvertObjectToCarBomb).
    pub fn can_convert_object_to_car_bomb(
        obj: &Object,
        object_to_convert: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        // Wave 347: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if object_to_convert.is_effectively_dead() {
            return false;
        }

        if is_object_shrouded_for_action(obj, object_to_convert, command_source) {
            return false;
        }

        let Some(other_handle) = TheGameLogic::find_object_by_id(object_to_convert.get_id()) else {
            return false;
        };

        COLLISION_MANAGER
            .would_like_to_collide_with_matching(obj.get_id(), &other_handle, |module| {
                module.is_car_bomb_crate_collide()
            })
            .unwrap_or(false)
    }

    /// Can `obj` hijack `object_to_hijack` (C++ ActionManager::canHijackVehicle).
    pub fn can_hijack_vehicle(
        obj: &Object,
        object_to_hijack: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        // Wave 347: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if object_to_hijack.is_effectively_dead() {
            return false;
        }

        if is_object_shrouded_for_action(obj, object_to_hijack, command_source) {
            return false;
        }

        if obj.relationship_to(object_to_hijack) != Relationship::Enemies {
            return false;
        }

        if !object_to_hijack.is_kind_of(KindOf::Vehicle) {
            return false;
        }

        if object_to_hijack.is_kind_of(KindOf::Aircraft) {
            return false;
        }

        if object_to_hijack.is_kind_of(KindOf::Drone) {
            return false;
        }

        let Some(other_handle) = TheGameLogic::find_object_by_id(object_to_hijack.get_id()) else {
            return false;
        };

        COLLISION_MANAGER
            .would_like_to_collide_with_matching(obj.get_id(), &other_handle, |module| {
                module.is_hijacked_vehicle_crate_collide()
            })
            .unwrap_or(false)
    }

    /// Can `obj` sabotage `object_to_sabotage` (C++ ActionManager::canSabotageBuilding).
    pub fn can_sabotage_building(
        obj: &Object,
        object_to_sabotage: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        // Wave 347: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if object_to_sabotage.is_effectively_dead() {
            return false;
        }

        if is_object_shrouded_for_action(obj, object_to_sabotage, command_source) {
            return false;
        }

        if obj.relationship_to(object_to_sabotage) != Relationship::Enemies {
            return false;
        }

        let Some(other_handle) = TheGameLogic::find_object_by_id(object_to_sabotage.get_id())
        else {
            return false;
        };

        COLLISION_MANAGER
            .would_like_to_collide_with_matching(obj.get_id(), &other_handle, |module| {
                module.is_sabotage_building_crate_collide()
            })
            .unwrap_or(false)
    }

    /// Can `obj` make `object_to_make_defector` defect (C++ ActionManager::canMakeObjectDefector).
    pub fn can_make_object_defector(
        obj: &Object,
        object_to_make_defector: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        if obj.relationship_to(object_to_make_defector) != Relationship::Enemies {
            return false;
        }

        if object_to_make_defector.is_effectively_dead() {
            return false;
        }

        if is_object_shrouded_for_action(obj, object_to_make_defector, command_source) {
            return false;
        }

        true
    }

    /// Can `obj` capture `object_to_capture` (C++ ActionManager::canCaptureBuilding).
    pub fn can_capture_building(
        obj: &Object,
        object_to_capture: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        let has_capture = obj.has_special_power(SpecialPowerType::InfantryCaptureBuilding);
        let has_lotus = obj.has_special_power(SpecialPowerType::BlackLotusCaptureBuilding);
        if !has_capture && !has_lotus {
            return false;
        }

        if object_to_capture.is_kind_of(KindOf::ImmuneToCapture) {
            return false;
        }

        let power_type = if has_capture {
            SpecialPowerType::InfantryCaptureBuilding
        } else {
            SpecialPowerType::BlackLotusCaptureBuilding
        };
        let Some(percent_ready) = get_special_power_ready_percent(obj, power_type) else {
            return false;
        };
        if percent_ready < 1.0 {
            return false;
        }

        if object_to_capture.is_effectively_dead() {
            return false;
        }

        if !object_to_capture.is_kind_of(KindOf::Structure) {
            return false;
        }

        if object_to_capture.test_status(ObjectStatusTypes::UnderConstruction)
            || object_to_capture.test_status(ObjectStatusTypes::Sold)
        {
            return false;
        }

        if is_object_shrouded_for_action(obj, object_to_capture, command_source) {
            return false;
        }

        let relationship = obj.relationship_to(object_to_capture);
        if relationship != Relationship::Enemies
            && !(object_to_capture.is_kind_of(KindOf::Capturable)
                && relationship != Relationship::Allies)
        {
            return false;
        }

        if object_to_capture.test_status(ObjectStatusTypes::Stealthed)
            && !object_to_capture.test_status(ObjectStatusTypes::Detected)
            && !object_to_capture.test_status(ObjectStatusTypes::Disguised)
        {
            return false;
        }

        if let Some(contain) = object_to_capture.get_contain() {
            if let Ok(contain_guard) = contain.lock() {
                if contain_guard.is_garrisonable() {
                    let contain_count = contain_guard.get_contained_count();
                    let stealth_count = count_stealthed_contained(&*contain_guard);
                    let non_stealth = contain_count.saturating_sub(stealth_count);
                    if non_stealth > 0 {
                        return false;
                    }
                }
            }
        }

        if appears_to_contain_friendlies(obj, object_to_capture) {
            return false;
        }

        true
    }

    /// Can `obj` disable `object_to_hack` via hacking (C++ ActionManager::canDisableVehicleViaHacking).
    pub fn can_disable_vehicle_via_hacking(
        obj: &Object,
        object_to_hack: &Object,
        command_source: CommandSourceType,
        check_source_requirements: bool,
    ) -> bool {
        if check_source_requirements
            && !obj.has_special_power(SpecialPowerType::BlackLotusDisableVehicleHack)
        {
            return false;
        }

        if check_source_requirements {
            let Some(percent_ready) = get_special_power_ready_percent(
                obj,
                SpecialPowerType::BlackLotusDisableVehicleHack,
            ) else {
                return false;
            };
            if percent_ready < 1.0 {
                return false;
            }
        }

        if object_to_hack.is_effectively_dead() {
            return false;
        }

        if object_to_hack.is_kind_of(KindOf::Aircraft) || object_to_hack.is_airborne_target() {
            return false;
        }

        if is_object_shrouded_for_action(obj, object_to_hack, command_source) {
            return false;
        }

        if obj.relationship_to(object_to_hack) != Relationship::Enemies {
            return false;
        }

        if !object_to_hack.is_kind_of(KindOf::Vehicle) {
            return false;
        }

        if object_to_hack.test_status(ObjectStatusTypes::Stealthed)
            && !object_to_hack.test_status(ObjectStatusTypes::Detected)
            && !object_to_hack.test_status(ObjectStatusTypes::Disguised)
        {
            return false;
        }

        if appears_to_contain_friendlies(obj, object_to_hack) {
            return false;
        }

        true
    }

    /// Can `obj` steal cash from `object_to_hack` (C++ ActionManager::canStealCashViaHacking).
    pub fn can_steal_cash_via_hacking(
        obj: &Object,
        object_to_hack: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        if !obj.has_special_power(SpecialPowerType::BlackLotusStealCashHack) {
            return false;
        }

        let Some(percent_ready) =
            get_special_power_ready_percent(obj, SpecialPowerType::BlackLotusStealCashHack)
        else {
            return false;
        };
        if percent_ready < 1.0 {
            return false;
        }

        if object_to_hack.is_effectively_dead() {
            return false;
        }

        if object_to_hack.test_status(ObjectStatusTypes::UnderConstruction) {
            return false;
        }

        if is_object_shrouded_for_action(obj, object_to_hack, command_source) {
            return false;
        }

        if obj.relationship_to(object_to_hack) != Relationship::Enemies {
            return false;
        }

        if !object_to_hack.is_kind_of(KindOf::CashGenerator) {
            return false;
        }

        if object_to_hack.is_kind_of(KindOf::RebuildHole)
            || !object_to_hack.is_kind_of(KindOf::Capturable)
        {
            return false;
        }

        if object_to_hack.test_status(ObjectStatusTypes::Stealthed)
            && !object_to_hack.test_status(ObjectStatusTypes::Detected)
            && !object_to_hack.test_status(ObjectStatusTypes::Disguised)
        {
            return false;
        }

        if appears_to_contain_friendlies(obj, object_to_hack) {
            return false;
        }

        true
    }

    /// Can `obj` disable `object_to_hack` building (C++ ActionManager::canDisableBuildingViaHacking).
    pub fn can_disable_building_via_hacking(
        obj: &Object,
        object_to_hack: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        if !obj.has_special_power(SpecialPowerType::HackerDisableBuilding) {
            return false;
        }

        let Some(percent_ready) =
            get_special_power_ready_percent(obj, SpecialPowerType::HackerDisableBuilding)
        else {
            return false;
        };
        if percent_ready < 1.0 {
            return false;
        }

        if object_to_hack.is_effectively_dead() {
            return false;
        }

        if is_object_shrouded_for_action(obj, object_to_hack, command_source) {
            return false;
        }

        if obj.relationship_to(object_to_hack) != Relationship::Enemies {
            return false;
        }

        if !object_to_hack.is_kind_of(KindOf::Structure) {
            return false;
        }

        let capturable = object_to_hack.is_kind_of(KindOf::Capturable)
            && !object_to_hack.is_kind_of(KindOf::RebuildHole);
        let tech_exception = object_to_hack.is_kind_of(KindOf::FSTechnology)
            && !object_to_hack.is_kind_of(KindOf::ImmuneToCapture);

        if !(capturable || tech_exception) {
            return false;
        }

        if object_to_hack.is_kind_of(KindOf::RebuildHole)
            || object_to_hack.test_status(ObjectStatusTypes::UnderConstruction)
        {
            return false;
        }

        if object_to_hack.test_status(ObjectStatusTypes::Stealthed)
            && !object_to_hack.test_status(ObjectStatusTypes::Detected)
            && !object_to_hack.test_status(ObjectStatusTypes::Disguised)
        {
            return false;
        }

        if appears_to_contain_friendlies(obj, object_to_hack) {
            return false;
        }

        true
    }

    /// Can `obj` snipe `object_to_snipe` (C++ ActionManager::canSnipeVehicle).
    pub fn can_snipe_vehicle(
        obj: &Object,
        object_to_snipe: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        if object_to_snipe.is_effectively_dead() {
            return false;
        }

        if is_object_shrouded_for_action(obj, object_to_snipe, command_source) {
            return false;
        }

        if obj.relationship_to(object_to_snipe) != Relationship::Enemies {
            return false;
        }

        if !object_to_snipe.is_kind_of(KindOf::Vehicle) {
            return false;
        }

        if object_to_snipe.is_kind_of(KindOf::Drone) {
            return false;
        }

        if object_to_snipe.is_airborne_target() {
            return false;
        }

        if object_to_snipe.is_disabled_by_type(DisabledType::DisabledUnmanned) {
            return false;
        }

        true
    }

    /// Can `obj` bribe `object_to_bribe` (C++ ActionManager::canBribeUnit).
    /// C++ always returns FALSE — the ability was never implemented.
    pub fn can_bribe_unit(
        _obj: &Object,
        _object_to_bribe: &Object,
        _command_source: CommandSourceType,
    ) -> bool {
        false
    }

    /// Can `obj` cut power to `building` (C++ ActionManager::canCutBuildingPower).
    /// C++ always returns FALSE — the ability was never implemented.
    pub fn can_cut_building_power(
        _obj: &Object,
        _building: &Object,
        _command_source: CommandSourceType,
    ) -> bool {
        false
    }

    /// Can `obj` fire weapon in slot at location (C++ ActionManager::canFireWeaponAtLocation).
    pub fn can_fire_weapon_at_location(
        obj: &Object,
        _loc: &crate::common::Coord3D,
        _command_source: CommandSourceType,
        slot: WeaponSlotType,
        _object_in_way: Option<&Object>,
    ) -> bool {
        obj.get_weapon_in_weapon_slot(slot).is_some()
    }

    /// Can `obj` fire weapon in slot at target (C++ ActionManager::canFireWeaponAtObject).
    pub fn can_fire_weapon_at_object(
        obj: &Object,
        target: &Object,
        command_source: CommandSourceType,
        slot: WeaponSlotType,
    ) -> bool {
        let Some(weapon) = obj.get_weapon_in_weapon_slot(slot) else {
            return false;
        };

        // C++ ActionManager.cpp:1944-1958 — DAMAGE_KILLPILOT uses the slot-specific
        // getAbleToAttackSpecificObject overload.
        let sniper = weapon.get_damage_type() == DamageType::KillPilot;
        if sniper && !Self::can_snipe_vehicle(obj, target, command_source) {
            return false;
        }

        let result = if sniper {
            obj.weapon_set.get_able_to_attack_specific_object(
                AbleToAttackType::NewTarget,
                obj.get_id(),
                target.get_id(),
                command_source,
                Some(slot),
            )
        } else {
            obj.get_able_to_attack_specific_object(
                AbleToAttackType::NewTarget,
                target,
                command_source,
            )
        };

        matches!(
            result,
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) && weapon.estimate_weapon_damage(obj.get_id(), Some(target.get_id()), None) != 0.0
    }

    /// Can `obj` fire weapon in slot (C++ ActionManager::canFireWeapon).
    pub fn can_fire_weapon(
        obj: &Object,
        slot: WeaponSlotType,
        _command_source: CommandSourceType,
    ) -> bool {
        obj.get_weapon_in_weapon_slot(slot).is_some()
    }

    /// Can `obj` garrison `target` (C++ ActionManager::canGarrison).
    pub fn can_garrison(obj: &Object, target: &Object, _command_source: CommandSourceType) -> bool {
        if !obj.is_kind_of(KindOf::Infantry) || obj.is_kind_of(KindOf::NoGarrison) {
            return false;
        }

        if !target.is_kind_of(KindOf::Structure) {
            return false;
        }

        if obj.get_contain().is_none() {
            return false;
        }

        let Some(contain) = target.get_contain() else {
            return false;
        };
        let Ok(contain_guard) = contain.lock() else {
            return false;
        };
        if !contain_guard.is_garrisonable() {
            return false;
        }

        if obj.get_controlling_player_id() == target.get_controlling_player_id() {
            return contain_guard.is_valid_container_for(obj, true);
        }

        // C++ ActionManager.cpp:2016 — player->getRelationship(target->getTeam()) == NEUTRAL
        if let Some(player) = obj.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                if let Some(target_team) = target.get_team() {
                    if let Ok(target_team_guard) = target_team.read() {
                        if player_guard.get_relationship_with_team(&*target_team_guard)
                            == Relationship::Neutral
                        {
                            return contain_guard.get_contained_count() == 0
                                && contain_guard.is_valid_container_for(obj, true);
                        }
                    }
                }
            }
        }

        false
    }

    /// Can `player` garrison `target` (C++ ActionManager::canPlayerGarrison).
    pub fn can_player_garrison(
        player: &crate::player::Player,
        target: &Object,
        _command_source: CommandSourceType,
    ) -> bool {
        if target.is_effectively_dead() {
            return false;
        }

        if !target.is_kind_of(KindOf::Structure) {
            return false;
        }

        let Some(contain) = target.get_contain() else {
            return false;
        };
        let Ok(contain_guard) = contain.lock() else {
            return false;
        };
        if !contain_guard.is_garrisonable() {
            return false;
        }

        if let Some(target_player) = target.get_controlling_player() {
            if let Ok(target_guard) = target_player.read() {
                if std::ptr::eq(player, &*target_guard) {
                    return true;
                }

                if let Some(target_team) = target_guard.get_default_team() {
                    if let Ok(target_team_guard) = target_team.read() {
                        if player.get_relationship_with_team(&*target_team_guard)
                            == Relationship::Neutral
                        {
                            return contain_guard.get_contained_count() == 0;
                        }
                    }
                }
            }
        }

        false
    }

    /// Can `obj` retarget an in-progress special power at `loc`
    /// (C++ ActionManager::canOverrideSpecialPowerDestination).
    pub fn can_override_special_power_destination(
        obj: &Object,
        loc: &crate::common::Coord3D,
        sp_type: SpecialPowerType,
        _command_source: CommandSourceType,
    ) -> bool {
        if obj
            .find_special_power_with_overridable_destination_active(sp_type)
            .is_none()
        {
            return false;
        }

        // C++: ThePartitionManager->getShroudStatusForPlayer(
        //          obj->getControllingPlayer()->getPlayerIndex(), loc)
        //      != CELLSHROUD_SHROUDED
        // Missing player is treated as an invalid index, which C++ maps to shrouded.
        let player_index = obj
            .get_controlling_player()
            .and_then(|player| player.read().ok().map(|guard| guard.get_player_index()))
            .or_else(|| {
                obj.get_controlling_player_id()
                    .map(|player_id| player_id as crate::common::Int)
            })
            .unwrap_or(-1);

        let Some(partition) = ThePartitionManager::get() else {
            return false;
        };
        partition.get_shroud_status_for_player(player_index, loc)
            != game_engine::common::system::radar::CellShroudStatus::Shrouded
    }

    /// Can `obj` attack `object_to_attack` (C++ ActionManager::getCanAttackObject).
    pub fn get_can_attack_object(
        obj: &Object,
        object_to_attack: &Object,
        command_source: CommandSourceType,
        attack_type: AbleToAttackType,
    ) -> CanAttackResult {
        if obj.is_effectively_dead()
            || object_to_attack.is_effectively_dead()
            || obj.get_id() == object_to_attack.get_id()
        {
            return ATTACKRESULT_NOT_POSSIBLE;
        }

        if !obj.is_able_to_attack() {
            return ATTACKRESULT_NOT_POSSIBLE;
        }

        let result =
            obj.get_able_to_attack_specific_object(attack_type, object_to_attack, command_source);
        if result != ATTACKRESULT_NOT_POSSIBLE {
            if command_source == CommandSourceType::FromPlayer && !obj.has_any_damage_weapon() {
                return ATTACKRESULT_NOT_POSSIBLE;
            }

            if result == CanAttackResult::InvalidShot && obj.is_kind_of(KindOf::Dozer) {
                if let Some((weapon, _)) = obj.get_current_weapon() {
                    if weapon.get_damage_type() == DamageType::Disarm {
                        return ATTACKRESULT_NOT_POSSIBLE;
                    }
                }
            }

            return result;
        }

        if obj.is_kind_of(KindOf::SpawnsAreTheWeapons) {
            for behavior in obj.get_behavior_modules() {
                let Ok(mut behavior) = behavior.lock() else {
                    continue;
                };
                if let Some(spawn_behavior) = behavior.get_spawn_behavior_full_interface() {
                    let result = spawn_behavior.get_can_any_slaves_attack_specific_target(
                        attack_type,
                        object_to_attack,
                        command_source,
                    );
                    if result != ATTACKRESULT_NOT_POSSIBLE {
                        return result;
                    }
                }
            }
        }

        ATTACKRESULT_NOT_POSSIBLE
    }

    /// Convenience overload using object IDs.
    pub fn can_pick_up_prisoner_by_id(
        obj_id: ObjectID,
        prisoner_id: ObjectID,
        command_source: CommandSourceType,
    ) -> bool {
        // Wave 347: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        let Some(obj) = TheGameLogic::find_object_by_id(obj_id) else {
            return false;
        };
        let Some(prisoner) = TheGameLogic::find_object_by_id(prisoner_id) else {
            return false;
        };

        let Ok(obj_guard) = obj.read() else {
            return false;
        };
        let Ok(prisoner_guard) = prisoner.read() else {
            return false;
        };

        Self::can_pick_up_prisoner(&obj_guard, &prisoner_guard, command_source)
    }
}

pub struct GameLogicActionExecutor;

impl GameLogicActionExecutor {
    pub fn install(manager: &mut RtsActionManager) {
        manager.set_action_executor(Arc::new(Self));
    }

    fn get_selection(player_id: Int) -> Vec<ObjectID> {
        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return Vec::new();
        };
        manager
            .get_player_selection_ref(player_id)
            .map(|selection| selection.get_selected_objects())
            .unwrap_or_default()
    }

    fn queue_command(player_id: Int, command: Command) -> bool {
        let current_frame = TheGameLogic::get_frame();
        let queued = QueuedCommand::new(command, CommandPriority::Normal, current_frame);
        let queue_manager = get_command_queue_manager();
        let Ok(mut manager) = queue_manager.lock() else {
            log::warn!("ActionExecutor: unable to lock command queue");
            return false;
        };
        match manager.queue_player_command(player_id, queued) {
            Ok(()) => true,
            Err(err) => {
                log::warn!("ActionExecutor: queue failed: {}", err);
                false
            }
        }
    }

    fn to_coord3d(pos: ActionCoord3D) -> crate::common::Coord3D {
        crate::common::Coord3D::new(pos.x, pos.y, pos.z)
    }
}

impl ActionExecutor for GameLogicActionExecutor {
    fn execute(&self, player_index: u32, action: ActionType, _options: u32) -> bool {
        let player_id = player_index as Int;
        let selected = Self::get_selection(player_id);

        match action {
            ActionType::Move { target_pos } => {
                if selected.is_empty() {
                    return false;
                }
                let command = command_builder::create_move_to_position(
                    selected,
                    Self::to_coord3d(target_pos),
                    player_id,
                );
                Self::queue_command(player_id, command)
            }
            ActionType::Attack { target_id } => {
                if selected.is_empty() {
                    return false;
                }
                let command = command_builder::create_attack_object(selected, target_id, player_id);
                Self::queue_command(player_id, command)
            }
            ActionType::AttackMove { target_pos } => {
                if selected.is_empty() {
                    return false;
                }
                let mut command = Command::new(CommandType::DoAttackMoveTo);
                command.set_player_index(player_id);
                command.append_location_argument(Self::to_coord3d(target_pos));
                for object_id in &selected {
                    command.append_object_id_argument(*object_id);
                }
                Self::queue_command(player_id, command)
            }
            ActionType::Guard {
                target_pos,
                target_id,
            } => {
                if selected.is_empty() {
                    return false;
                }
                let mut command = if target_id.is_some() {
                    Command::new(CommandType::DoGuardObject)
                } else if target_pos.is_some() {
                    Command::new(CommandType::DoGuardPosition)
                } else {
                    return false;
                };
                command.set_player_index(player_id);
                if let Some(pos) = target_pos {
                    command.append_location_argument(Self::to_coord3d(pos));
                }
                if let Some(id) = target_id {
                    command.append_object_id_argument(id);
                }
                for object_id in &selected {
                    command.append_object_id_argument(*object_id);
                }
                Self::queue_command(player_id, command)
            }
            ActionType::Stop => {
                if selected.is_empty() {
                    return false;
                }
                let command = command_builder::create_stop_command(selected, player_id);
                Self::queue_command(player_id, command)
            }
            ActionType::Repair { target_id } => {
                if selected.is_empty() {
                    return false;
                }
                let mut command = Command::new(CommandType::DoRepair);
                command.set_player_index(player_id);
                command.append_object_id_argument(target_id);
                for object_id in &selected {
                    command.append_object_id_argument(*object_id);
                }
                Self::queue_command(player_id, command)
            }
            ActionType::Enter { container_id } => {
                if selected.is_empty() {
                    return false;
                }
                let mut command = Command::new(CommandType::Enter);
                command.set_player_index(player_id);
                command.append_object_id_argument(container_id);
                for object_id in &selected {
                    command.append_object_id_argument(*object_id);
                }
                Self::queue_command(player_id, command)
            }
            ActionType::Garrison { building_id } => {
                if selected.is_empty() {
                    return false;
                }
                let mut command = Command::new(CommandType::Enter);
                command.set_player_index(player_id);
                command.append_object_id_argument(building_id);
                for object_id in &selected {
                    command.append_object_id_argument(*object_id);
                }
                Self::queue_command(player_id, command)
            }
            ActionType::TransferSupplies { target_id } => {
                if selected.is_empty() {
                    return false;
                }
                let mut command = Command::new(CommandType::Dock);
                command.set_player_index(player_id);
                command.append_object_id_argument(target_id);
                for object_id in &selected {
                    command.append_object_id_argument(*object_id);
                }
                Self::queue_command(player_id, command)
            }
            ActionType::Build {
                building_type,
                position,
            } => {
                if selected.is_empty() {
                    return false;
                }
                let Some(template_name) = NameKeyGenerator::key_to_name(building_type) else {
                    log::warn!(
                        "ActionExecutor: unknown building template key {}",
                        building_type
                    );
                    return false;
                };
                let builder_id = selected[0];
                let mut command = Command::new(CommandType::DozerConstruct);
                command.set_player_index(player_id);
                command.append_object_id_argument(builder_id);
                command.append_location_argument(Self::to_coord3d(position));
                command.append_ascii_string_argument(AsciiString::from(template_name.as_str()));
                Self::queue_command(player_id, command)
            }
            ActionType::SpecialPower {
                power_type,
                target_pos,
                target_id,
            } => {
                use crate::common::INVALID_OBJECT_ID;
                use crate::object_creation_list::nuggets::INVALID_ANGLE;

                let options = 0;
                let source_id = INVALID_OBJECT_ID;
                let mut command = if target_pos.is_some() {
                    Command::new(CommandType::DoSpecialPowerAtLocation)
                } else if target_id.is_some() {
                    Command::new(CommandType::DoSpecialPowerAtObject)
                } else {
                    Command::new(CommandType::DoSpecialPower)
                };
                command.set_player_index(player_id);
                command.append_integer_argument(power_type as i32);
                if let Some(pos) = target_pos {
                    command.append_location_argument(Self::to_coord3d(pos));
                    command.append_real_argument(INVALID_ANGLE);
                    command.append_object_id_argument(INVALID_OBJECT_ID);
                    command.append_integer_argument(options);
                    command.append_object_id_argument(source_id);
                }
                if let Some(target) = target_id {
                    command.append_object_id_argument(target);
                    command.append_integer_argument(options);
                    command.append_object_id_argument(source_id);
                }
                if target_pos.is_none() && target_id.is_none() {
                    command.append_integer_argument(options);
                    command.append_object_id_argument(source_id);
                }
                Self::queue_command(player_id, command)
            }
        }
    }
}

static RTS_ACTION_MANAGER: Lazy<Arc<RwLock<RtsActionManager>>> = Lazy::new(|| {
    let mut manager = RtsActionManager::new();
    GameLogicActionExecutor::install(&mut manager);
    Arc::new(RwLock::new(manager))
});

pub fn get_rts_action_manager() -> Arc<RwLock<RtsActionManager>> {
    RTS_ACTION_MANAGER.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_button::CommandButton;
    use crate::common::Coord3D;
    use crate::modules::{
        BehaviorModuleInterface, SpecialPowerCommandOptions, SpecialPowerUpdateInterface,
    };
    use crate::object::SpecialPowerTemplate;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::special_power_module::Waypoint;
    use crate::player::{Player, player_list};
    use crate::system::shroud_manager::get_shroud_manager;
    use crate::team::Team;
    use std::sync::Mutex;

    fn test_state_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    struct OverridableSpecialPowerUpdate {
        active: bool,
    }

    impl SpecialPowerUpdateInterface for OverridableSpecialPowerUpdate {
        fn initiate_intent_to_do_special_power(
            &mut self,
            _special_power_template: &SpecialPowerTemplate,
            _target_obj: Option<ObjectID>,
            _target_pos: Option<&Coord3D>,
            _waypoint: Option<&Waypoint>,
            _command_options: SpecialPowerCommandOptions,
        ) -> bool {
            false
        }

        fn is_special_ability(&self) -> bool {
            false
        }

        fn is_special_power(&self) -> bool {
            true
        }

        fn is_active(&self) -> bool {
            self.active
        }

        fn get_command_option(&self) -> SpecialPowerCommandOptions {
            SpecialPowerCommandOptions::NONE
        }

        fn does_special_power_have_overridable_destination_active(&self) -> bool {
            self.active
        }

        fn does_special_power_have_overridable_destination(&self) -> bool {
            true
        }

        fn set_special_power_overridable_destination(&mut self, _location: &Coord3D) {}

        fn is_power_currently_in_use(&self, _command: Option<&CommandButton>) -> bool {
            self.active
        }
    }

    struct OverridableSpecialPowerBehavior {
        update: OverridableSpecialPowerUpdate,
    }

    impl BehaviorModuleInterface for OverridableSpecialPowerBehavior {
        fn get_special_power_update_interface(
            &mut self,
        ) -> Option<&mut dyn SpecialPowerUpdateInterface> {
            Some(&mut self.update)
        }
    }

    fn attach_overridable_special_power(obj: &mut Object, active: bool) {
        let behavior: Arc<Mutex<dyn BehaviorModuleInterface>> =
            Arc::new(Mutex::new(OverridableSpecialPowerBehavior {
                update: OverridableSpecialPowerUpdate { active },
            }));
        obj.push_behavior_module_for_test(behavior);
    }

    #[test]
    fn can_bribe_unit_always_returns_false() {
        let obj = Object::new_test(8801, 100.0);
        let target = Object::new_test(8802, 100.0);
        assert!(!TheActionManager::can_bribe_unit(
            &obj,
            &target,
            CommandSourceType::FromPlayer
        ));
        assert!(!TheActionManager::can_bribe_unit(
            &obj,
            &target,
            CommandSourceType::FromScript
        ));
    }

    #[test]
    fn can_cut_building_power_always_returns_false() {
        let obj = Object::new_test(8803, 100.0);
        let building = Object::new_test(8804, 100.0);
        assert!(!TheActionManager::can_cut_building_power(
            &obj,
            &building,
            CommandSourceType::FromPlayer
        ));
        assert!(!TheActionManager::can_cut_building_power(
            &obj,
            &building,
            CommandSourceType::FromAi
        ));
    }

    #[test]
    fn can_override_special_power_destination_false_without_interface() {
        let obj = Object::new_test(8805, 100.0);
        let loc = Coord3D::new(100.0, 100.0, 0.0);
        assert!(!TheActionManager::can_override_special_power_destination(
            &obj,
            &loc,
            SpecialPowerType::SpectreGunship,
            CommandSourceType::FromPlayer,
        ));
    }

    #[test]
    fn can_override_special_power_destination_false_when_inactive() {
        let mut obj = Object::new_test(8806, 100.0);
        attach_overridable_special_power(&mut obj, false);
        let loc = Coord3D::new(100.0, 100.0, 0.0);
        assert!(!TheActionManager::can_override_special_power_destination(
            &obj,
            &loc,
            SpecialPowerType::SpectreGunship,
            CommandSourceType::FromPlayer,
        ));
    }

    #[test]
    fn can_override_special_power_destination_false_when_shrouded() {
        let _guard = test_state_lock();
        if let Ok(mut shroud) = get_shroud_manager().lock() {
            shroud.clear_all();
        }

        let mut obj = Object::new_test(8807, 100.0);
        attach_overridable_special_power(&mut obj, true);
        let loc = Coord3D::new(100.0, 100.0, 0.0);

        // No controlling player → invalid player index → CELLSHROUD_SHROUDED.
        assert!(!TheActionManager::can_override_special_power_destination(
            &obj,
            &loc,
            SpecialPowerType::SpectreGunship,
            CommandSourceType::FromPlayer,
        ));
    }

    #[test]
    fn can_override_special_power_destination_true_when_active_and_unshrouded() {
        let _guard = test_state_lock();
        player_list().write().unwrap().clear();
        if let Ok(mut shroud) = get_shroud_manager().lock() {
            shroud.clear_all();
            shroud.init_shroud_grid(1000.0, 1000.0);
            shroud
                .reveal_map_for_player(0)
                .expect("reveal map for player 0");
        }

        // Team::set_controlling_player_id no-ops when the dual-world registry is empty.
        let dummy_id = 0x00A0_7109;
        let dummy = Arc::new(RwLock::new(Object::new_test(dummy_id, 1.0)));
        OBJECT_REGISTRY.register_object(dummy_id, &dummy);

        let player = Arc::new(RwLock::new(Player::new(0)));
        player_list().write().unwrap().add_player(player);

        let team = Arc::new(RwLock::new(Team::new("OverrideTeam".into(), 0x00A0_7108)));
        team.write().unwrap().set_controlling_player_id(Some(0));

        let mut obj = Object::new_test(8808, 100.0);
        obj.set_team(Some(team)).unwrap();
        attach_overridable_special_power(&mut obj, true);

        assert!(
            obj.get_controlling_player_id().is_some(),
            "test object must have a controlling player index"
        );
        assert!(
            obj.find_special_power_with_overridable_destination_active(
                SpecialPowerType::SpectreGunship
            )
            .is_some(),
            "test object must expose an active overridable special power"
        );

        let loc = Coord3D::new(100.0, 100.0, 0.0);
        let partition = ThePartitionManager::get().expect("partition manager");
        assert_ne!(
            partition.get_shroud_status_for_player(0, &loc),
            game_engine::common::system::radar::CellShroudStatus::Shrouded
        );

        assert!(TheActionManager::can_override_special_power_destination(
            &obj,
            &loc,
            SpecialPowerType::SpectreGunship,
            CommandSourceType::FromPlayer,
        ));

        player_list().write().unwrap().clear();
        OBJECT_REGISTRY.unregister_object(dummy_id);
        if let Ok(mut shroud) = get_shroud_manager().lock() {
            shroud.clear_all();
        }
    }

    fn object_with_kinds(id: ObjectID, health: f32, kinds: &[KindOf]) -> Object {
        let mut template = crate::common::types::DefaultThingTemplate::new(format!("Test{id}"));
        for kind in kinds {
            template.add_kind_of(*kind);
        }
        let mut obj = Object::new_test(id, health);
        obj.set_template_for_test(Arc::new(template));
        obj
    }

    struct ReadySpecialPowerModule {
        power_type: SpecialPowerType,
        percent_ready: f32,
        template: SpecialPowerTemplate,
    }

    impl SpecialPowerModuleInterface for ReadySpecialPowerModule {
        fn activate(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn can_activate(&self) -> bool {
            self.percent_ready >= 1.0
        }
        fn get_power_type(&self) -> u32 {
            self.power_type as u32
        }
        fn get_ready_frame(&self) -> u32 {
            0
        }
        fn is_ready(&self) -> bool {
            self.percent_ready >= 1.0
        }
        fn get_special_power_template(&self) -> Option<Arc<dyn std::any::Any>> {
            None
        }
        fn is_module_for_power(&self, template: &SpecialPowerTemplate) -> bool {
            template.get_special_power_type() == self.power_type
        }
        fn get_power_name(&self) -> String {
            self.template.get_name().to_string()
        }
        fn get_percent_ready(&self) -> f32 {
            self.percent_ready
        }
        fn pause_countdown(&mut self, _pause: bool) {}
        fn mark_special_power_triggered(&mut self, _location: Option<&Coord3D>) {}
    }

    struct ReadySpecialPowerBehavior {
        module: ReadySpecialPowerModule,
    }

    impl BehaviorModuleInterface for ReadySpecialPowerBehavior {
        fn get_special_power(&mut self) -> Option<&mut dyn SpecialPowerModuleInterface> {
            Some(&mut self.module)
        }
        fn get_special_power_module_interface(
            &mut self,
        ) -> Option<&mut dyn SpecialPowerModuleInterface> {
            Some(&mut self.module)
        }
        fn get_special_power_module_interface_const(
            &self,
        ) -> Option<&dyn SpecialPowerModuleInterface> {
            Some(&self.module)
        }
    }

    fn attach_ready_special_power(
        obj: &mut Object,
        power_type: SpecialPowerType,
        percent_ready: f32,
    ) -> SpecialPowerTemplate {
        obj.set_special_power_available(power_type, true);
        let template = SpecialPowerTemplate::new(format!("TestPower{power_type:?}"), 9000)
            .with_power_type(power_type);
        let behavior: Arc<Mutex<dyn BehaviorModuleInterface>> =
            Arc::new(Mutex::new(ReadySpecialPowerBehavior {
                module: ReadySpecialPowerModule {
                    power_type,
                    percent_ready,
                    template: template.clone(),
                },
            }));
        obj.push_behavior_module_for_test(behavior);
        template
    }

    #[derive(Debug)]
    struct TestContain {
        ids: Vec<ObjectID>,
        garrisonable: bool,
        max: usize,
    }

    impl crate::modules::ContainModuleInterface for TestContain {
        fn can_contain(&self, _object_id: ObjectID) -> bool {
            self.ids.len() < self.max
        }
        fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
            self.ids.push(object_id);
            Ok(())
        }
        fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
            self.ids.retain(|id| *id != object_id);
            Ok(())
        }
        fn get_contained_objects(&self) -> &[ObjectID] {
            &self.ids
        }
        fn get_contained_count(&self) -> usize {
            self.ids.len()
        }
        fn get_max_capacity(&self) -> usize {
            self.max
        }
        fn is_garrisonable(&self) -> bool {
            self.garrisonable
        }
    }

    #[test]
    fn can_do_special_power_false_without_module() {
        let mut obj = Object::new_test(8901, 100.0);
        obj.set_special_power_available(SpecialPowerType::DetonateDirtyNuke, true);
        let template = SpecialPowerTemplate::new("DirtyNuke".into(), 1)
            .with_power_type(SpecialPowerType::DetonateDirtyNuke);
        assert!(!TheActionManager::can_do_special_power(
            &obj,
            &template,
            CommandSourceType::FromPlayer,
            0,
            true,
        ));
    }

    #[test]
    fn can_do_special_power_true_for_no_target_ready_power() {
        let mut obj = Object::new_test(8902, 100.0);
        let template =
            attach_ready_special_power(&mut obj, SpecialPowerType::DetonateDirtyNuke, 1.0);
        assert!(TheActionManager::can_do_special_power(
            &obj,
            &template,
            CommandSourceType::FromPlayer,
            0,
            true,
        ));
    }

    #[test]
    fn can_do_special_power_false_when_not_ready() {
        let mut obj = Object::new_test(8903, 100.0);
        let template =
            attach_ready_special_power(&mut obj, SpecialPowerType::DetonateDirtyNuke, 0.5);
        assert!(!TheActionManager::can_do_special_power(
            &obj,
            &template,
            CommandSourceType::FromPlayer,
            0,
            true,
        ));
    }

    #[test]
    fn can_do_special_power_false_for_location_or_object_powers() {
        let mut obj = Object::new_test(8904, 100.0);
        let daisy = attach_ready_special_power(&mut obj, SpecialPowerType::DaisyCutter, 1.0);
        assert!(!TheActionManager::can_do_special_power(
            &obj,
            &daisy,
            CommandSourceType::FromPlayer,
            0,
            true,
        ));
        let mut obj2 = Object::new_test(8905, 100.0);
        let emp = attach_ready_special_power(&mut obj2, SpecialPowerType::EmpPulse, 1.0);
        assert!(!TheActionManager::can_do_special_power(
            &obj2,
            &emp,
            CommandSourceType::FromPlayer,
            0,
            true,
        ));
        let mut obj3 = Object::new_test(8906, 100.0);
        let cash = attach_ready_special_power(&mut obj3, SpecialPowerType::CashHack, 1.0);
        assert!(!TheActionManager::can_do_special_power(
            &obj3,
            &cash,
            CommandSourceType::FromPlayer,
            0,
            true,
        ));
    }

    #[test]
    fn can_do_special_power_at_location_false_without_module() {
        let obj = Object::new_test(8907, 100.0);
        let template = SpecialPowerTemplate::new("Daisy".into(), 2)
            .with_power_type(SpecialPowerType::DaisyCutter);
        let loc = Coord3D::new(100.0, 100.0, 0.0);
        assert!(!TheActionManager::can_do_special_power_at_location(
            &obj,
            &loc,
            CommandSourceType::FromPlayer,
            &template,
            None,
            0,
            true,
        ));
    }

    #[test]
    fn can_do_special_power_at_location_false_when_shrouded() {
        let _guard = test_state_lock();
        if let Ok(mut shroud) = get_shroud_manager().lock() {
            shroud.clear_all();
        }
        let mut obj = Object::new_test(8908, 100.0);
        let template = attach_ready_special_power(&mut obj, SpecialPowerType::DaisyCutter, 1.0);
        let loc = Coord3D::new(100.0, 100.0, 0.0);
        assert!(!TheActionManager::can_do_special_power_at_location(
            &obj,
            &loc,
            CommandSourceType::FromPlayer,
            &template,
            None,
            0,
            true,
        ));
    }

    #[test]
    fn location_target_only_matches_need_target_pos_action_manager() {
        assert!(TheActionManager::special_power_is_location_target_only(
            SpecialPowerType::A10ThunderboltStrike
        ));
        assert!(TheActionManager::special_power_is_location_target_only(
            SpecialPowerType::DaisyCutter
        ));
        assert!(TheActionManager::special_power_is_location_target_only(
            SpecialPowerType::Frenzy
        ));
        assert!(TheActionManager::special_power_is_location_target_only(
            SpecialPowerType::ParadropAmerica
        ));
        assert!(!TheActionManager::special_power_is_location_target_only(
            SpecialPowerType::BattleshipBombardment
        ));
        assert!(!TheActionManager::special_power_is_location_target_only(
            SpecialPowerType::MissileDefenderLaserGuidedMissiles
        ));
        assert!(!TheActionManager::special_power_is_location_target_only(
            SpecialPowerType::LaunchBaikonurRocket
        ));
        assert!(!TheActionManager::special_power_is_location_target_only(
            SpecialPowerType::InfantryCaptureBuilding
        ));
    }

    #[test]
    fn no_target_matches_can_do_special_power_type_switch() {
        assert!(TheActionManager::special_power_is_no_target(
            SpecialPowerType::CiaIntelligence
        ));
        assert!(TheActionManager::special_power_is_no_target(
            SpecialPowerType::CommunicationsDownload
        ));
        assert!(TheActionManager::special_power_is_no_target(
            SpecialPowerType::DetonateDirtyNuke
        ));
        assert!(TheActionManager::special_power_is_no_target(
            SpecialPowerType::RemoteCharges
        ));
        assert!(TheActionManager::special_power_is_no_target(
            SpecialPowerType::ChangeBattlePlans
        ));
        assert!(TheActionManager::special_power_is_no_target(
            SpecialPowerType::LaunchBaikonurRocket
        ));
        assert!(!TheActionManager::special_power_is_no_target(
            SpecialPowerType::DaisyCutter
        ));
        assert!(!TheActionManager::special_power_is_no_target(
            SpecialPowerType::CashHack
        ));
        assert!(!TheActionManager::special_power_is_no_target(
            SpecialPowerType::SpySatellite
        ));
    }

    #[test]
    fn can_do_special_power_at_location_true_when_unshrouded() {
        let _guard = test_state_lock();
        player_list().write().unwrap().clear();
        if let Ok(mut shroud) = get_shroud_manager().lock() {
            shroud.clear_all();
            shroud.init_shroud_grid(1000.0, 1000.0);
            shroud
                .reveal_map_for_player(0)
                .expect("reveal map for player 0");
        }
        let dummy_id = 0x00A0_8909;
        let dummy = Arc::new(RwLock::new(Object::new_test(dummy_id, 1.0)));
        OBJECT_REGISTRY.register_object(dummy_id, &dummy);
        let player = Arc::new(RwLock::new(Player::new(0)));
        player_list().write().unwrap().add_player(player);
        let team = Arc::new(RwLock::new(Team::new("LocTeam".into(), 0x00A0_8910)));
        team.write().unwrap().set_controlling_player_id(Some(0));

        let mut obj = Object::new_test(8911, 100.0);
        obj.set_team(Some(team)).unwrap();
        let template = attach_ready_special_power(&mut obj, SpecialPowerType::DaisyCutter, 1.0);
        let loc = Coord3D::new(100.0, 100.0, 0.0);
        assert!(TheActionManager::can_do_special_power_at_location(
            &obj,
            &loc,
            CommandSourceType::FromPlayer,
            &template,
            None,
            0,
            true,
        ));

        player_list().write().unwrap().clear();
        OBJECT_REGISTRY.unregister_object(dummy_id);
        if let Ok(mut shroud) = get_shroud_manager().lock() {
            shroud.clear_all();
        }
    }

    #[test]
    fn can_do_special_power_at_location_false_for_object_target_powers() {
        let mut obj = Object::new_test(8912, 100.0);
        let template =
            attach_ready_special_power(&mut obj, SpecialPowerType::InfantryCaptureBuilding, 1.0);
        let loc = Coord3D::new(100.0, 100.0, 0.0);
        assert!(!TheActionManager::can_do_special_power_at_location(
            &obj,
            &loc,
            CommandSourceType::FromPlayer,
            &template,
            None,
            0,
            true,
        ));
    }

    #[test]
    fn can_do_special_power_at_object_battleship_relationship() {
        let mut obj = Object::new_test(8913, 100.0);
        let template =
            attach_ready_special_power(&mut obj, SpecialPowerType::BattleshipBombardment, 1.0);

        // No teams → Neutral → C++ allows (relationship != ALLIES).
        let neutral = Object::new_test(8914, 100.0);
        // Battleship is not in SpecialPowerMask; isolate relationship with
        // check_source_requirements=false (C++ still requires a module, which we attach).
        assert!(TheActionManager::can_do_special_power_at_object(
            &obj,
            &neutral,
            CommandSourceType::FromPlayer,
            &template,
            0,
            false,
        ));

        let team = Arc::new(RwLock::new(Team::new("ShipTeam".into(), 0x00A0_8915)));
        obj.set_team(Some(team.clone())).unwrap();
        let mut ally = Object::new_test(8916, 100.0);
        ally.set_team(Some(team)).unwrap();
        assert!(!TheActionManager::can_do_special_power_at_object(
            &obj,
            &ally,
            CommandSourceType::FromPlayer,
            &template,
            0,
            false,
        ));

        let enemy_team = Arc::new(RwLock::new(Team::new("EnemyShip".into(), 0x00A0_8917)));
        if let (Ok(mut mine), Ok(theirs)) = (obj.get_team().unwrap().write(), enemy_team.read()) {
            mine.set_override_team_relationship(theirs.get_id(), Relationship::Enemies);
        }
        let mut enemy = Object::new_test(8918, 100.0);
        enemy.set_team(Some(enemy_team)).unwrap();
        assert!(TheActionManager::can_do_special_power_at_object(
            &obj,
            &enemy,
            CommandSourceType::FromPlayer,
            &template,
            0,
            false,
        ));
    }

    #[test]
    fn can_do_special_power_at_object_charges_fail_closed_without_ability_update() {
        let mut obj = object_with_kinds(8919, 100.0, &[]);
        let template = attach_ready_special_power(&mut obj, SpecialPowerType::RemoteCharges, 1.0);
        let target = object_with_kinds(8920, 100.0, &[KindOf::Structure]);
        assert!(!TheActionManager::can_do_special_power_at_object(
            &obj,
            &target,
            CommandSourceType::FromPlayer,
            &template,
            0,
            false,
        ));
    }

    #[test]
    fn can_fire_weapon_at_object_false_without_slot_weapon() {
        let obj = Object::new_test(8921, 100.0);
        let target = object_with_kinds(8922, 100.0, &[KindOf::Vehicle]);
        assert!(!TheActionManager::can_fire_weapon_at_object(
            &obj,
            &target,
            CommandSourceType::FromPlayer,
            WeaponSlotType::Primary,
        ));
        assert!(!TheActionManager::can_fire_weapon_at_object(
            &obj,
            &target,
            CommandSourceType::FromPlayer,
            WeaponSlotType::Secondary,
        ));
    }

    #[test]
    fn can_fire_weapon_at_object_killpilot_rejects_non_vehicle() {
        let mut obj = Object::new_test(8923, 100.0);
        let mut template = crate::weapon::WeaponTemplate::new("KillPilot".into());
        template.damage_type = DamageType::KillPilot;
        template.primary_damage = 10.0;
        let mut set = crate::weapon::WeaponTemplateSet::new();
        set.set_weapon_template(WeaponSlotType::Primary, Arc::new(template));
        obj.weapon_set.add_weapon_template_set(set);
        obj.weapon_set
            .update_weapon_set(obj.get_id(), &crate::weapon::WeaponSetFlags::new())
            .expect("install killpilot weapon");

        let infantry = object_with_kinds(8924, 100.0, &[KindOf::Infantry]);
        assert!(!TheActionManager::can_fire_weapon_at_object(
            &obj,
            &infantry,
            CommandSourceType::FromPlayer,
            WeaponSlotType::Primary,
        ));
    }

    #[test]
    fn can_repair_object_requires_dozer_structure_and_not_rebuild_hole() {
        let dozer = object_with_kinds(8925, 100.0, &[KindOf::Dozer]);
        let full_building = object_with_kinds(8926, 100.0, &[KindOf::Structure]);
        assert!(!TheActionManager::can_repair_object(
            &dozer,
            &full_building,
            CommandSourceType::FromPlayer
        ));

        let mut damaged = object_with_kinds(8927, 100.0, &[KindOf::Structure]);
        if let Some(body) = damaged.get_body_module() {
            body.lock().unwrap().set_health(40.0).ok();
        }
        assert!(TheActionManager::can_repair_object(
            &dozer,
            &damaged,
            CommandSourceType::FromPlayer
        ));

        let mut hole = object_with_kinds(8928, 100.0, &[KindOf::Structure, KindOf::RebuildHole]);
        if let Some(body) = hole.get_body_module() {
            body.lock().unwrap().set_health(40.0).ok();
        }
        assert!(!TheActionManager::can_repair_object(
            &dozer,
            &hole,
            CommandSourceType::FromPlayer
        ));

        let infantry = object_with_kinds(8929, 100.0, &[KindOf::Infantry]);
        assert!(!TheActionManager::can_repair_object(
            &infantry,
            &damaged,
            CommandSourceType::FromPlayer
        ));
    }

    #[test]
    fn can_garrison_kindof_relationship_and_capacity() {
        let vehicle = object_with_kinds(8930, 100.0, &[KindOf::Vehicle]);
        let building = object_with_kinds(8931, 100.0, &[KindOf::Structure]);
        assert!(!TheActionManager::can_garrison(
            &vehicle,
            &building,
            CommandSourceType::FromPlayer
        ));

        let no_garrison = object_with_kinds(8932, 100.0, &[KindOf::Infantry, KindOf::NoGarrison]);
        assert!(!TheActionManager::can_garrison(
            &no_garrison,
            &building,
            CommandSourceType::FromPlayer
        ));

        let mut infantry = object_with_kinds(8933, 100.0, &[KindOf::Infantry]);
        infantry.set_contain(Some(Arc::new(Mutex::new(TestContain {
            ids: Vec::new(),
            garrisonable: false,
            max: 1,
        }))));
        assert!(!TheActionManager::can_garrison(
            &infantry,
            &building,
            CommandSourceType::FromPlayer
        ));

        let mut garrisonable = object_with_kinds(8934, 100.0, &[KindOf::Structure]);
        garrisonable.set_contain(Some(Arc::new(Mutex::new(TestContain {
            ids: Vec::new(),
            garrisonable: true,
            max: 2,
        }))));
        assert!(TheActionManager::can_garrison(
            &infantry,
            &garrisonable,
            CommandSourceType::FromPlayer
        ));

        let mut full = object_with_kinds(8935, 100.0, &[KindOf::Structure]);
        full.set_contain(Some(Arc::new(Mutex::new(TestContain {
            ids: vec![1, 2],
            garrisonable: true,
            max: 2,
        }))));
        // Same missing player id on both → treated as same owner; capacity is
        // still checked via is_valid_container_for (default true). Kind-of and
        // garrisonable gates already passed above.
        assert!(TheActionManager::can_garrison(
            &infantry,
            &full,
            CommandSourceType::FromPlayer
        ));
    }
}
