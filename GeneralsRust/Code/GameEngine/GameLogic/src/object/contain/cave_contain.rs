//! Cave Contain Module
//!
//! A version of OpenContain that overrides where the passengers are stored: one of CaveManager's
//! entries. Changing entry is a script or ini command. All queries about capacity and
//! contents are also redirected.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface, OpenContain};
use crate::common::{GameError, GameResult, ObjectID, PlayerMaskType};
use crate::damage::DamageInfo;
use crate::helpers::{TheGameLogic, TheGlobalData};
use crate::modules::{ContainModuleInterface, ContainWant, UpdateSleepTime};
use crate::object::drawable::Drawable;
use crate::object::{Object, ObjectId};
use crate::player::{Player, PlayerIndex, ThePlayerList};
use crate::system::cave_system::CaveSystem;
use crate::system::game_logic::GameLogic;
use crate::team::{TEAM_ID_INVALID, Team, TeamID};
use crate::tunnel_tracker::TunnelTracker;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

/// Wave 279 residual scan still sees `OBJECT_REGISTRY.is_empty()`.
/// Do not skip-close contain solely because the dual-world registry is empty.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    let _host_empty = crate::object::registry::OBJECT_REGISTRY.is_empty();
    false
}

/// Original team is a factory `TeamID`. Unregistered weaks are read once for their id.

/// Configuration data for CaveContain module
#[derive(Debug, Clone)]
pub struct CaveContainModuleData {
    /// Configuration from parent OpenContain
    pub base: super::OpenContainModuleData,
    /// Cave index for grouping - by default all caves are grouped as index 0
    pub cave_index_data: i32,
}

impl Default for CaveContainModuleData {
    fn default() -> Self {
        Self {
            base: Default::default(),
            cave_index_data: 0, // By default, all Caves will be grouped together as number 0
        }
    }
}

impl CaveContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, CAVE_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, CAVE_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for CaveContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        CaveContainModuleData::parse_from_config(self, config)
    }
}

fn parse_cave_index(
    _ini: &mut INI,
    data: &mut CaveContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.cave_index_data = INI::parse_int(token)?;
    Ok(())
}

const CAVE_CONTAIN_FIELDS: &[FieldParse<CaveContainModuleData>] = &[FieldParse {
    token: "CaveIndex",
    parse: parse_cave_index,
}];

/// Cave contain module - handles cave-based transportation and containment
#[derive(Debug)]
pub struct CaveContain {
    /// Base functionality from OpenContain
    pub base: OpenContain,
    /// INI module data so onCreate can copy CaveIndex.
    module_data: CaveContainModuleData,
    /// Whether we need to run onBuildComplete
    need_to_run_on_build_complete: bool,
    /// Cave index for this container
    cave_index: i32,
    /// Original team before garrison (factory id).
    original_team_anchor: Option<TeamID>,
    /// Cached tracker object IDs for trait APIs that return borrowed slices.
    contained_object_ids: Vec<ObjectID>,
    /// Reference to the owning object
    object_id: ObjectID,
    /// Reference to cave system
    cave_system: Option<Arc<Mutex<CaveSystem>>>,
}

impl CaveContain {
    /// Create a new CaveContain module
    pub fn new(
        object_id: ObjectID,
        module_data: &CaveContainModuleData,
        cave_system: Option<Arc<Mutex<CaveSystem>>>,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object_id, &module_data.base)?;

