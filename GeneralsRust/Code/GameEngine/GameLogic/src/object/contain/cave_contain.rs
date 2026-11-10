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
use crate::player::{Player, ThePlayerList};
use crate::system::cave_system::CaveSystem;
use crate::system::game_logic::GameLogic;
use crate::team::{TEAM_ID_INVALID, Team, TeamID};
use crate::tunnel_tracker::TunnelTracker;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use game_engine::common::thing::module::{ModuleDataHandle, ModuleDataView};
use crate::contain_module_overrides::ContainModuleDataAdapter;

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
        ini.init_from_ini_with_inherited_fields(
            self,
            |data| &mut data.base,
            super::open_contain::OPEN_CONTAIN_FIELDS,
            CAVE_CONTAIN_FIELDS,
        )
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
    module_data: ModuleDataView<CaveContainModuleData>,
    /// Whether we need to run onBuildComplete
    need_to_run_on_build_complete: bool,
    /// Cave index for this container
    cave_index: i32,
    /// Original team before garrison
    original_team: Option<Weak<RwLock<Team>>>,
    /// Cached tracker object IDs for trait APIs that return borrowed slices.
    contained_object_ids: Vec<ObjectID>,
    /// Reference to the owning object
    object_id: ObjectID,
    /// Exact owner supplied at construction; an equal ID in another world
    /// must never select a different cave entrance.
    object: Weak<RwLock<Object>>,
    /// The owning world's cave system (C++ `TheCaveSystem` of the GameLogic
    /// that built this entrance). Always present: a cave is never assembled
    /// without its receiving world.
    cave_system: Arc<Mutex<CaveSystem>>,
}

