//! Cave Contain Module
//!
//! A version of OpenContain that overrides where the passengers are stored: one of CaveManager's
//! entries. Changing entry is a script or ini command. All queries about capacity and
//! contents are also redirected.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface, OpenContain};
use crate::common::{GameError, GameResult, ObjectID, PlayerMaskType};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::modules::{ContainModuleInterface, ContainWant, UpdateSleepTime};
use crate::object::drawable::Drawable;
use crate::object::{Object, ObjectId};
use crate::player::Player;
use crate::system::cave_system::CaveSystem;
use crate::system::game_logic::GameLogic;
use crate::team::Team;
use crate::tunnel_tracker::TunnelTracker;
use game_engine::common::ini::{FieldParse, INIError, INI};

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
    /// Whether we need to run onBuildComplete
    need_to_run_on_build_complete: bool,
    /// Cave index for this container
    cave_index: i32,
    /// Original team before garrison
    original_team: Option<Weak<RwLock<Team>>>,
    /// Reference to the owning object
    object: Weak<RwLock<Object>>,
    /// Reference to cave system
    cave_system: Option<Arc<Mutex<CaveSystem>>>,
}

impl CaveContain {
    /// Create a new CaveContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &CaveContainModuleData,
        cave_system: Option<Arc<Mutex<CaveSystem>>>,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object.clone(), &module_data.base)?;

        Ok(Self {
            base,
            need_to_run_on_build_complete: true,
            cave_index: 0,
            original_team: None,
            object,
            cave_system,
        })
    }

    /// Get the object this module belongs to
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
    pub fn on_containing(
        &mut self,
        obj: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> GameResult<()> {
        self.base.on_containing(obj.clone(), was_selected)?;

        // Objects inside a building are held
        if let Ok(mut object) = obj.write() {
            object.set_disabled_held(true)?;
        }

        // Recalculate apparent controlling player
        self.recalc_apparent_controlling_player()?;

        Ok(())
    }

    /// Called when removing an object from containment
    pub fn on_removing(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.base.on_removing(obj.clone())?;

        // Object is no longer held inside a garrisoned building
        if let Ok(mut object) = obj.write() {
            object.set_disabled_held(false)?;
        }

        // Register object in partition manager and set position
        if let Some(owner_obj) = self.get_object() {
            if let (Ok(owner), Ok(mut contained)) = (owner_obj.read(), obj.write()) {
                contained.register_in_partition_manager()?;
                if let Err(err) = contained.set_position(owner.get_position()) {
                    log::warn!(
                        "CaveContain::on_removing failed to place contained object {}: {}",
                        contained.get_id(),
                        err
                    );
                }

                if let Some(drawable) = contained.get_drawable() {
                    if let Ok(mut draw) = drawable.write() {
                        draw.set_drawable_hidden(false)?;
                    }
                }
            }
        }

        self.do_unload_sound()?;

        // If no more units contained, revert to original team
        if self.get_contain_count()? == 0 {
            if let Some(owner_obj) = self.get_object() {
                if let Ok(owner) = owner_obj.read() {
                    if owner.get_team().is_some() {
                        self.change_team_on_all_connected_caves(self.original_team.clone(), false)?;
                        self.original_team = None;
                    }
                }
            }

            // Clear garrisoned model condition
            if let Some(owner_obj) = self.get_object() {
                if let Ok(owner) = owner_obj.read() {
                    if let Some(drawable) = owner.get_drawable() {
                        if let Ok(mut draw) = drawable.write() {
                            draw.clear_model_condition_garrisoned()?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if this container is valid for the given object
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> GameResult<bool> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
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
    pub fn add_to_contain_list(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(mut tunnel) = tracker.write() {
            tunnel.add_to_contain_list(obj.clone())?;
        }
        let _ = self.base.add_to_contain_list(obj);
        Ok(())
    }

    /// Remove object from contain list
    pub fn remove_from_contain(
        &mut self,
        obj: Arc<RwLock<Object>>,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(mut tunnel) = tracker.write() {
            if !tunnel.is_in_container(&obj)? {
                return Ok(());
            }

            tunnel.remove_from_contain(obj.clone(), expose_stealth_units)?;
        }

        // Trigger events
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if let Some(contain) = owner.get_contain() {
                    // Note: This would need the actual contain interface
                    // contain.on_removing(obj.clone())?;
                }
            }
        }

        if let Ok(mut contained) = obj.write() {
            if let Some(owner_obj) = self.get_object() {
                contained.on_removed_from(owner_obj)?;
            }
            self.base.remove_from_contain_list(contained.get_id());
        }

        Ok(())
    }

    /// Remove all contained objects
    pub fn remove_all_contained(&mut self, expose_stealth_units: bool) -> GameResult<()> {
        // Extract the full list first before calling remove_from_contain
        let full_list = if let Some(cave_system) = &self.cave_system {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
            let tracker = system.get_tunnel_tracker_for_cave_index(self.cave_index)?;
            let tunnel = tracker.read().map_err(|_| GameError::LockError)?;
            tunnel.get_contained_items_list().to_vec()
        } else {
            return Ok(());
        };

        // Now that the lock is released, iterate over the list
        for obj in full_list {
            self.remove_from_contain(obj, expose_stealth_units)?;
        }

        Ok(())
    }

    /// Iterate contained objects with callback
    pub fn iterate_contained<F>(&self, mut func: F, reverse: bool) -> GameResult<()>
    where
        F: FnMut(Arc<RwLock<Object>>) -> GameResult<()>,
    {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
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
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
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
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
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
    pub fn get_contained_items_list(&self) -> GameResult<Vec<Arc<RwLock<Object>>>> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(Vec::new());
        };

        if let Ok(tunnel) = tracker.read() {
            return Ok(tunnel.get_contained_items_list().clone());
        }
        Ok(Vec::new())
    }

    /// Check if should kick out on capture (caves don't)
    pub fn is_kick_out_on_capture(&self) -> bool {
        false // Caves and Tunnels don't kick out on capture
    }

    /// Handle death event
    pub fn on_die(&mut self, damage_info: Option<&DamageInfo>) -> GameResult<()> {
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if owner.is_under_construction() {
                    return Ok(()); // Never registered itself as a tunnel
                }
            }
        }

        let tracker = if let Some(cave_system) = &self.cave_system {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.unregister_cave(self.cave_index)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(mut tunnel) = tracker.write() {
            if let Some(owner_obj) = self.get_object() {
                if let Ok(owner) = owner_obj.read() {
                    tunnel.on_tunnel_destroyed(&*owner)?;
                }
            }
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

        let tracker = if let Some(cave_system) = &self.cave_system {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.register_new_cave(self.cave_index)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(mut tunnel) = tracker.write() {
            if let Some(owner_obj) = self.get_object() {
                if let Ok(owner) = owner_obj.read() {
                    tunnel.on_tunnel_created(&*owner)?;
                }
            }
        }

        Ok(())
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
            if let Some(owner_obj) = self.get_object() {
                if let Ok(owner) = owner_obj.read() {
                    tunnel.on_tunnel_destroyed(&*owner)?;
                }
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
            if let Some(owner_obj) = self.get_object() {
                if let Ok(owner) = owner_obj.read() {
                    tunnel.on_tunnel_created(&*owner)?;
                }
            }
        }

        Ok(())
    }

    /// Set the original team (used for distributed garrison)
    pub fn set_original_team(&mut self, old_team: Option<Weak<RwLock<Team>>>) {
        self.original_team = old_team;
    }

    /// Recalculate apparent controlling player
    pub fn recalc_apparent_controlling_player(&mut self) -> GameResult<()> {
        // Record original team first time through
        if self.original_team.is_none() {
            if let Some(owner_obj) = self.get_object() {
                if let Ok(owner) = owner_obj.read() {
                    self.original_team = owner.get_team().map(|t| Arc::downgrade(&t));
                }
            }
        }

        // Check if team is null (game teardown)
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if owner.get_team().is_none() {
                    self.original_team = None;
                }
            }
        }

        // Edge trigger on count == 1 to do capture stuff
        if self.get_contain_count()? == 1 {
            let contained_list = self.get_contained_items_list()?;
            if let Some(rider) = contained_list.first() {
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
        } else if self.get_contain_count()? == 0 {
            // Edge trigger on count == 0 to do uncapture stuff
            self.change_team_on_all_connected_caves(self.original_team.clone(), false)?;
        }

        // Handle team color rendering
        // Note: This would need access to global data and player list
        // Implementation would depend on how these are managed in Rust version

        Ok(())
    }

    /// Change team on all connected caves (distributed garrison)
    pub fn change_team_on_all_connected_caves(
        &mut self,
        new_team: Option<Weak<RwLock<Team>>>,
        set_original_teams: bool,
    ) -> GameResult<()> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let mut system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(tunnel) = tracker.read() {
            let all_caves = tunnel.get_container_list()?;

            for cave_id in all_caves {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(cave_id) else {
                    continue;
                };
                let Ok(mut obj_guard) = obj_arc.write() else {
                    continue;
                };

                let team_arc = new_team.as_ref().and_then(|weak| weak.upgrade());
                let _ = if set_original_teams {
                    obj_guard.set_team(team_arc)
                } else {
                    obj_guard.set_temporary_team(team_arc)
                };
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
        // Implementation would depend on team ID system

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

        // Load original team would require team lookup system

        Ok(())
    }
}

impl ContainModuleInterface for CaveContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj.read() {
                return self
                    .is_valid_container_for(&*obj_guard, true)
                    .unwrap_or(false);
            }
        }
        false
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = TheGameLogic::find_object_by_id(object_id)
            .ok_or_else(|| format!("Contain object {} not found", object_id))?;
        self.add_to_contain_list(obj).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = match TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return Ok(()),
        };
        self.remove_from_contain(obj, true)
            .map_err(|e| e.to_string())
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        ContainModuleInterface::get_contained_objects(&self.base)
    }

    fn get_contained_count(&self) -> usize {
        let (current, _) = self.get_usage();
        current as usize
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let (_, max) = self.get_usage();
        if max == u32::MAX {
            usize::MAX
        } else {
            max as usize
        }
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

    fn is_kick_out_on_capture(&self) -> bool {
        CaveContain::is_kick_out_on_capture(self)
    }
}

impl ContainerInterface for CaveContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.is_valid_container_for(obj, true).unwrap_or(false)
    }

    fn add_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.base.add_to_contain(obj.clone())?;
        self.on_containing(obj, false)
    }

    fn remove_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.on_removing(obj.clone())?;
        self.base.remove_from_contain(obj, true)
    }

    fn get_usage(&self) -> (u32, u32) {
        let current = self.base.get_contain_count();
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