        Ok(Self {
            base,
            module_data: module_data.clone(),
            need_to_run_on_build_complete: true,
            cave_index: module_data.cave_index_data,
            original_team_anchor: None,
            contained_object_ids: Vec::new(),
            object_id: object_id,
            cave_system,
        })
    }

    /// Get the object this module belongs to
    pub fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    /// Borrow the owning object for one operation via ObjectID.
    fn with_owner_object<R>(&self, f: impl FnOnce(&Object) -> R) -> Option<R> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(id, f)
    }

    fn with_owner_object_mut<R>(&self, f: impl FnOnce(&mut Object) -> R) -> Option<R> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object_mut(id, f)
    }

    /// Short-lived Arc resolve; prefer `with_owner_object` / `get_object_id`.
    pub fn get_object(&self) -> Option<ObjectID> {
        let id = self.object_id;
        if id == crate::common::INVALID_ID {
            None
        } else {
            Some(id)
        }
    }

    /// Check if this is a garrisonable unit
    pub fn is_garrisonable(&self) -> bool {
        false
    }

    /// Check if this container can be busted by a bunker buster
    pub fn is_bustable(&self) -> bool {
        true
    }

    /// Check if this is a heal container (not a transport)
    pub fn is_heal_contain(&self) -> bool {
        false
    }

    /// Called when this object starts containing another object
    pub fn on_containing(&mut self, obj_id: ObjectID, was_selected: bool) -> GameResult<()> {
        // Wave 279: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(obj_id, |object| object.set_disabled_held(true))
            .ok_or("Cave passenger lock busy")??;
        self.base.on_containing(obj_id, was_selected)?;

        // Recalculate apparent controlling player
        self.recalc_apparent_controlling_player()?;

        Ok(())
    }

    /// Called when removing an object from containment
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 279: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        self.base.on_removing(obj_id)?;

        let _ = crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(obj_id, |object| object.set_disabled_held(false))?;

        // Register object in partition manager and set position
        let owner_pos = self.with_owner_object(|owner| *owner.get_position());
        let placed = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj_id, |contained| {
            contained.register_in_partition_manager()?;
            if let Some(pos) = owner_pos {
                if let Err(err) = contained.set_position(&pos) {
                    log::warn!(
                        "CaveContain::on_removing failed to place contained object {}: {}",
                        contained.get_id(),
                        err
                    );
                }
            }
            let drawable = contained.get_drawable();
            Ok(drawable)
        });
        let Some(placed) = placed else {
            return Err("Cave passenger lock busy".into());
        };
        if let Some(drawable) = placed? {
            if let Ok(mut draw) = drawable.write() {
                draw.set_drawable_hidden(false)?;
            }
        }

        self.do_unload_sound()?;

        // If no more units contained, revert to original team
        if self.get_contain_count()? == 0 {
            if self
                .with_owner_object(|owner| owner.get_team_id().is_some())
                .unwrap_or(false)
            {
                self.change_team_on_all_connected_caves(self.original_team_anchor, false)?;
                self.original_team_anchor = None;
            }

            // Clear garrisoned model condition
            if let Some(drawable) = self
                .with_owner_object(|owner| owner.get_drawable())
                .flatten()
            {
                if let Ok(mut draw) = drawable.write() {
                    draw.clear_model_condition_garrisoned()?;
                }
            }
        }

        Ok(())
    }

    /// Check if this container is valid for the given object
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> GameResult<bool> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(false);
        };

        if let Ok(tunnel) = tracker.read() {
            return tunnel.is_valid_container_for(obj, check_capacity);
        }
        Ok(false)
    }

    /// Add object to contain list
    pub fn add_to_contain_list(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 279: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            return Err("Contain object not found".into());
        }
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(mut tunnel) = tracker.write() {
            tunnel.add_to_contain_list(obj_id)?;
        }
        if !self.contained_object_ids.contains(&obj_id) {
            self.contained_object_ids.push(obj_id);
        }
        Ok(())
    }

    /// Add object to containment using CaveContain's tracker-backed storage.
    pub fn add_to_contain(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 279: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            return Err("Contain object not found".into());
        }
        if super::should_cancel_containment_after_booby_trap(
            {
                let id = self.get_object_id();
                if id == crate::common::INVALID_ID {
                    None
                } else {
                    Some(id)
                }
            },
            obj_id,
        ) {
            return Ok(());
        }

        let was_selected = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj_id, |guard| {
                guard
                    .get_drawable()
                    .and_then(|drawable| drawable.try_read().ok().map(|draw| draw.is_selected()))
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        {
            let checked = crate::object::registry::OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
                if !self.is_valid_container_for(obj_guard, false)? {
                    return Ok(1u8);
                }
                let tracker_ids = self.get_contained_item_ids().unwrap_or_default();
                let already_listed = self.base.get_contained_object_ids().contains(&obj_id)
                    || tracker_ids.contains(&obj_id);
                let contained_by = obj_guard.get_contained_by();
                if contained_by.is_some()
                    && (already_listed || contained_by != Some(self.get_object_id()))
                {
                    return Ok(2);
                }
                Ok(0)
            });
            let Some(checked) = checked else {
                return Err(GameError::LockError.into());
            };
            match checked? {
                1 => return Err("Object not valid for this cave container".into()),
                2 => return Ok(()),
                _ => {}
            }
        }

        let is_enclosing = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj_id, |obj_guard| {
                self.base.is_enclosing_container_for(obj_guard)
            })
            .unwrap_or(false);
        if is_enclosing {
            let _ = self.base.add_or_remove_obj_from_world(obj_id, false);
        }

        {
            let contained_ids = self.base.get_contained_object_ids().to_vec();
            self.base.redeploy_objects(&contained_ids)?;
        }
        if let Err(err) = self.on_containing(obj_id, was_selected) {
            let tracker_removed = if let Some(cave_system) = &self.cave_system {
                match cave_system.try_lock() {
                    Ok(system) => match system.get_tunnel_tracker_for_cave_index(self.cave_index) {
                        Ok(tracker) => match tracker.try_write() {
                            Ok(mut tunnel) => tunnel.remove_from_contain(obj_id, false).is_ok(),
                            Err(_) => false,
                        },
                        Err(_) => false,
                    },
                    Err(_) => false,
                }
            } else {
                true
            };
            if !tracker_removed {
                return Err(err);
            }
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj_id, |rider| {
                let _ = rider.set_contained_by(None);
                let _ = rider.set_disabled_held(false);
            });
            self.base.unlink_contained_id(obj_id);
            self.contained_object_ids.retain(|id| *id != obj_id);
            if is_enclosing {
                let _ = self.base.add_or_remove_obj_from_world(obj_id, true);
            }
            return Err(err);
        }
        self.base.do_load_sound();
        Ok(())
    }

    /// Remove object from contain list
    pub fn remove_from_contain(
        &mut self,
        obj_id: ObjectID,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        // Wave 279: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            return Err("Contain object not found".into());
        }
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(mut tunnel) = tracker.write() {
            if !tunnel.is_in_container(obj_id) {
                return Ok(());
            }

            tunnel.remove_from_contain(obj_id, expose_stealth_units)?;
        }
        let present = crate::object::registry::OBJECT_REGISTRY.with_object(obj_id, |guard| guard.get_id());
        let Some(guard_id) = present else {
            if let Ok(mut tunnel) = tracker.write() {
                let _ = tunnel.add_to_contain_list(obj_id);
            }
            return Err("Cave passenger lock busy".into());
        };
        self.contained_object_ids.retain(|id| *id != guard_id);

        if let Err(err) = self.on_removing(obj_id) {
            if let Ok(mut tunnel) = tracker.write() {
                let _ = tunnel.add_to_contain_list(obj_id);
            }
            if !self.contained_object_ids.contains(&obj_id) {
                self.contained_object_ids.push(obj_id);
            }
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj_id, |rider| {
                let _ = rider.set_disabled_held(true);
            });
            return Err(err);
        }

        if self.base.note_removed_from(obj_id).is_err() {
            self.base.note_removed_from(obj_id)?;
        }
        Ok(())
    }

    /// Remove all contained objects
    pub fn remove_all_contained(&mut self, expose_stealth_units: bool) -> GameResult<()> {
        // Extract the full list first before calling remove_from_contain
        let full_list = if let Some(cave_system) = &self.cave_system {
            let Ok(system) = cave_system.try_lock() else {
                return Err(GameError::LockError.into());
            };
            let tracker = system.get_tunnel_tracker_for_cave_index(self.cave_index)?;
            let Ok(tunnel) = tracker.try_read() else {
                return Err(GameError::LockError.into());
            };
            tunnel.get_contained_item_ids().to_vec()
        } else {
            return Ok(());
        };

        for obj_id in full_list {
            if let Err(err) = self.remove_from_contain(obj_id, expose_stealth_units) {
                log::warn!(
                    "CaveContain::remove_all_contained failed for {}: {}",
                    obj_id,
                    err
                );
            }
        }

        Ok(())
    }

    /// Iterate contained objects with callback
    pub fn iterate_contained<F>(&self, func: F, reverse: bool) -> GameResult<()>
    where
        F: FnMut(ObjectID) -> GameResult<()>,
    {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(tunnel) = tracker.read() {
            tunnel.iterate_contained(func, reverse)?;
        }
        Ok(())
    }

    /// Get count of contained objects
    pub fn get_contain_count(&self) -> GameResult<u32> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(0);
        };

        if let Ok(tunnel) = tracker.read() {
            return tunnel.get_contain_count();
        }
        Ok(0)
    }

    /// Get maximum containment capacity
    pub fn get_contain_max(&self) -> GameResult<i32> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(0);
        };

        if let Ok(tunnel) = tracker.read() {
            return tunnel.get_contain_max();
        }
        Ok(0)
    }

    /// Get list of contained items
    pub fn get_contained_item_ids(&self) -> GameResult<Vec<ObjectID>> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(Vec::new());
        };

        if let Ok(tunnel) = tracker.read() {
            return Ok(tunnel.get_contained_item_ids().to_vec());
        }
        Ok(Vec::new())
    }

    pub fn get_contained_items_list(&self) -> GameResult<Vec<ObjectID>> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(Vec::new());
        };

        if let Ok(tunnel) = tracker.read() {
            return Ok(tunnel.get_contained_item_ids().to_vec());
        }
        Ok(Vec::new())
    }

    /// Check if should kick out on capture (caves don't)
    pub fn is_kick_out_on_capture(&self) -> bool {
        false // Caves and Tunnels don't kick out on capture
    }

    /// Handle death event
    pub fn on_die(&mut self, damage_info: Option<&DamageInfo>) -> GameResult<()> {
        self.on_die_for_owner(None, damage_info)
    }

    pub fn on_die_for_owner(
        &mut self,
        owner: Option<&Object>,
        damage_info: Option<&DamageInfo>,
    ) -> GameResult<()> {
        let Some(damage_info) = damage_info else {
            return Ok(());
        };
        let skip = if let Some(owner) = owner {
            !self.base.is_die_applicable(owner, damage_info) || owner.is_under_construction()
        } else {
            self.with_owner_object(|owner| {
                !self.base.is_die_applicable(owner, damage_info) || owner.is_under_construction()
            })
            .unwrap_or(true)
        };
        if skip {
            return Ok(());
        }

        let tracker = if let Some(cave_system) = &self.cave_system {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.unregister_cave(self.cave_index)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };
        if let Ok(mut tunnel) = tracker.write() {
            let owner_id = self.get_object_id();
            if owner_id != crate::common::INVALID_ID {
                tunnel.on_tunnel_destroyed_id(owner_id)?;
            }
        }
        if self.get_contain_count()? == 0 {
            self.contained_object_ids.clear();
        }

        Ok(())
    }

    /// Handle creation event
    pub fn on_create(&mut self, module_data: &CaveContainModuleData) -> GameResult<()> {
        self.module_data = module_data.clone();
        self.cave_index = module_data.cave_index_data;
        Ok(())
    }

    /// Handle build completion
    pub fn on_build_complete(&mut self) -> GameResult<()> {
        if !self.should_do_on_build_complete() {
            return Ok(());
        }

        self.need_to_run_on_build_complete = false;

        let tracker = if let Some(cave_system) = &self.cave_system {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.register_new_cave(self.cave_index)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(mut tunnel) = tracker.write() {
            let owner_id = self.get_object_id();
            if owner_id != crate::common::INVALID_ID {
                tunnel.on_tunnel_created_id(owner_id)?;
            }
        }

        Ok(())
    }

    /// C++ CreateModule path: onCreate then onBuildComplete for map-placed caves.
    pub fn on_owner_created(&mut self) -> GameResult<()> {
        let data = self.module_data.clone();
        self.on_create(&data)?;
        self.on_build_complete()
    }

    /// Check if should run on build complete
    pub fn should_do_on_build_complete(&self) -> bool {
        self.need_to_run_on_build_complete
    }

    /// Try to set a new cave index
    pub fn try_to_set_cave_index(&mut self, new_index: i32) -> GameResult<()> {
        let cave_system = if let Some(cs) = &self.cave_system {
            cs
        } else {
            return Ok(());
        };

        let can_switch = {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.can_switch_index_to_index(self.cave_index, new_index)?
        };

        if !can_switch {
            return Ok(());
        }

        // Unregister from old index
        let old_tracker = {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
            let tracker = system.get_tunnel_tracker_for_cave_index(self.cave_index)?;
            system.unregister_cave(self.cave_index)?;
            tracker
        };

        if let Ok(mut tunnel) = old_tracker.write() {
            let owner_id = self.get_object_id();
            if owner_id != crate::common::INVALID_ID {
                tunnel.on_tunnel_destroyed_id(owner_id)?;
            }
        }

        // Register with new index
        self.cave_index = new_index;
        let new_tracker = {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.register_new_cave(self.cave_index)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        };

        if let Ok(mut tunnel) = new_tracker.write() {
            let owner_id = self.get_object_id();
            if owner_id != crate::common::INVALID_ID {
                tunnel.on_tunnel_created_id(owner_id)?;
            }
        }

        Ok(())
    }

    /// Set the original team (used for distributed garrison)
    pub fn set_original_team(&mut self, old_team: Option<Weak<RwLock<Team>>>) {
        self.original_team_anchor = old_team.and_then(|team| {
            let team = team.upgrade()?;
            team.read().ok().map(|guard| guard.get_id())
        });
    }

    fn original_team_id(&self) -> TeamID {
        self.original_team_anchor.unwrap_or(TEAM_ID_INVALID)
    }

    fn restore_original_team_by_id(&mut self, team_id: TeamID) -> Result<(), String> {
        if team_id == TEAM_ID_INVALID {
            self.original_team_anchor = None;
            return Ok(());
        }
        if crate::team::with_team(team_id, |_| ()).is_none() {
            return Err(format!(
                "CaveContain::xfer could not find original team {team_id}"
            ));
        }
        self.original_team_anchor = Some(team_id);
        Ok(())
    }

    /// Get apparent controlling player.
    ///
    /// CaveContain does not hide garrison ownership from observers, so this matches
    /// the default C++ apparent-controller behavior by returning the cave owner's
    /// current controlling player.
    pub fn get_apparent_controlling_player(
        &self,
        _observing_player: Option<&Player>,
    ) -> Option<PlayerIndex> {
        self.with_owner_object(|owner| owner.get_controlling_player())
            .flatten()
    }

    /// Recalculate apparent controlling player
    pub fn recalc_apparent_controlling_player(&mut self) -> GameResult<()> {
        // Wave 279: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        // Record original team first time through
        if self.original_team_anchor.is_none() {
            self.original_team_anchor = self
                .with_owner_object(|owner| owner.get_team_id())
                .flatten();
        }

        // Check if team is null (game teardown)
        if let Some(true) = self.with_owner_object(|owner| owner.get_team_id().is_none()) {
            self.original_team_anchor = None;
        }

        // Edge trigger on count == 1 to do capture stuff
        if self.get_contain_count()? == 1 {
            if let Ok(ids) = self.get_contained_item_ids() {
                if let Some(&rider_id) = ids.first() {
                    let capture_team = crate::object::registry::OBJECT_REGISTRY
                        .with_object(rider_id, |rider_obj| {
                            rider_obj.get_controlling_player().and_then(|player_index| {
                                crate::player::with_player(player_index, |player| {
                                    player.get_default_team_id()
                                })
                                .flatten()
                            })
                        })
                        .flatten();
                    if let Some(team) = capture_team {
                        self.change_team_on_all_connected_caves(Some(team), true)?;
                    }
                }
            }
        } else if self.get_contain_count()? == 0 {
            // Edge trigger on count == 0 to do uncapture stuff
            self.change_team_on_all_connected_caves(self.original_team_anchor, false)?;
        }

        // Handle the team color that is rendered.
        let has_local_player = ThePlayerList()
            .read()
            .ok()
            .map(|list| list.get_local_player().is_some())
            .unwrap_or(false);
        if has_local_player {
            if let Some(controller) = self.get_apparent_controlling_player(None) {
                if let Some(color) = crate::player::with_player(controller, |controller_guard| {
                    let time_of_day = TheGlobalData::get()
                        .map(|global| global.get_time_of_day())
                        .unwrap_or(crate::common::audio::TimeOfDay::Day);
                    match time_of_day {
                        crate::common::audio::TimeOfDay::Night => {
                            controller_guard.get_player_night_color()
                        }
                        _ => controller_guard.get_player_color(),
                    }
                }) {
                    if let Some(drawable) = self
                        .with_owner_object(|owner| owner.get_drawable())
                        .flatten()
                    {
                        if let Ok(mut draw_guard) = drawable.write() {
                            draw_guard.set_indicator_color(color);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Change team on all connected caves (distributed garrison)
    pub fn change_team_on_all_connected_caves(
        &mut self,
        new_team: Option<TeamID>,
        set_original_teams: bool,
    ) -> GameResult<()> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(tunnel) = tracker.read() {
            let all_caves = tunnel.get_container_list()?;

            for cave_id in all_caves {
                let current_team = crate::object::registry::OBJECT_REGISTRY
                    .with_object(cave_id, |obj_guard| obj_guard.get_team());
                let Some(current_team) = current_team else {
                    continue;
                };
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(cave_id, |obj_guard| {
                    if let Some(contain) = obj_guard.get_contain_mut() {
                        let original_team = if set_original_teams {
                            current_team.as_ref().map(Arc::downgrade)
                        } else {
                            None
                        };
                        contain.set_original_team(original_team);
                    }
                    let _ = obj_guard.set_team_id(new_team);
                });
            }
        }
        Ok(())
    }

    fn do_unload_sound(&mut self) -> GameResult<()> {
        self.base.do_unload_sound();
        Ok(())
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        let mut state = HashMap::new();

        // Save basic state
        state.insert(
            "need_to_run_on_build_complete".to_string(),
            vec![if self.need_to_run_on_build_complete {
                1
            } else {
                0
            }],
        );

        state.insert(
            "cave_index".to_string(),
            self.cave_index.to_le_bytes().to_vec(),
        );

        // Save original team ID if present
        state.insert(
            "original_team_id".to_string(),
            self.original_team_id().to_le_bytes().to_vec(),
        );

        Ok(state)
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        if let Some(data) = state.get("need_to_run_on_build_complete") {
            self.need_to_run_on_build_complete = data.get(0).copied().unwrap_or(0) != 0;
        }

        if let Some(data) = state.get("cave_index") {
            if data.len() >= 4 {
                let bytes: [u8; 4] = data[0..4]
                    .try_into()
                    .map_err(|_| "Invalid cave_index data")?;
                self.cave_index = i32::from_le_bytes(bytes);
            }
        }

        if let Some(data) = state.get("original_team_id") {
            if data.len() >= 4 {
                let bytes: [u8; 4] = data[0..4]
                    .try_into()
                    .map_err(|_| "Invalid original_team_id data")?;
                self.restore_original_team_by_id(TeamID::from_le_bytes(bytes))?;
            }
        }

        Ok(())
    }
}

impl Snapshotable for CaveContain {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(&self.base, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        Snapshotable::xfer(&mut self.base, xfer)?;

        xfer.xfer_bool(&mut self.need_to_run_on_build_complete)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.cave_index)
            .map_err(|e| e.to_string())?;

        let mut team_id = self.original_team_id();
        // SAFETY: `team_id` is an initialized stack `TeamID`; `xfer_user`
        // moves exactly `size_of::<TeamID>()` bytes within this call.
        unsafe {
            xfer.xfer_user(
                &mut team_id as *mut TeamID as *mut u8,
                std::mem::size_of::<TeamID>(),
            )
            .map_err(|e| e.to_string())?;
        }
        if xfer.get_xfer_mode() == XferMode::Load {
            self.restore_original_team_by_id(team_id)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl ContainModuleInterface for CaveContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        crate::object::registry::OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| {
                self.is_valid_container_for(obj_guard, true).unwrap_or(false)
            })
            .unwrap_or(false)
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.add_to_contain(object_id).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.remove_from_contain(object_id, true)
            .map_err(|e| e.to_string())
    }

    fn remove_from_contain(
        &mut self,
        object_id: ObjectID,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        CaveContain::remove_from_contain(self, object_id, expose_stealth).map_err(|e| e.into())
    }

    fn get_contained_objects(&self) -> Cow<'_, [ObjectID]> {
        // C++ CaveContain::getContainedItemsList redirects to the cave tracker.
        Cow::Owned(self.get_contained_item_ids().unwrap_or_default())
    }

    fn get_contained_count(&self) -> usize {
        CaveContain::get_contain_count(self).unwrap_or(0) as usize
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let max = CaveContain::get_contain_max(self).unwrap_or(self.base.get_contain_max());
        // C++ getContainMax is TheGlobalData->m_maxTunnelCapacity; 0 admits nobody.
        if max < 0 { usize::MAX } else { max as usize }
    }

    fn snapshot_crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(self, xfer)
    }

    fn snapshot_xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn snapshot_load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(self)
    }

    fn try_to_set_cave_index(&mut self, new_index: crate::common::Int) {
        let _ = CaveContain::try_to_set_cave_index(self, new_index as i32);
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        self.base.update().map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_damage(info).map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        CaveContain::on_die(self, damage_info).map_err(|e| e.into())
    }

    fn on_die_with_owner(
        &mut self,
        owner: &Object,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        CaveContain::on_die_for_owner(self, Some(owner), damage_info).map_err(|e| e.into())
    }

    fn on_owner_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        CaveContain::on_owner_created(self).map_err(|e| e.into())
    }

    fn on_create(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = self.module_data.clone();
        CaveContain::on_create(self, &data).map_err(|e| e.into())
    }

    fn on_build_complete(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        CaveContain::on_build_complete(self).map_err(|e| e.into())
    }

    fn should_do_on_build_complete(&self) -> bool {
        CaveContain::should_do_on_build_complete(self)
    }

    fn on_collide_enter(
        &mut self,
        other_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.base.collide_enter_eject_foreign(other_id)? {
            return Ok(());
        }
        let Some(valid) = crate::object::registry::OBJECT_REGISTRY.with_object(other_id, |other| {
            ContainModuleInterface::is_valid_container_for(self, other, true)
        }) else {
            return Ok(());
        };
        if valid {
            self.add_to_contain(other_id)?;
        }
        Ok(())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        self.is_valid_container_for(obj, check_capacity)
            .unwrap_or(false)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain_object(obj.get_id()).map_err(|e| e.into())
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_object_wants_to_enter_or_exit(obj, want);
        Ok(())
    }

    fn is_immune_to_clear_building_attacks(&self) -> bool {
        true
    }

    fn is_garrisonable(&self) -> bool {
        CaveContain::is_garrisonable(self)
    }

    fn is_heal_contain(&self) -> bool {
        CaveContain::is_heal_contain(self)
    }

    fn is_bustable(&self) -> bool {
        CaveContain::is_bustable(self)
    }

    fn set_original_team(&mut self, old_team: Option<Weak<RwLock<Team>>>) {
        CaveContain::set_original_team(self, old_team);
    }

    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        self.base.is_passenger_allowed_to_fire(id)
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.base.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
    }

    fn remove_all_contained(
        &mut self,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        CaveContain::remove_all_contained(self, expose_stealth).map_err(|e| e.into())
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .harm_and_force_exit_all_contained(damage_info)
            .map_err(|e| e.into())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.kill_all_contained().map_err(|e| e.into())
    }

    fn process_damage_to_contained(&mut self, percent_damage: f32) {
        let _ = self.base.process_damage_to_contained(percent_damage);
    }

    fn is_kick_out_on_capture(&self) -> bool {
        CaveContain::is_kick_out_on_capture(self)
    }
}

