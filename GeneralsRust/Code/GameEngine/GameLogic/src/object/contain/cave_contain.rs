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
use crate::helpers::{TheGameLogic, TheGlobalData};
use crate::modules::{ContainModuleInterface, ContainWant, UpdateSleepTime};
use crate::object::drawable::Drawable;
use crate::object::{Object, ObjectId};
use crate::player::{Player, ThePlayerList};
use crate::system::cave_system::CaveSystem;
use crate::system::game_logic::GameLogic;
use crate::team::{Team, TeamID, TheTeamFactory, TEAM_ID_INVALID};
use crate::tunnel_tracker::TunnelTracker;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

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
            contained_object_ids: Vec::new(),
            object_id: object
                .upgrade()
                .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
                .unwrap_or(crate::common::INVALID_ID),
            cave_system,
        })
    }

    /// Get the object this module belongs to
    pub fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        })
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
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.base.on_containing(obj_id, was_selected)?;

        // Objects inside a building are held
        if let Ok(mut object) = obj.write() {
            object.set_disabled_held(true)?;
        }

        // Recalculate apparent controlling player
        self.recalc_apparent_controlling_player()?;

        Ok(())
    }

    /// Called when removing an object from containment
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.base.on_removing(obj_id)?;

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
    pub fn add_to_contain_list(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        let obj_id = obj.read().map_err(|_| GameError::LockError)?.get_id();
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(mut tunnel) = tracker.write() {
            tunnel.add_to_contain_list(obj.clone())?;
        }
        if !self.contained_object_ids.contains(&obj_id) {
            self.contained_object_ids.push(obj_id);
        }
        Ok(())
    }

    /// Add object to containment using CaveContain's tracker-backed storage.
    pub fn add_to_contain(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        let owner = self.get_object();
        if super::should_cancel_containment_after_booby_trap(owner.as_ref(), &obj) {
            return Ok(());
        }

        let was_selected = obj
            .read()
            .ok()
            .and_then(|guard| guard.get_drawable())
            .and_then(|drawable| drawable.read().ok().map(|draw| draw.is_selected()))
            .unwrap_or(false);

        {
            let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
            if !self.is_valid_container_for(&obj_guard, true)? {
                return Err("Object not valid for this cave container".into());
            }
            if obj_guard.get_contained_by().is_some() {
                return Ok(());
            }
        }

        self.add_to_contain_list(obj.clone())?;

        let is_enclosing = obj
            .read()
            .map(|obj_guard| self.base.is_enclosing_container_for(&obj_guard))
            .unwrap_or(false);
        if is_enclosing {
            let _ = self.base.add_or_remove_obj_from_world(obj.clone(), false);
        }

        let contained = self.get_contained_items_list()?;
        self.base.redeploy_objects(&contained)?;
        self.on_containing(obj.read().map(|g| g.get_id()).unwrap_or(0), was_selected)?;
        Ok(())
    }

    /// Remove object from contain list
    pub fn remove_from_contain(
        &mut self,
        obj: Arc<RwLock<Object>>,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
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
        if let Ok(guard) = obj.read() {
            self.contained_object_ids.retain(|id| *id != guard.get_id());
        }

        self.on_removing(obj.read().map(|g| g.get_id()).unwrap_or(0))?;

        Ok(())
    }

    /// Remove all contained objects
    pub fn remove_all_contained(&mut self, expose_stealth_units: bool) -> GameResult<()> {
        // Extract the full list first before calling remove_from_contain
        let full_list = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
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
    pub fn iterate_contained<F>(&self, func: F, reverse: bool) -> GameResult<()>
    where
        F: FnMut(Arc<RwLock<Object>>) -> GameResult<()>,
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
    pub fn get_contained_items_list(&self) -> GameResult<Vec<Arc<RwLock<Object>>>> {
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
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
        let Some(damage_info) = damage_info else {
            return Ok(());
        };

        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if !self.base.is_die_applicable(&*owner, damage_info) {
                    return Ok(());
                }

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

        let factory = TheTeamFactory().lock().map_err(|e| e.to_string())?;
        let team = factory
            .find_team_by_id(team_id)
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
        self.get_object()
            .and_then(|owner| owner.read().ok()?.get_controlling_player())
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

        // Handle the team color that is rendered.
        let has_local_player = {
            ThePlayerList()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .is_some()
        };
        if has_local_player {
            if let Some(controller) = self.get_apparent_controlling_player(None) {
                if let Ok(controller_guard) = controller.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|global| global.get_time_of_day())
                        .unwrap_or(crate::common::audio::TimeOfDay::Day);
                    let color = match time_of_day {
                        crate::common::audio::TimeOfDay::Night => {
                            controller_guard.get_player_night_color()
                        }
                        _ => controller_guard.get_player_color(),
                    };
                    if let Some(owner_obj) = self.get_object() {
                        if let Ok(owner) = owner_obj.read() {
                            if let Some(drawable) = owner.get_drawable() {
                                if let Ok(mut draw_guard) = drawable.write() {
                                    draw_guard.set_indicator_color(color);
                                }
                            }
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
        let tracker = if let Some(cave_system) = &self.cave_system {
            let system = cave_system.lock().map_err(|_| GameError::LockError)?;
            system.get_tunnel_tracker_for_cave_index(self.cave_index)?
        } else {
            return Ok(());
        };

        if let Ok(tunnel) = tracker.read() {
            let all_caves = tunnel.get_container_list()?;
            let team_arc = new_team.as_ref().and_then(|weak| weak.upgrade());

            for cave_id in all_caves {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(cave_id) else {
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

        let mut team_id = self.original_team_id();
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
        self.add_to_contain(obj).map_err(|e| e.to_string())
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
        &self.contained_object_ids
    }

    fn get_contained_count(&self) -> usize {
        CaveContain::get_contain_count(self).unwrap_or(0) as usize
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let max = CaveContain::get_contain_max(self).unwrap_or(self.base.get_contain_max());
        if max <= 0 {
            usize::MAX
        } else {
            max as usize
        }
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

    fn is_kick_out_on_capture(&self) -> bool {
        CaveContain::is_kick_out_on_capture(self)
    }
}

impl ContainerInterface for CaveContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.is_valid_container_for(obj, true).unwrap_or(false)
    }

    fn add_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.add_to_contain(obj)
    }

    fn remove_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.remove_from_contain(obj, true)
    }

    fn get_usage(&self) -> (u32, u32) {
        let current = self.get_contain_count().unwrap_or(0);
        let max_raw = self
            .get_contain_max()
            .unwrap_or_else(|_| self.base.get_contain_max());
        let max = match max_raw {
            super::CONTAIN_MAX_UNKNOWN => u32::MAX,
            value if value <= 0 => u32::MAX,
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
        set_death_type_flag, DamageInfo, DamageType, DeathType, DEATH_TYPE_FLAGS_NONE,
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

        fn get_contained_objects(&self) -> &[ObjectID] {
            &[]
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
            CaveContain::new(Arc::downgrade(owner), &data, Some(Arc::clone(&cave_system)))
                .expect("cave contain");
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
        assert_eq!(ContainModuleInterface::get_contained_count(&cave), 1);
        assert_eq!(
            ContainModuleInterface::get_contained_objects(&cave),
            &[93002]
        );
        assert_eq!(
            passenger.read().expect("passenger read").get_contained_by(),
            Some(93001)
        );
        assert!(ContainModuleInterface::is_bustable(&cave));

        OBJECT_REGISTRY.unregister_object(93001);
        OBJECT_REGISTRY.unregister_object(93002);
    }

    #[test]
    fn container_interface_usage_reports_tracker_state_like_cpp() {
        let _lock = crate::test_sync::lock();
        let owner = test_object("CaveUsageOwner", 93003);
        let passenger = test_object("CaveUsagePassenger", 93004);
        let (mut cave, _cave_system) = cave_with_registered_tracker(&owner, 0);

        ContainerInterface::add_object(&mut cave, passenger.clone()).expect("add object");

        assert_eq!(ContainerInterface::get_usage(&cave), (1, u32::MAX));
        ContainerInterface::remove_object(&mut cave, passenger.clone()).expect("remove object");
        assert_eq!(ContainerInterface::get_usage(&cave), (0, u32::MAX));
        assert_eq!(
            passenger.read().expect("passenger read").get_contained_by(),
            None
        );

        OBJECT_REGISTRY.unregister_object(93003);
        OBJECT_REGISTRY.unregister_object(93004);
    }

    #[test]
    fn connected_cave_team_change_updates_each_cave_original_team_like_cpp() {
        let _lock = crate::test_sync::lock();
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

        let calls_a = Arc::new(Mutex::new(Vec::new()));
        let calls_b = Arc::new(Mutex::new(Vec::new()));
        cave_a
            .write()
            .expect("cave a write")
            .set_contain(Some(Arc::new(Mutex::new(RecordingContain {
                original_team_calls: Arc::clone(&calls_a),
            }))));
        cave_b
            .write()
            .expect("cave b write")
            .set_contain(Some(Arc::new(Mutex::new(RecordingContain {
                original_team_calls: Arc::clone(&calls_b),
            }))));

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
            .on_tunnel_created(&*owner.read().expect("owner read"))
            .expect("register owner tunnel");
        cave.add_to_contain_list(passenger.clone())
            .expect("add passenger to tracker");
        assert_eq!(
            ContainModuleInterface::get_contained_objects(&cave),
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
        assert!(tracker
            .read()
            .expect("tracker read")
            .get_container_list()
            .expect("container list")
            .is_empty());
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
        let team = Arc::new(RwLock::new(Team::new("CaveColorTeam".into(), 9300)));
        team.write()
            .expect("team write")
            .set_controlling_player_id(Some(0));
        let owner = test_object_with_team("CaveColorOwner", 93008, Arc::clone(&team));
        let owner_drawable = attach_drawable(&owner, 930080);
        let data = CaveContainModuleData::default();
        let mut cave = CaveContain::new(Arc::downgrade(&owner), &data, None).expect("cave contain");
        cave.on_create(&data).expect("on create");

        let player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut player_guard = player.write().expect("player write");
            player_guard.set_default_team(Some(Arc::clone(&team)));
            player_guard.set_colors(player_color, night_color);
        }
        {
            let mut list = ThePlayerList().write().expect("player list write");
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
    }
}