impl CaveContain {
    /// Create a new CaveContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: ModuleDataHandle,
        world: crate::object::ObjectWorld,
    ) -> GameResult<Self> {
        let data = ModuleDataView::new(module_data.clone(), |data| {
            data.downcast_ref::<ContainModuleDataAdapter<CaveContainModuleData>>()
                .expect("cave contain module data has an incompatible type").contain_data()
        });
        let cave_index = data.read().cave_index_data;
        let mut base = OpenContain::new(object.clone(), module_data)?;
        base.bind_owning_world(world.world());

        Ok(Self {
            base,
            module_data: data,
            need_to_run_on_build_complete: true,
            cave_index,
            original_team: None,
            contained_object_ids: Vec::new(),
            object_id: object
                .upgrade()
                .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
                .unwrap_or(crate::common::INVALID_ID),
            object,
            cave_system: Arc::clone(world.cave()),
        })
    }

    /// Resolve this cave's tracker from an explicitly borrowed `CaveSystem`
    /// (C++ `TheCaveSystem->getTunnelTrackerForCaveIndex(m_caveIndex)`).
    pub fn tracker_in(&self, system: &CaveSystem) -> GameResult<Arc<RwLock<TunnelTracker>>> {
        system.get_tunnel_tracker_for_cave_index(self.cave_index)
    }

    /// C++ `registerNewCave(m_caveIndex)` then tracker lookup, on a borrowed system.
    pub fn register_in(&self, system: &mut CaveSystem) -> GameResult<Arc<RwLock<TunnelTracker>>> {
        system.register_new_cave(self.cave_index)?;
        self.tracker_in(system)
    }

    /// C++ `unregisterCave(m_caveIndex)` on a borrowed system.
    pub fn unregister_in(&self, system: &mut CaveSystem) -> GameResult<()> {
        system.unregister_cave(self.cave_index)
    }

    /// Run `f` with the owning world's cave system borrowed immutably. The
    /// borrow must not be held across tracker/object callbacks.
    fn with_cave_system<R>(
        &self,
        f: impl FnOnce(&CaveSystem) -> GameResult<R>,
    ) -> GameResult<R> {
        let system = self.cave_system.lock().map_err(|_| GameError::LockError)?;
        f(&system)
    }

    /// Mutable counterpart of [`Self::with_cave_system`].
    fn with_cave_system_mut<R>(
        &self,
        f: impl FnOnce(&mut CaveSystem) -> GameResult<R>,
    ) -> GameResult<R> {
        let mut system = self.cave_system.lock().map_err(|_| GameError::LockError)?;
        f(&mut system)
    }

    /// This cave's tracker via the owning world's cave system.
    fn tracker(&self) -> GameResult<Arc<RwLock<TunnelTracker>>> {
        self.with_cave_system(|system| self.tracker_in(system))
    }

    /// Non-blocking [`Self::tracker`]: a busy cave system is a `LockError`.
    fn try_tracker(&self) -> GameResult<Arc<RwLock<TunnelTracker>>> {
        let Ok(system) = self.cave_system.try_lock() else {
            return Err(GameError::LockError.into());
        };
        self.tracker_in(&system)
    }

    /// True when `system` is the cave system this entrance belongs to.
    pub fn is_owned_by(&self, system: &Arc<Mutex<CaveSystem>>) -> bool {
        Arc::ptr_eq(&self.cave_system, system)
    }

    /// Get the object this module belongs to
    pub fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    /// Borrow the exact owning object for one operation.
    fn with_owner_object<R>(&self, f: impl FnOnce(&Object) -> R) -> Option<R> {
        self.object.upgrade().and_then(|owner| {
            let owner = owner.read().ok()?;
            Some(f(&owner))
        })
    }

    fn with_owner_object_mut<R>(&self, f: impl FnOnce(&mut Object) -> R) -> Option<R> {
        self.object.upgrade().and_then(|owner| {
            let mut owner = owner.write().ok()?;
            Some(f(&mut owner))
        })
    }

    /// Short-lived Arc resolve; prefer `with_owner_object` / `get_object_id`.
    pub fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.object.upgrade()
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
        let Some(obj) = self.base.objects().get_object(obj_id) else {
            return Ok(());
        };

        let Ok(mut object) = obj.try_write() else {
            return Err("Cave passenger lock busy".into());
        };
        object.set_disabled_held(true)?;
        drop(object);
        self.base.on_containing(obj_id, was_selected)?;

        // Recalculate apparent controlling player
        self.recalc_apparent_controlling_player()?;

        Ok(())
    }

    /// Called when removing an object from containment
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        let Some(obj) = self.base.objects().get_object(obj_id) else {
            return Ok(());
        };

        self.base.on_removing(obj_id)?;

        // Object is no longer held inside a garrisoned building
        if let Ok(mut object) = obj.try_write() {
            object.set_disabled_held(false)?;
        }

        // Register object in partition manager and set position
        let owner_pos = self.with_owner_object(|owner| *owner.get_position());
        let Ok(mut contained) = obj.try_write() else {
            return Err("Cave passenger lock busy".into());
        };
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

        if let Some(drawable) = contained.get_drawable() {
            if let Ok(mut draw) = drawable.write() {
                draw.set_drawable_hidden(false)?;
            }
        }
        drop(contained);

        self.do_unload_sound()?;

        // If no more units contained, revert to original team
        if self.get_contain_count()? == 0 {
            if self
                .with_owner_object(|owner| owner.get_team().is_some())
                .unwrap_or(false)
            {
                self.change_team_on_all_connected_caves(self.original_team.clone(), false)?;
                self.original_team = None;
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
        let tracker = self.tracker()?;

        if let Ok(tunnel) = tracker.read() {
            let capacity = if check_capacity { self.base.world_tunnel_capacity() } else { 0 };
            return tunnel.is_valid_container_for_with_capacity(obj, check_capacity, capacity);
        }
        Ok(false)
    }

    /// Add object to contain list
    pub fn add_to_contain_list(&mut self, obj_id: ObjectID) -> GameResult<()> {
        let obj = self.base.objects()
            .get_object(obj_id)
            .ok_or("Contain object not found")?;
        let tracker = self.tracker()?;

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
        let obj = self.base.objects()
            .get_object(obj_id)
            .ok_or("Contain object not found")?;
        if super::should_cancel_containment_after_booby_trap(
            &self.base.objects(),
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

        let was_selected = obj
            .try_read()
            .ok()
            .and_then(|guard| guard.get_drawable())
            .and_then(|drawable| drawable.try_read().ok().map(|draw| draw.is_selected()))
            .unwrap_or(false);

        {
            let Ok(obj_guard) = obj.try_read() else {
                return Err(GameError::LockError.into());
            };
            // C++ OpenContain::addToContain validates with checkCapacity=false;
            // admission callers perform the capacity check before this method.
            if !self.is_valid_container_for(&obj_guard, false)? {
                return Err("Object not valid for this cave container".into());
            }
            // The C++ getter reads the canonical cave tracker without
            // mutating module state. Consult that tracker here as well so a
            // previous list query is not required to refresh a local mirror.
            let tracker_ids = self.get_contained_item_ids().unwrap_or_default();
            let already_listed = self.base.get_contained_object_ids().contains(&obj_id)
                || tracker_ids.contains(&obj_id);
            let contained_by = obj_guard.get_contained_by();
            if contained_by.is_some()
                && (already_listed || contained_by != Some(self.get_object_id()))
            {
                return Ok(());
            }
        }

        self.add_to_contain_list(obj_id)?;
        let is_enclosing = obj
            .try_read()
            .map(|obj_guard| self.base.is_enclosing_container_for(&obj_guard))
            .unwrap_or(false);
        if is_enclosing {
            let _ = self.base.add_or_remove_obj_from_world(obj_id, false);
        }

        {
            let contained_ids = self.base.get_contained_object_ids().to_vec();
            self.base.redeploy_objects(&contained_ids)?;
        }
        if let Err(err) = self.on_containing(obj_id, was_selected) {
            let tracker_removed = match self.try_tracker() {
                Ok(tracker) => match tracker.try_write() {
                    Ok(mut tunnel) => tunnel.remove_from_contain(obj_id, false).is_ok(),
                    Err(_) => false,
                },
                Err(_) => false,
            };
            if !tracker_removed {
                return Err(err);
            }
            if let Ok(mut rider) = obj.try_write() {
                let _ = rider.set_contained_by(None);
                let _ = rider.set_disabled_held(false);
            }
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
        let obj = self.base.objects()
            .get_object(obj_id)
            .ok_or("Contain object not found")?;
        let tracker = self.tracker()?;

        if let Ok(mut tunnel) = tracker.write() {
            if !tunnel.is_in_container(obj_id) {
                return Ok(());
            }

            tunnel.remove_from_contain(obj_id, expose_stealth_units)?;
        }
        let Ok(guard) = obj.try_read() else {
            if let Ok(mut tunnel) = tracker.write() {
                let _ = tunnel.add_to_contain_list(obj_id);
            }
            return Err("Cave passenger lock busy".into());
        };
        self.contained_object_ids.retain(|id| *id != guard.get_id());
        drop(guard);

        if let Err(err) = self.on_removing(obj_id) {
            if let Ok(mut tunnel) = tracker.write() {
                let _ = tunnel.add_to_contain_list(obj_id);
            }
            if !self.contained_object_ids.contains(&obj_id) {
                self.contained_object_ids.push(obj_id);
            }
            if let Ok(mut rider) = obj.try_write() {
                let _ = rider.set_disabled_held(true);
            }
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
        let tracker = self.try_tracker()?;
        let full_list = {
            let Ok(tunnel) = tracker.try_read() else {
                return Err(GameError::LockError.into());
            };
            tunnel.get_contained_item_ids().to_vec()
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
        F: FnMut(Arc<RwLock<Object>>) -> GameResult<()>,
    {
        let tracker = self.tracker()?;

        if let Ok(tunnel) = tracker.read() {
            tunnel.iterate_contained(func, reverse)?;
        }
        Ok(())
    }

    /// Get count of contained objects
    pub fn get_contain_count(&self) -> GameResult<u32> {
        let tracker = self.tracker()?;

        if let Ok(tunnel) = tracker.read() {
            return tunnel.get_contain_count();
        }
        Ok(0)
    }

    /// Get maximum containment capacity
    pub fn get_contain_max(&self) -> GameResult<i32> {
        self.tracker()?;
        // C++ TunnelTracker::getContainMax reads this world's GameData.
        Ok(self.base.world_tunnel_capacity())
    }

    /// Get list of contained items
    pub fn get_contained_item_ids(&self) -> GameResult<Vec<ObjectID>> {
        let tracker = self.tracker()?;

        if let Ok(tunnel) = tracker.read() {
            return Ok(tunnel.get_contained_item_ids().to_vec());
        }
        Ok(Vec::new())
    }

    pub fn get_contained_items_list(&self) -> GameResult<Vec<ObjectID>> {
        let tracker = self.tracker()?;

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

        let tracker = self.with_cave_system_mut(|system| {
            self.unregister_in(system)?;
            self.tracker_in(system)
        })?;
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
        self.cave_index = module_data.cave_index_data;
        Ok(())
    }

    /// Handle build completion
    pub fn on_build_complete(&mut self) -> GameResult<()> {
        if !self.should_do_on_build_complete() {
            return Ok(());
        }

        self.need_to_run_on_build_complete = false;

        let tracker = self.with_cave_system_mut(|system| self.register_in(system))?;

        self.finish_tracker_creation(&tracker)
    }

    /// C++ onBuildComplete with the driving CaveSystem borrowed by the caller.
    pub fn on_build_complete_in(&mut self, system: &mut CaveSystem) -> GameResult<()> {
        if !self.should_do_on_build_complete() {
            return Ok(());
        }
        self.need_to_run_on_build_complete = false;
        let tracker = self.register_in(system)?;
        self.finish_tracker_creation(&tracker)
    }

    fn finish_tracker_creation(&self, tracker: &Arc<RwLock<TunnelTracker>>) -> GameResult<()> {
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
        self.cave_index = self.module_data.read().cave_index_data;
        self.on_build_complete()
    }

    /// Check if should run on build complete
    pub fn should_do_on_build_complete(&self) -> bool {
        self.need_to_run_on_build_complete
    }

    /// Try to set a new cave index
    pub fn try_to_set_cave_index(&mut self, new_index: i32, system: &mut CaveSystem) -> GameResult<()> {
        // Both tracker rosters must be empty before C++ retires the old
        // entrance, so there are no passenger callbacks to run during this
        // borrow (CaveSystem.cpp:44-65, TunnelTracker.cpp:187-212).
        if !system.can_switch_index_to_index(self.cave_index, new_index)? {
            return Ok(());
        }

        let old_tracker = self.tracker_in(system)?;
        self.unregister_in(system)?;
        {
            let mut tunnel = old_tracker.write().map_err(|_| GameError::LockError)?;
            let owner_id = self.get_object_id();
            if owner_id != crate::common::INVALID_ID {
                tunnel.retire_entrance_metadata(owner_id);
            }
        }

        self.cave_index = new_index;
        let new_tracker = self.register_in(system)?;
        {
            let mut tunnel = new_tracker.write().map_err(|_| GameError::LockError)?;
            let owner_id = self.get_object_id();
            if owner_id != crate::common::INVALID_ID {
                tunnel.on_tunnel_created_id(owner_id)?;
            }
        }
        Ok(())
    }

    /// Set the original team (used for distributed garrison)
    pub fn set_original_team(&mut self, old_team: Option<Weak<RwLock<Team>>>) {
        self.original_team = old_team;
    }

    fn original_team_id(&self) -> TeamID {
        self.original_team
            .as_ref()
            .and_then(|team| team.upgrade())
            .and_then(|team| team.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(TEAM_ID_INVALID)
    }

    fn restore_original_team_by_id(&mut self, team_id: TeamID) -> Result<(), String> {
        if team_id == TEAM_ID_INVALID {
            self.original_team = None;
            return Ok(());
        }

        // C++ CaveContain::xfer: TheTeamFactory->findTeamByID of this world.
        let team = self
            .base
            .find_world_team(team_id)
            .ok_or_else(|| format!("CaveContain::xfer could not find original team {team_id}"))?;
        self.original_team = Some(Arc::downgrade(&team));
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
    ) -> Option<Arc<RwLock<Player>>> {
        self.with_owner_object(|owner| owner.get_controlling_player())
            .flatten()
    }

    /// Recalculate apparent controlling player
    pub fn recalc_apparent_controlling_player(&mut self) -> GameResult<()> {
        // Record original team first time through
        if self.original_team.is_none() {
            if let Some(team) = self.with_owner_object(|owner| owner.get_team()).flatten() {
                self.original_team = Some(Arc::downgrade(&team));
            }
        }

        // Check if team is null (game teardown)
        if let Some(true) = self.with_owner_object(|owner| owner.get_team().is_none()) {
            self.original_team = None;
        }

        // Edge trigger on count == 1 to do capture stuff
        if self.get_contain_count()? == 1 {
            if let Ok(ids) = self.get_contained_item_ids() {
                if let Some(&rider_id) = ids.first() {
                    if let Some(rider) =
                        self.base.objects().get_object(rider_id)
                    {
                        if let Ok(rider_obj) = rider.read() {
                            if let Some(controlling_player) = rider_obj.get_controlling_player() {
                                if let Ok(player) = controlling_player.read() {
                                    let default_team = player.get_default_team();
                                    self.change_team_on_all_connected_caves(
                                        default_team.map(|t| Arc::downgrade(&t)),
                                        true,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
        } else if self.get_contain_count()? == 0 {
            // Edge trigger on count == 0 to do uncapture stuff
            self.change_team_on_all_connected_caves(self.original_team.clone(), false)?;
        }

        // Handle the team color that is rendered.
        let has_local_player = {
            self.base
                .players()
                .and_then(|players| players.read().ok()?.get_local_player().cloned())
                .is_some()
        };
        if has_local_player {
            if let Some(controller) = self.get_apparent_controlling_player(None) {
                if let Ok(controller_guard) = controller.read() {
                    // C++ TheGlobalData->m_timeOfDay of the receiving world.
                    let time_of_day = self.base.time_of_day();
                    let color = match time_of_day {
                        crate::common::audio::TimeOfDay::Night => {
                            controller_guard.get_player_night_color()
                        }
                        _ => controller_guard.get_player_color(),
                    };
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
        new_team: Option<Weak<RwLock<Team>>>,
        set_original_teams: bool,
    ) -> GameResult<()> {
        let tracker = self.tracker()?;

        if let Ok(tunnel) = tracker.read() {
            let all_caves = tunnel.get_container_list()?;
            let team_arc = new_team.as_ref().and_then(|weak| weak.upgrade());

            for cave_id in all_caves {
                let Some(obj_arc) = self.base.objects().get_object(cave_id) else {
                    continue;
                };

                let (contain_arc, current_team) = {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    (obj_guard.get_contain(), obj_guard.get_team())
                };

                if let Some(contain) = contain_arc {
                    if let Ok(mut contain_guard) = contain.lock() {
                        let original_team = if set_original_teams {
                            current_team.as_ref().map(Arc::downgrade)
                        } else {
                            None
                        };
                        contain_guard.set_original_team(original_team);
                    }
                }

                let Ok(mut obj_guard) = obj_arc.write() else {
                    continue;
                };
                obj_guard.defect(team_arc.clone(), 0);
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

        let mut team_id = self.original_team_id().to_ne_bytes();
        xfer.xfer_user(&mut team_id).map_err(|e| e.to_string())?;
        let team_id = TeamID::from_ne_bytes(team_id);
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
    fn bind_owning_world(&mut self, world: &std::sync::Weak<crate::system::engine_stores::WorldServices>) {
        self.base.bind_owning_world(world);
    }

    fn is_valid_container_for_with_world(
        &self,
        passenger: &Object,
        owner: &Object,
        check_capacity: bool,
        world: &dyn crate::modules::ai_world::AiWorld,
    ) -> bool {
        if owner.get_id() != self.object_id {
            return false;
        }
        let (Some(owner_handle), Some(actual)) = (self.object.upgrade(), world.object(self.object_id)) else {
            return false;
        };
        if !Arc::ptr_eq(&owner_handle, &actual) {
            return false;
        }
        let Ok(guard) = owner_handle.try_read() else {
            return false;
        };
        if !std::ptr::eq(&*guard, owner) {
            return false;
        }
        drop(guard);
        let Some(tracker) = world.cave_tracker(self.cave_index) else {
            return false;
        };
        let Ok(tracker) = tracker.try_read() else {
            return false;
        };
        let capacity = if check_capacity {
            let Some(capacity) = world.cave_capacity() else {
                return false;
            };
            capacity
        } else {
            0
        };
        tracker.is_valid_container_for_with_capacity(passenger, check_capacity, capacity).unwrap_or(false)
    }

    fn can_contain(&self, object_id: ObjectID) -> bool {
        self.base.objects()
            .with_object(object_id, |obj_guard| {
            return self
                .is_valid_container_for(&*obj_guard, true)
                .unwrap_or(false);
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

    fn try_to_set_cave_index(
        &mut self,
        new_index: crate::common::Int,
        system: &mut CaveSystem,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        CaveContain::try_to_set_cave_index(self, new_index, system)
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

    fn on_owner_created_with_cave(
        &mut self,
        system: &mut CaveSystem,
    ) -> Option<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        self.cave_index = self.module_data.read().cave_index_data;
        Some(self.on_build_complete_in(system))
    }

    fn on_build_complete_with_cave(
        &mut self,
        system: &mut CaveSystem,
    ) -> Option<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        Some(self.on_build_complete_in(system))
    }

    fn on_create(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.cave_index = self.module_data.read().cave_index_data;
        Ok(())
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
        let Some(other) = self.base.objects().get_object(other_id) else {
            return Ok(());
        };
        let valid = other
            .try_read()
            .map(|guard| ContainModuleInterface::is_valid_container_for(self, &*guard, true))
            .unwrap_or(false);
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
        original_team_calls: Vec<bool>,
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
            self.original_team_calls.push(old_team.is_some());
        }
    }

    fn test_object(name: &str, id: ObjectID) -> Arc<RwLock<Object>> {
        let template = Arc::new(DefaultThingTemplate::new(name.to_string()));
        Object::new_with_id(template, id, ObjectStatusMaskType::none(), None).expect("test object")
    }

    fn test_object_with_team(
        name: &str,
        id: ObjectID,
        team: Arc<RwLock<Team>>,
    ) -> Arc<RwLock<Object>> {
        let template = Arc::new(DefaultThingTemplate::new(name.to_string()));
        Object::new_with_id(template, id, ObjectStatusMaskType::none(), Some(team))
            .expect("test object")
    }

    fn attach_drawable(obj: &Arc<RwLock<Object>>, drawable_id: ObjectID) -> Arc<RwLock<Drawable>> {
        let object_id = obj.read().expect("object read").get_id();
        let drawable = Arc::new(RwLock::new(Drawable::new(
            drawable_id,
            object_id,
            format!("Drawable{object_id}"),
            DrawableType::Animated,
        )));
        obj.write()
            .expect("object write")
            .set_drawable(Some(drawable.clone()));
        drawable
    }

    /// Run a test body inside a fresh published world so the object registry,
    /// player list and common services resolve (no process bag).
    fn with_test_world(body: impl FnOnce()) {
        let common = Arc::new(game_engine::common::common_services::CommonServices::new());
        game_engine::common::common_services::with_common_services(&common, || {
            let core = Arc::new(crate::system::engine_stores::EngineStores::new_for_world());
            crate::system::engine_stores::with_active_stores(&core, body);
        });
    }

    /// Fixture entrances live in the test's published world with `cave` as
    /// their CaveSystem.
    fn published_world_owner(cave: Arc<Mutex<CaveSystem>>) -> crate::object::ObjectWorld {
        crate::object::ObjectWorld::with_cave(&crate::system::engine_stores::active_services(), cave)
    }

    fn reset_players() {
        ThePlayerList().write().expect("player list write").clear();
    }

    fn cave_with_registered_tracker(
        owner: &Arc<RwLock<Object>>,
        cave_index: i32,
    ) -> (CaveContain, Arc<Mutex<CaveSystem>>) {
        let mut data = CaveContainModuleData::default();
        data.base.allow_neutral_inside = true;
        cave_with_data_registered_tracker(owner, cave_index, data)
    }

    fn cave_with_data_registered_tracker(
        owner: &Arc<RwLock<Object>>,
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
            CaveContain::new(Arc::downgrade(owner), ModuleDataHandle::new(ContainModuleDataAdapter::new(data.clone())), published_world_owner(Arc::clone(&cave_system)))
                .expect("cave contain");
        cave.on_create(&data).expect("on create");
        (cave, cave_system)
    }

    #[test]
    fn trait_add_uses_tracker_not_base_list_like_cpp() {
        let _lock = crate::test_sync::lock();
        with_test_world(|| {
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
            assert_eq!(
                passenger.read().expect("passenger read").get_contained_by(),
                Some(93001)
            );
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
        });
    }

    #[test]
    fn container_interface_usage_reports_tracker_state_like_cpp() {
        let _lock = crate::test_sync::lock();
        with_test_world(|| {
            let owner = test_object("CaveUsageOwner", 93003);
            let passenger = test_object("CaveUsagePassenger", 93004);
            let (mut cave, _cave_system) = cave_with_registered_tracker(&owner, 0);

            ContainerInterface::add_object(
                &mut cave,
                passenger
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID),
            )
            .expect("add object");

            assert_eq!(
                ContainerInterface::get_usage(&cave),
                (1, 0),
                "C++ MaxTunnelCapacity default 0 is not unlimited"
            );
            ContainerInterface::remove_object(
                &mut cave,
                passenger
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID),
            )
            .expect("remove object");
            assert_eq!(ContainerInterface::get_usage(&cave), (0, 0));
            assert_eq!(
                passenger.read().expect("passenger read").get_contained_by(),
                None
            );

            OBJECT_REGISTRY.unregister_object(93003);
            OBJECT_REGISTRY.unregister_object(93004);
        });
    }

    #[test]
    fn connected_cave_team_change_updates_each_cave_original_team_like_cpp() {
        let _lock = crate::test_sync::lock();
        with_test_world(|| {
            let cave_a = test_object("CaveTeamA", 93005);
            let cave_b = test_object("CaveTeamB", 93006);
            let (mut controller, cave_system) = cave_with_registered_tracker(&cave_a, 0);
            let team = Arc::new(RwLock::new(Team::new("TunnelTeam".into(), 930)));

            cave_a
                .write()
                .expect("cave a write")
                .set_team(Some(Arc::clone(&team)))
                .expect("set cave a team");
            cave_b
                .write()
                .expect("cave b write")
                .set_team(Some(Arc::clone(&team)))
                .expect("set cave b team");

            let receiver_a = Arc::new(Mutex::new(RecordingContain {
                original_team_calls: Vec::new(),
            }));
            let receiver_b = Arc::new(Mutex::new(RecordingContain {
                original_team_calls: Vec::new(),
            }));
            cave_a
                .write()
                .expect("cave a write")
                .set_contain(Some(receiver_a.clone()));
            cave_b
                .write()
                .expect("cave b write")
                .set_contain(Some(receiver_b.clone()));

            let tracker = cave_system
                .lock()
                .expect("cave system lock")
                .get_tunnel_tracker_for_cave_index(0)
                .expect("tracker");
            {
                let mut tracker = tracker.write().expect("tracker write");
                tracker
                    .on_tunnel_created(&*cave_a.read().expect("cave a read"))
                    .expect("register cave a");
                tracker
                    .on_tunnel_created(&*cave_b.read().expect("cave b read"))
                    .expect("register cave b");
            }

            controller
                .change_team_on_all_connected_caves(None, true)
                .expect("capture team change");
            controller
                .change_team_on_all_connected_caves(None, false)
                .expect("uncapture team change");

            assert_eq!(
                receiver_a
                    .lock()
                    .expect("contain a receiver")
                    .original_team_calls,
                vec![true, false]
            );
            assert_eq!(
                receiver_b
                    .lock()
                    .expect("contain b receiver")
                    .original_team_calls,
                vec![true, false]
            );

            OBJECT_REGISTRY.unregister_object(93005);
            OBJECT_REGISTRY.unregister_object(93006);
        });
    }

    #[test]
    fn on_die_respects_die_mux_before_destroying_tunnel_like_cpp() {
        let _lock = crate::test_sync::lock();
        with_test_world(|| {
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
                .on_tunnel_created(&*owner.read().expect("owner read"))
                .expect("register owner tunnel");
            cave.add_to_contain_list(
                passenger
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID),
            )
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
        });
    }

    #[test]
    fn recalc_updates_cave_indicator_color_like_cpp() {
        let _lock = crate::test_sync::lock();
        with_test_world(|| {
            reset_players();

            let player_color = Color::rgb(12, 34, 56);
            let night_color = Color::rgb(65, 43, 21);
            let team = Arc::new(RwLock::new(Team::new("CaveColorTeam".into(), 9300)));
            team.write()
                .expect("team write")
                .set_controlling_player_id(Some(0));
            let owner = test_object_with_team("CaveColorOwner", 93008, Arc::clone(&team));
            let owner_drawable = attach_drawable(&owner, 930080);
            let data = CaveContainModuleData::default();
            let (mut cave, _cave_system) = cave_with_data_registered_tracker(&owner, data.cave_index_data, data.clone());
            cave.on_create(&data).expect("on create");

            let player = Arc::new(RwLock::new(Player::new(0)));
            {
                let mut player_guard = player.write().expect("player write");
                player_guard.set_default_team(Some(Arc::clone(&team)));
                player_guard.set_colors(player_color, night_color);
            }
            {
                let __player_list_arc = ThePlayerList();
            let mut list = __player_list_arc.write().expect("player list write");
                list.clear();
                list.add_player(Arc::clone(&player));
                list.set_local_player_index(0);
            }

            cave.recalc_apparent_controlling_player()
                .expect("recalc apparent controller");

            assert_eq!(
                owner_drawable
                    .read()
                    .expect("drawable read")
                    .get_indicator_color(),
                player_color,
                "C++ CaveContain applies the apparent controller color to the cave drawable"
            );

            reset_players();
            OBJECT_REGISTRY.unregister_object(93008);
        });
    }

    #[test]
    fn constructor_copies_cave_index_from_module_data() {
        let _lock = crate::test_sync::lock();
        with_test_world(|| {
            let owner = test_object("CaveIndexOwner", 93101);
            let mut data = CaveContainModuleData::default();
            data.cave_index_data = 7;
            let cave = CaveContain::new(Arc::downgrade(&owner), ModuleDataHandle::new(ContainModuleDataAdapter::new(data)), published_world_owner(Arc::new(Mutex::new(CaveSystem::new())))).expect("cave");
            assert_eq!(
                cave.cave_index, 7,
                "C++ onCreate copies INI CaveIndex; ctor must not hardcode 0"
            );
            OBJECT_REGISTRY.unregister_object(93101);
        });
    }

    #[test]
    fn owner_created_registers_cave_with_cave_system_like_cpp() {
        let _lock = crate::test_sync::lock();
        with_test_world(|| {
            let owner = test_object("CaveCreateOwner", 93102);
            let cave_system = Arc::new(Mutex::new(CaveSystem::new()));
            let mut data = CaveContainModuleData::default();
            data.cave_index_data = 3;
            let mut cave = CaveContain::new(
                Arc::downgrade(&owner),
                ModuleDataHandle::new(ContainModuleDataAdapter::new(data)),
                published_world_owner(Arc::clone(&cave_system)),
            )
            .expect("cave");
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
        });
    }

    #[test]
    fn other_cave_entrance_sees_shared_tracker_occupants() {
        let _lock = crate::test_sync::lock();
        with_test_world(|| {
            let owner_a = test_object("CaveSharedA", 93110);
            let owner_b = test_object("CaveSharedB", 93111);
            let passenger = test_object("CaveSharedPax", 93112);
            let (mut cave_a, cave_system) = cave_with_registered_tracker(&owner_a, 2);
            let mut data = CaveContainModuleData::default();
            data.base.allow_neutral_inside = true;
            data.cave_index_data = 2;
            let mut cave_b = CaveContain::new(
                Arc::downgrade(&owner_b),
                ModuleDataHandle::new(ContainModuleDataAdapter::new(data.clone())),
                published_world_owner(Arc::clone(&cave_system)),
            )
            .expect("cave b");
            cave_b.on_create(&data).expect("on create");
            cave_system
                .lock()
                .expect("lock")
                .get_tunnel_tracker_for_cave_index(2)
                .expect("tracker")
                .write()
                .expect("write")
                .on_tunnel_created(&*owner_b.read().expect("b"))
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
        });
    }

    fn world_cave_handle() -> (
        Arc<crate::system::engine_stores::WorldServices>,
        Arc<Mutex<CaveSystem>>,
    ) {
        let services = crate::system::engine_stores::new_world_services();
        let cave = Arc::clone(&services.logic_managers().cave);
        (services, cave)
    }

    fn world_cave(
        owner: &Arc<RwLock<Object>>,
        handle: &Arc<Mutex<CaveSystem>>,
    ) -> CaveContain {
        let mut data = CaveContainModuleData::default();
        data.base.allow_neutral_inside = true;
        data.cave_index_data = 0;
        let mut cave =
            CaveContain::new(Arc::downgrade(owner), ModuleDataHandle::new(ContainModuleDataAdapter::new(data)), published_world_owner(Arc::clone(handle))).expect("cave");
        ContainModuleInterface::on_owner_created(&mut cave).expect("owner created");
        cave
    }

    #[test]
    fn two_world_cave_systems_do_not_contaminate_each_other() {
        let _lock = crate::test_sync::lock();
        with_test_world(|| {
            // Same owner object id in both worlds, same cave index, different occupants.
            let owner = test_object("CaveWorldOwner", 93201);
            let pax_a = test_object("CaveWorldPaxA", 93202);
            let pax_b1 = test_object("CaveWorldPaxB1", 93203);
            let pax_b2 = test_object("CaveWorldPaxB2", 93204);
            let (_services_a, handle_a) = world_cave_handle();
            let (_services_b, handle_b) = world_cave_handle();
            assert!(!Arc::ptr_eq(&handle_a, &handle_b));

            let mut cave_a = world_cave(&owner, &handle_a);
            let mut cave_b = world_cave(&owner, &handle_b);

            // A same-ID registry entry from another world cannot replace the
            // exact owner carried by the cave module.
            let foreign_owner = Arc::new(RwLock::new(Object::new_test(93201, 100.0)));
            OBJECT_REGISTRY.register_object(93201, &foreign_owner);
            assert!(Arc::ptr_eq(&cave_a.get_object().expect("exact owner"), &owner));
            OBJECT_REGISTRY.register_object(93201, &owner);

            // Interleave enters across the two worlds.
            ContainModuleInterface::contain_object(&mut cave_a, 93202).expect("a enter");
            ContainModuleInterface::contain_object(&mut cave_b, 93203).expect("b enter 1");
            ContainModuleInterface::contain_object(&mut cave_b, 93204).expect("b enter 2");
            assert_eq!(
                ContainModuleInterface::get_contained_objects(&cave_a).as_ref(),
                &[93202]
            );
            assert_eq!(
                ContainModuleInterface::get_contained_objects(&cave_b).as_ref(),
                &[93203, 93204],
                "enter order is preserved per world"
            );

            // Exit in world B must not touch world A.
            ContainModuleInterface::release_object(&mut cave_b, 93203).expect("b exit");
            assert_eq!(ContainModuleInterface::get_contained_count(&cave_a), 1);
            assert_eq!(
                ContainModuleInterface::get_contained_objects(&cave_b).as_ref(),
                &[93204]
            );

            // Destroying world A's cave empties only world A's network.
            let death = DamageInfo::with_simple(1.0, 0, DamageType::Explosion, DeathType::Exploded);
            cave_a.on_die(Some(&death)).expect("a die");
            let tracker_a = handle_a
                .lock()
                .expect("a lock")
                .get_tunnel_tracker_for_cave_index(0)
                .expect("a tracker");
            let tracker_b = handle_b
                .lock()
                .expect("b lock")
                .get_tunnel_tracker_for_cave_index(0)
                .expect("b tracker");
            assert!(tracker_a.read().expect("a read").get_container_list().expect("a list").is_empty());
            assert_eq!(
                tracker_b.read().expect("b read").get_container_list().expect("b list"),
                vec![93201],
                "world B still has its entrance"
            );
            assert_eq!(
                tracker_b.read().expect("b read").get_contained_item_ids(),
                &[93204]
            );

            // Explicit-borrow API addresses a specific system.
            let mut standalone = CaveSystem::new();
            let registered = cave_b.register_in(&mut standalone).expect("register in");
            assert_eq!(standalone.registered_indices(), vec![0]);
            assert!(!Arc::ptr_eq(&registered, &tracker_b));

            for id in [93201, 93202, 93203, 93204] {
                OBJECT_REGISTRY.unregister_object(id);
            }
            let _ = (pax_a, pax_b1, pax_b2);
        });
    }

    #[test]
    fn world_owned_cave_system_xfer_round_trip_preserves_order() {
        use game_engine::{Xfer, XferLoad, XferSave};
        let path = std::env::temp_dir().join(format!(
            "cave_system_round_trip_{}.xfer",
            std::process::id()
        ));
        let path_str = path.to_string_lossy().into_owned();

        let (_services, handle) = world_cave_handle();
        {
            let mut system = handle.lock().expect("lock");
            // C++ XferSave rejects a NULL tracker slot, so a saved system is dense.
            system.register_new_cave(0).expect("register 0");
            system.register_new_cave(1).expect("register 1");
            system.register_new_cave(2).expect("register 2");
            let tracker = system.get_tunnel_tracker_for_cave_index(2).expect("tracker 2");
            let mut tracker = tracker.write().expect("tracker write");
            tracker.on_tunnel_created_id(501).expect("tunnel 501");
            tracker.on_tunnel_created_id(502).expect("tunnel 502");
        }

        {
            let mut save = XferSave::new();
            save.open(path_str.clone()).expect("open save");
            handle.lock().expect("lock").xfer_game_logic(&mut save).expect("save");
            save.close().expect("close save");
        }

        let (_other_services, other) = world_cave_handle();
        {
            let mut load = XferLoad::new();
            load.open(path_str.clone()).expect("open load");
            other.lock().expect("lock").xfer_game_logic(&mut load).expect("load");
            load.close().expect("close load");
        }
        let _ = std::fs::remove_file(&path);

        let restored = other.lock().expect("lock");
        // Slot count and order survive; C++ restores every slot as a tracker.
        assert_eq!(restored.registered_indices(), vec![0, 1, 2]);
        let tracker = restored.get_tunnel_tracker_for_cave_index(2).expect("tracker 2");
        assert_eq!(
            tracker.read().expect("read").get_container_list().expect("list"),
            vec![501, 502]
        );
        let empty = restored.get_tunnel_tracker_for_cave_index(0).expect("tracker 0");
        assert!(empty.read().expect("read").get_container_list().expect("list").is_empty());
        // The source world is unchanged.
        assert_eq!(handle.lock().expect("lock").registered_indices(), vec![0, 1, 2]);
    }

    /// Observable cave state of one world after a full entrance lifecycle.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CaveWorldTrace {
        after_enter: Vec<ObjectID>,
        after_exit: Vec<ObjectID>,
        indices_after_switch: Vec<i32>,
        entrances_after_death: Vec<ObjectID>,
        restored_indices: Vec<i32>,
        restored_switched_entrances: Vec<ObjectID>,
        indices_after_reset: Vec<i32>,
    }

    /// C++ CaveContain.cpp enter/exit (addToContain/removeFromContain via the
    /// shared tracker), cave switch (tryToSetCaveIndex, :236-252, to a dense slot since XferSave rejects NULL trackers), death
    /// (onDie, :200-214), CaveSystem::xfer and CaveSystem::reset, all on the
    /// one world published on this thread. `gate` interleaves two worlds.
    fn run_cave_world(passengers: &[ObjectID], gate: Option<&std::sync::Barrier>) -> CaveWorldTrace {
        let wait = || {
            if let Some(gate) = gate {
                gate.wait();
            }
        };
        let mut trace = None;
        with_test_world(|| {
            let (_services, handle) = world_cave_handle();
            // Same entrance ids in every world.
            let entrance_a = test_object("ParallelCaveA", 93401);
            let entrance_b = test_object("ParallelCaveB", 93402);
            let riders: Vec<_> = passengers
                .iter()
                .map(|&id| test_object("ParallelCaveRider", id))
                .collect();
            let mut cave_a = world_cave(&entrance_a, &handle);
            let mut cave_b = world_cave(&entrance_b, &handle);
            wait();

            for &id in passengers {
                ContainModuleInterface::contain_object(&mut cave_a, id).expect("enter");
            }
            wait();
            let after_enter = ContainModuleInterface::get_contained_objects(&cave_b).into_owned();

            // Exit the first rider through the other entrance of the network.
            ContainModuleInterface::release_object(&mut cave_b, passengers[0]).expect("exit");
            wait();
            let after_exit = ContainModuleInterface::get_contained_objects(&cave_a).into_owned();

            cave_a.remove_all_contained(false).expect("evacuate");
            wait();
            cave_b
                .try_to_set_cave_index(1, &mut handle.lock().expect("switch lock"))
                .expect("switch");
            let indices_after_switch = handle.lock().expect("lock").registered_indices();
            wait();

            let death = DamageInfo::with_simple(1.0, 0, DamageType::Explosion, DeathType::Exploded);
            cave_a.on_die(Some(&death)).expect("die");
            let entrances_after_death = handle
                .lock()
                .expect("lock")
                .get_tunnel_tracker_for_cave_index(0)
                .expect("tracker 0")
                .read()
                .expect("read")
                .get_container_list()
                .expect("list");
            wait();

            use game_engine::{Xfer, XferLoad, XferSave};
            let path = std::env::temp_dir().join(format!(
                "parallel_cave_world_{}_{}.xfer",
                std::process::id(),
                passengers.len()
            ));
            let path_str = path.to_string_lossy().into_owned();
            let mut save = XferSave::new();
            save.open(path_str.clone()).expect("open save");
            handle.lock().expect("lock").xfer_game_logic(&mut save).expect("save");
            save.close().expect("close save");
            let (_restored_services, restored) = world_cave_handle();
            let mut load = XferLoad::new();
            load.open(path_str).expect("open load");
            restored.lock().expect("lock").xfer_game_logic(&mut load).expect("load");
            load.close().expect("close load");
            let _ = std::fs::remove_file(&path);
            let restored = restored.lock().expect("restored");
            let restored_indices = restored.registered_indices();
            let restored_switched_entrances = restored
                .get_tunnel_tracker_for_cave_index(1)
                .expect("tracker 1")
                .read()
                .expect("read")
                .get_container_list()
                .expect("list");
            wait();

            handle.lock().expect("lock").reset();
            let indices_after_reset = handle.lock().expect("lock").registered_indices();

            for id in [93401, 93402].into_iter().chain(passengers.iter().copied()) {
                OBJECT_REGISTRY.unregister_object(id);
            }
            drop(riders);
            trace = Some(CaveWorldTrace {
                after_enter,
                after_exit,
                indices_after_switch,
                entrances_after_death,
                restored_indices,
                restored_switched_entrances,
                indices_after_reset,
            });
        });
        trace.expect("cave world trace")
    }

    /// C++ each GameEngine's CaveContain resolves riders through its own
    /// TheGameLogic (CaveContain.cpp addToContain -> OpenContain::addToContain).
    /// Two worlds take turns on ONE thread with the same object ids: an entrance
    /// of world A, driven while world B is published, still moves world A's
    /// rider and leaves world B's same-id rider untouched, and vice versa.
    #[test]
    fn cave_entrances_resolve_their_own_world_when_worlds_take_turns_on_one_thread() {
        use crate::system::engine_stores::{with_active_stores, EngineStores};
        let _lock = crate::test_sync::lock();
        let common = Arc::new(game_engine::common::common_services::CommonServices::new());
        game_engine::common::common_services::with_common_services(&common, || {
            let world_a = Arc::new(EngineStores::new_for_world());
            let world_b = Arc::new(EngineStores::new_for_world());
            const ENTRANCE: ObjectID = 93601;
            const RIDER: ObjectID = 93602;
            let build = |stores: &Arc<EngineStores>, tag: &str| {
                with_active_stores(stores, || {
                    let entrance = test_object(&format!("TurnCaveEntrance{tag}"), ENTRANCE);
                    let rider = test_object(&format!("TurnCaveRider{tag}"), RIDER);
                    let mut data = CaveContainModuleData::default();
                    data.base.allow_neutral_inside = true;
                    let mut cave = CaveContain::new(
                        Arc::downgrade(&entrance),
                        ModuleDataHandle::new(ContainModuleDataAdapter::new(data)),
                        crate::object::ObjectWorld::of(stores.services()),
                    )
                    .expect("cave");
                    ContainModuleInterface::on_owner_created(&mut cave).expect("owner created");
                    (entrance, rider, cave)
                })
            };
            let (_entrance_a, rider_a, mut cave_a) = build(&world_a, "A");
            let (_entrance_b, rider_b, mut cave_b) = build(&world_b, "B");
            let contained_by = |rider: &Arc<RwLock<Object>>| rider.read().expect("rider").get_contained_by();

            // World A's entrance, driven while world B is published.
            with_active_stores(&world_b, || {
                ContainModuleInterface::contain_object(&mut cave_a, RIDER).expect("enter A");
            });
            assert_eq!(contained_by(&rider_a), Some(ENTRANCE), "world A's rider entered A's cave");
            assert_eq!(contained_by(&rider_b), None, "world B's same-id rider was not touched");
            assert_eq!(
                ContainModuleInterface::get_contained_objects(&cave_a).into_owned(),
                vec![RIDER]
            );
            assert!(ContainModuleInterface::get_contained_objects(&cave_b).is_empty());

            // World B's entrance, driven while world A is published.
            with_active_stores(&world_a, || {
                ContainModuleInterface::contain_object(&mut cave_b, RIDER).expect("enter B");
            });
            assert_eq!(contained_by(&rider_b), Some(ENTRANCE));

            // Releasing A's rider under B's publication leaves B's rider inside.
            with_active_stores(&world_b, || {
                ContainModuleInterface::release_object(&mut cave_a, RIDER).expect("exit A");
            });
            assert_eq!(contained_by(&rider_a), None, "world A's rider left A's cave");
            assert_eq!(contained_by(&rider_b), Some(ENTRANCE), "world B's rider stayed inside");
            assert_eq!(
                ContainModuleInterface::get_contained_objects(&cave_b).into_owned(),
                vec![RIDER]
            );

            for stores in [&world_a, &world_b] {
                with_active_stores(stores, || {
                    OBJECT_REGISTRY.unregister_object(ENTRANCE);
                    OBJECT_REGISTRY.unregister_object(RIDER);
                });
            }
        });
    }

    /// C++ CaveContain::xfer resolves m_originalTeam with TheTeamFactory of
    /// the engine that owns the cave. Two worlds take turns on ONE thread and
    /// both hold a team with the same id: each cave restores its own world's
    /// team even while the other world is published, and never the
    /// process-wide factory.
    #[test]
    fn cave_original_team_resolves_on_its_own_world_factory_when_worlds_take_turns() {
        use crate::system::engine_stores::{with_active_stores, EngineStores};
        let _lock = crate::test_sync::lock();
        let common = Arc::new(game_engine::common::common_services::CommonServices::new());
        game_engine::common::common_services::with_common_services(&common, || {
            let world_a = Arc::new(EngineStores::new_for_world());
            let world_b = Arc::new(EngineStores::new_for_world());
            const ENTRANCE: ObjectID = 93701;
            const TEAM: TeamID = 0x7A11;
            let build = |stores: &Arc<EngineStores>, tag: &str| {
                let prototype = crate::team::TeamPrototype::new(format!("CaveTeam{tag}").into());
                stores
                    .services()
                    .team_factory()
                    .lock()
                    .expect("world factory")
                    .create_team_on_prototype_with_id(&prototype, TEAM)
                    .expect("world team");
                with_active_stores(stores, || {
                    let entrance = test_object(&format!("TeamCaveEntrance{tag}"), ENTRANCE);
                    let cave = CaveContain::new(
                        Arc::downgrade(&entrance),
                        ModuleDataHandle::new(ContainModuleDataAdapter::new(
                            CaveContainModuleData::default(),
                        )),
                        crate::object::ObjectWorld::of(stores.services()),
                    )
                    .expect("cave");
                    (entrance, cave)
                })
            };
            let (_entrance_a, mut cave_a) = build(&world_a, "A");
            let (_entrance_b, mut cave_b) = build(&world_b, "B");
            let original_name = |cave: &CaveContain| {
                cave.original_team
                    .as_ref()
                    .and_then(|team| team.upgrade())
                    .map(|team| team.read().expect("team").get_name().to_string())
            };

            with_active_stores(&world_b, || cave_a.restore_original_team_by_id(TEAM))
                .expect("A restores under B");
            with_active_stores(&world_a, || cave_b.restore_original_team_by_id(TEAM))
                .expect("B restores under A");
            assert_eq!(original_name(&cave_a).as_deref(), Some("CaveTeamA"));
            assert_eq!(original_name(&cave_b).as_deref(), Some("CaveTeamB"));
            assert!(
                crate::team::TheTeamFactory().lock().expect("process factory").find_team_by_id(TEAM).is_none(),
                "world teams never land in the process-wide factory"
            );

            for stores in [&world_a, &world_b] {
                with_active_stores(stores, || {
                    OBJECT_REGISTRY.unregister_object(ENTRANCE);
                });
            }
        });
    }

    /// C++ TunnelTracker::getContainMax returns TheGlobalData->m_maxTunnelCapacity
    /// of the engine that owns the tunnel (TunnelTracker.cpp:81-84). Two worlds
    /// with different GameData take turns on ONE thread: each cave reports and
    /// admits by its own world's capacity, never the published world's.
    #[test]
    fn cave_capacity_reads_its_own_world_gamedata_when_worlds_take_turns() {
        use crate::system::engine_stores::{with_active_stores, EngineStores};
        let _lock = crate::test_sync::lock();
        let world_a = Arc::new(EngineStores::new_for_world());
        let world_b = Arc::new(EngineStores::new_for_world());
        const ENTRANCE: ObjectID = 93801;
        const RIDER: ObjectID = 93802;
        let build = |stores: &Arc<EngineStores>, tag: &str, capacity: i32| {
            let mut data = game_engine::common::ini::ini_game_data::GlobalData::new();
            data.max_tunnel_capacity = capacity;
            *stores.services().common().system.global_data.lock().expect("gamedata") =
                Some(Arc::new(parking_lot::RwLock::new(data)));
            with_active_stores(stores, || {
                let entrance = test_object(&format!("CapacityCave{tag}"), ENTRANCE);
                let rider = test_object(&format!("CapacityRider{tag}"), RIDER);
                let mut data = CaveContainModuleData::default();
                data.base.allow_neutral_inside = true;
                let mut cave = CaveContain::new(
                    Arc::downgrade(&entrance),
                    ModuleDataHandle::new(ContainModuleDataAdapter::new(data)),
                    crate::object::ObjectWorld::of(stores.services()),
                )
                .expect("cave");
                ContainModuleInterface::on_owner_created(&mut cave).expect("owner created");
                (entrance, rider, cave)
            })
        };
        let (_entrance_a, rider_a, cave_a) = build(&world_a, "A", 1);
        let (_entrance_b, rider_b, cave_b) = build(&world_b, "B", 0);

        with_active_stores(&world_b, || {
            assert_eq!(cave_a.get_contain_max().expect("A max"), 1, "A reads A's GameData");
            let rider = rider_a.read().expect("rider A");
            assert!(cave_a.is_valid_container_for(&rider, true).expect("A admits"));
        });
        with_active_stores(&world_a, || {
            assert_eq!(cave_b.get_contain_max().expect("B max"), 0, "B reads B's GameData");
            let rider = rider_b.read().expect("rider B");
            assert!(
                !cave_b.is_valid_container_for(&rider, true).expect("B rejects"),
                "C++ capacity 0 admits nobody"
            );
        });

        for stores in [&world_a, &world_b] {
            with_active_stores(stores, || {
                OBJECT_REGISTRY.unregister_object(ENTRANCE);
                OBJECT_REGISTRY.unregister_object(RIDER);
            });
        }
    }

    #[test]
    fn parallel_cave_worlds_match_single_instance_runs() {
        let _lock = crate::test_sync::lock();
        // Same entrance ids and an overlapping rider id, different contents.
        let riders_a: &[ObjectID] = &[93410, 93411];
        let riders_b: &[ObjectID] = &[93410, 93412, 93413];
        let solo_a = run_cave_world(riders_a, None);
        let solo_b = run_cave_world(riders_b, None);
        assert_eq!(solo_a.after_enter, riders_a);
        assert_eq!(solo_a.after_exit, &riders_a[1..]);
        assert_eq!(solo_a.indices_after_switch, vec![0, 1]);
        assert!(solo_a.entrances_after_death.is_empty());
        assert_eq!(solo_a.restored_switched_entrances, vec![93402]);
        assert!(solo_a.indices_after_reset.is_empty());
        assert_ne!(solo_a, solo_b);

        let gate = Arc::new(std::sync::Barrier::new(2));
        let spawn = |riders: &'static [ObjectID]| {
            let gate = Arc::clone(&gate);
            std::thread::spawn(move || run_cave_world(riders, Some(&gate)))
        };
        let thread_a = spawn(&[93410, 93411]);
        let thread_b = spawn(&[93410, 93412, 93413]);
        let parallel_a = thread_a.join().expect("world A");
        let parallel_b = thread_b.join().expect("world B");
        assert_eq!(parallel_a, solo_a, "world A diverged when run beside world B");
        assert_eq!(parallel_b, solo_b, "world B diverged when run beside world A");
    }
}