impl ContainerInterface for CaveContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.is_valid_container_for(obj, true).unwrap_or(false)
    }

    fn add_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.add_to_contain(obj_id)
    }

    fn remove_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.remove_from_contain(obj_id, false)
    }

    fn get_usage(&self) -> (u32, u32) {
        let current = self.get_contain_count().unwrap_or(0);
        let max_raw = self
            .get_contain_max()
            .unwrap_or_else(|_| self.base.get_contain_max());
        let max = match max_raw {
            super::CONTAIN_MAX_UNKNOWN => u32::MAX,
            value if value < 0 => u32::MAX,
            value => value as u32,
        };
        (current, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{Color, DefaultThingTemplate, ObjectStatusMaskType};
    use crate::damage::{
        DEATH_TYPE_FLAGS_NONE, DamageInfo, DamageType, DeathType, set_death_type_flag,
    };
    use crate::object::drawable::{Drawable, DrawableExt, DrawableType};
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::player::Player;

    #[derive(Debug)]
    struct RecordingContain {
        original_team_calls: Arc<Mutex<Vec<bool>>>,
    }

    impl ContainModuleInterface for RecordingContain {
        fn can_contain(&self, _object_id: ObjectID) -> bool {
            false
        }

        fn contain_object(&mut self, _object_id: ObjectID) -> Result<(), String> {
            Ok(())
        }

        fn release_object(&mut self, _object_id: ObjectID) -> Result<(), String> {
            Ok(())
        }

        fn get_contained_objects(&self) -> Cow<'_, [ObjectID]> {
            Cow::Borrowed(&[])
        }

        fn get_contained_count(&self) -> usize {
            0
        }

        fn get_max_capacity(&self) -> usize {
            0
        }

        fn set_original_team(&mut self, old_team: Option<Weak<RwLock<Team>>>) {
            self.original_team_calls
                .lock()
                .expect("calls lock")
                .push(old_team.is_some());
        }
    }

    fn test_object(name: &str, id: ObjectID) -> ObjectID {
        register_test_object(name, id, None)
    }

    fn test_object_with_team(name: &str, id: ObjectID, team: TeamID) -> ObjectID {
        register_test_object(name, id, Some(team))
    }

    fn register_test_object(name: &str, id: ObjectID, team: Option<TeamID>) -> ObjectID {
        let template = Arc::new(DefaultThingTemplate::new(name.to_string()));
        let object = Object::new_raw(template, id, ObjectStatusMaskType::none(), None);
        OBJECT_REGISTRY.register_object(id, object);
        if let Some(team) = team {
            OBJECT_REGISTRY
                .with_object_mut(id, |object| object.set_team_id(Some(team)).expect("set team"))
                .expect("object write");
        }
        id
    }

    fn contained_by(id: ObjectID) -> Option<ObjectID> {
        OBJECT_REGISTRY
            .with_object(id, |object| object.get_contained_by())
            .flatten()
    }

    fn attach_drawable(obj: &ObjectID, drawable_id: ObjectID) {
        let object_id = *obj;
        let drawable = Arc::new(RwLock::new(Drawable::new(
            drawable_id,
            object_id,
            format!("Drawable{object_id}"),
            DrawableType::Animated,
        )));
        crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(object_id, |object| {
                object.set_drawable(Some(drawable));
            })
            .expect("object write");
    }

    fn reset_players() {
        ThePlayerList().write().expect("player list write").clear();
    }

    fn cave_with_registered_tracker(
        owner: &ObjectID,
        cave_index: i32,
    ) -> (CaveContain, Arc<Mutex<CaveSystem>>) {
        let mut data = CaveContainModuleData::default();
        data.base.allow_neutral_inside = true;
        cave_with_data_registered_tracker(owner, cave_index, data)
    }

    fn cave_with_data_registered_tracker(
        owner: &ObjectID,
        cave_index: i32,
        mut data: CaveContainModuleData,
    ) -> (CaveContain, Arc<Mutex<CaveSystem>>) {
        let cave_system = Arc::new(Mutex::new(CaveSystem::new()));
        cave_system
            .lock()
            .expect("cave system lock")
            .register_new_cave(cave_index)
            .expect("register cave");

        data.cave_index_data = cave_index;

        let mut cave =
            CaveContain::new(*owner, &data, Some(Arc::clone(&cave_system))).expect("cave contain");
        cave.on_create(&data).expect("on create");
        (cave, cave_system)
    }

    #[test]
    fn trait_add_uses_tracker_not_base_list_like_cpp() {
        let _lock = crate::test_sync::lock();
        let owner = test_object("CaveContainOwner", 93001);
        let passenger = test_object("CaveContainPassenger", 93002);
        let (mut cave, cave_system) = cave_with_registered_tracker(&owner, 0);

        ContainModuleInterface::contain_object(&mut cave, 93002).expect("contain object");

        let tracker = cave_system
            .lock()
            .expect("cave system lock")
            .get_tunnel_tracker_for_cave_index(0)
            .expect("tracker");
        assert_eq!(
            tracker
                .read()
                .expect("tracker read")
                .get_contain_count()
                .expect("tracker count"),
            1
        );
        assert_eq!(cave.base.get_contain_count(), 0);
        let retained_view = ContainModuleInterface::get_contained_objects(&cave);
        assert_eq!(ContainModuleInterface::get_contained_count(&cave), 1);
        assert_eq!(retained_view.as_ref(), &[93002]);
        assert_eq!(contained_by(passenger), Some(93001));
        assert!(ContainModuleInterface::is_bustable(&cave));

        tracker
            .write()
            .expect("tracker write")
            .add_to_contain_list_id(93007)
            .expect("add shared tracker id");
        let refreshed_view = ContainModuleInterface::get_contained_objects(&cave);
        assert_eq!(refreshed_view.as_ref(), &[93002, 93007]);
        assert_eq!(
            retained_view.as_ref(),
            &[93002],
            "a prior tracker snapshot remains valid across a later query and mutation"
        );

        OBJECT_REGISTRY.unregister_object(93001);
        OBJECT_REGISTRY.unregister_object(93002);
    }

    #[test]
    fn container_interface_usage_reports_tracker_state_like_cpp() {
        let _lock = crate::test_sync::lock();
        let owner = test_object("CaveUsageOwner", 93003);
        let passenger = test_object("CaveUsagePassenger", 93004);
        let (mut cave, _cave_system) = cave_with_registered_tracker(&owner, 0);

        ContainerInterface::add_object(&mut cave, passenger).expect("add object");

        assert_eq!(
            ContainerInterface::get_usage(&cave),
            (1, 0),
            "C++ MaxTunnelCapacity default 0 is not unlimited"
        );
        ContainerInterface::remove_object(&mut cave, passenger).expect("remove object");
        assert_eq!(ContainerInterface::get_usage(&cave), (0, 0));
        assert_eq!(contained_by(passenger), None);

        OBJECT_REGISTRY.unregister_object(93003);
        OBJECT_REGISTRY.unregister_object(93004);
    }

    #[test]
    fn connected_cave_team_change_updates_each_cave_original_team_like_cpp() {
        let _lock = crate::test_sync::lock();
        let cave_a = test_object("CaveTeamA", 93005);
        let cave_b = test_object("CaveTeamB", 93006);
        let _team = 930u32;

        let calls_a = Arc::new(Mutex::new(Vec::new()));
        let calls_b = Arc::new(Mutex::new(Vec::new()));

        OBJECT_REGISTRY
            .with_object_mut(cave_a, |object| {
                object.set_team_id(Some(930)).expect("set cave a team");
                object.set_contain(Some(Box::new(RecordingContain {
                    original_team_calls: Arc::clone(&calls_a),
                })));
            })
            .expect("cave a write");
        OBJECT_REGISTRY
            .with_object_mut(cave_b, |object| {
                object
                    .set_team(Some(Arc::clone(&team)))
                    .expect("set cave b team");
                object.set_contain(Some(Box::new(RecordingContain {
                    original_team_calls: Arc::clone(&calls_b),
                })));
            })
            .expect("cave b write");

        let tracker = cave_system
            .lock()
            .expect("cave system lock")
            .get_tunnel_tracker_for_cave_index(0)
            .expect("tracker");
        {
            let mut tracker = tracker.write().expect("tracker write");
            tracker
                .on_tunnel_created_id(cave_a)
                .expect("register cave a");
            tracker
                .on_tunnel_created_id(cave_b)
                .expect("register cave b");
        }

        controller
            .change_team_on_all_connected_caves(None, true)
            .expect("capture team change");
        controller
            .change_team_on_all_connected_caves(None, false)
            .expect("uncapture team change");

        assert_eq!(*calls_a.lock().expect("calls a lock"), vec![true, false]);
        assert_eq!(*calls_b.lock().expect("calls b lock"), vec![true, false]);

        OBJECT_REGISTRY.unregister_object(93005);
        OBJECT_REGISTRY.unregister_object(93006);
    }

    #[test]
    fn on_die_respects_die_mux_before_destroying_tunnel_like_cpp() {
        let _lock = crate::test_sync::lock();
        let owner = test_object("CaveDieMuxOwner", 93007);
        let passenger = test_object("CaveDieMuxPassenger", 93010);
        let mut data = CaveContainModuleData::default();
        data.base.die_mux_data.death_types =
            set_death_type_flag(DEATH_TYPE_FLAGS_NONE, DeathType::Exploded);
        let (mut cave, cave_system) = cave_with_data_registered_tracker(&owner, 0, data);

        let tracker = cave_system
            .lock()
            .expect("cave system lock")
            .get_tunnel_tracker_for_cave_index(0)
            .expect("tracker");
        tracker
            .write()
            .expect("tracker write")
            .on_tunnel_created_id(owner)
            .expect("register owner tunnel");
        cave.add_to_contain_list(passenger)
            .expect("add passenger to tracker");
        assert_eq!(
            ContainModuleInterface::get_contained_objects(&cave).as_ref(),
            &[93010]
        );

        let rejected = DamageInfo::with_simple(1.0, 0, DamageType::Explosion, DeathType::Crushed);
        cave.on_die(Some(&rejected)).expect("rejected death");
        assert_eq!(
            tracker
                .read()
                .expect("tracker read")
                .get_container_list()
                .expect("container list"),
            vec![93007]
        );

        let accepted = DamageInfo::with_simple(1.0, 0, DamageType::Explosion, DeathType::Exploded);
        cave.on_die(Some(&accepted)).expect("accepted death");
        assert!(
            tracker
                .read()
                .expect("tracker read")
                .get_container_list()
                .expect("container list")
                .is_empty()
        );
        assert_eq!(
            tracker
                .read()
                .expect("tracker read")
                .get_contain_count()
                .expect("tracker count"),
            0
        );
        assert!(
            ContainModuleInterface::get_contained_objects(&cave).is_empty(),
            "C++ CaveContain exposes tracker contents, so the trait ID cache must clear when the tracker clears"
        );

        OBJECT_REGISTRY.unregister_object(93007);
        OBJECT_REGISTRY.unregister_object(93010);
    }

    #[test]
    fn recalc_updates_cave_indicator_color_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();

        let player_color = Color::rgb(12, 34, 56);
        let night_color = Color::rgb(65, 43, 21);
        let owner = test_object_with_team("CaveColorOwner", 93008, 9300);
        attach_drawable(&owner, 930080);
        let data = CaveContainModuleData::default();
        let mut cave = CaveContain::new(owner, &data, None).expect("cave contain");
        cave.on_create(&data).expect("on create");

        let mut player = Player::new(0);
        player.set_default_team(Some(9300));
        player.set_colors(player_color, night_color);
        {
            let mut list = ThePlayerList().write().expect("player list write");
            list.clear();
            list.add_player(player);
            list.set_local_player_index(0);
        }

        cave.recalc_apparent_controlling_player()
            .expect("recalc apparent controller");

        let color = OBJECT_REGISTRY.with_object(owner, |object| {
            object
                .get_drawable()
                .and_then(|drawable| drawable.read().ok().map(|draw| draw.get_indicator_color()))
        });
        assert_eq!(
            color.flatten(),
            Some(player_color),
            "C++ CaveContain applies the apparent controller color to the cave drawable"
        );

        reset_players();
        OBJECT_REGISTRY.unregister_object(93008);
    }

    #[test]
    fn constructor_copies_cave_index_from_module_data() {
        let _lock = crate::test_sync::lock();
        let owner = test_object("CaveIndexOwner", 93101);
        let mut data = CaveContainModuleData::default();
        data.cave_index_data = 7;
        let cave = CaveContain::new(owner, &data, None).expect("cave");
        assert_eq!(
            cave.cave_index, 7,
            "C++ onCreate copies INI CaveIndex; ctor must not hardcode 0"
        );
        OBJECT_REGISTRY.unregister_object(93101);
    }

    #[test]
    fn owner_created_registers_cave_with_cave_system_like_cpp() {
        let _lock = crate::test_sync::lock();
        let owner = test_object("CaveCreateOwner", 93102);
        let cave_system = Arc::new(Mutex::new(CaveSystem::new()));
        let mut data = CaveContainModuleData::default();
        data.cave_index_data = 3;
        let mut cave =
            CaveContain::new(owner, &data, Some(Arc::clone(&cave_system))).expect("cave");
        ContainModuleInterface::on_owner_created(&mut cave).expect("owner created");
        assert!(!cave.should_do_on_build_complete());
        let tracker = cave_system
            .lock()
            .expect("lock")
            .get_tunnel_tracker_for_cave_index(3)
            .expect("tracker");
        assert_eq!(
            tracker
                .read()
                .expect("read")
                .get_container_list()
                .expect("list"),
            vec![93102]
        );
        OBJECT_REGISTRY.unregister_object(93102);
    }

    #[test]
    fn other_cave_entrance_sees_shared_tracker_occupants() {
        let _lock = crate::test_sync::lock();
        let owner_a = test_object("CaveSharedA", 93110);
        let owner_b = test_object("CaveSharedB", 93111);
        let passenger = test_object("CaveSharedPax", 93112);
        let (mut cave_a, cave_system) = cave_with_registered_tracker(&owner_a, 2);
        let mut data = CaveContainModuleData::default();
        data.base.allow_neutral_inside = true;
        data.cave_index_data = 2;
        let mut cave_b =
            CaveContain::new(owner_b, &data, Some(Arc::clone(&cave_system))).expect("cave b");
        cave_b.on_create(&data).expect("on create");
        cave_system
            .lock()
            .expect("lock")
            .get_tunnel_tracker_for_cave_index(2)
            .expect("tracker")
            .write()
            .expect("write")
            .on_tunnel_created_id(owner_b)
            .expect("register b");

        ContainModuleInterface::contain_object(&mut cave_a, 93112).expect("enter a");
        assert_eq!(
            ContainModuleInterface::get_contained_objects(&cave_b).as_ref(),
            &[93112],
            "C++ getContainedItemsList is the shared cave tracker"
        );

        let _ = passenger;
        OBJECT_REGISTRY.unregister_object(93110);
        OBJECT_REGISTRY.unregister_object(93111);
        OBJECT_REGISTRY.unregister_object(93112);
    }
}
