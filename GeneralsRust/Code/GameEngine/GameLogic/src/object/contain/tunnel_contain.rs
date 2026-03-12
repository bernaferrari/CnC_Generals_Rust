//! Tunnel Contain Module - Rust port of C++ TunnelContain
//!
//! A version of OpenContain that stores passengers in the owning Player's TunnelTracker.
//! All queries about capacity and contents are redirected to the shared tunnel network.
//! Author: Graham Smallwood, March 2002 (C++ version)
//! Rust conversion: 2025
//!
//! Matches C++ TunnelContain.cpp from GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/

use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface};
use crate::common::{Coord3D, GameResult, PlayerMaskType};
use crate::damage::DamageInfo;
use crate::helpers::get_game_logic_random_value_real;
use crate::helpers::TheGameLogic;
use crate::modules::{
    ContainModuleInterface, ContainModuleInterfaceExt, ContainWant, DISABLED_HELD,
};
use crate::object::contain::OpenContain;
use crate::object::{Object, ObjectID, INVALID_ID};
use crate::terrain::THE_TERRAIN_LOGIC;
use game_engine::common::ini::{FieldParse, INIError, INI};

/// Configuration data for TunnelContain module
#[derive(Debug, Clone)]
pub struct TunnelContainModuleData {
    /// Configuration from parent OpenContain
    pub base: super::OpenContainModuleData,
    /// Time in frames for something to become fully healed
    pub frames_for_full_heal: f32,
}

impl TunnelContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, TUNNEL_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, TUNNEL_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for TunnelContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        TunnelContainModuleData::parse_from_config(self, config)
    }
}

impl Default for TunnelContainModuleData {
    fn default() -> Self {
        let mut base = super::OpenContainModuleData::default();
        base.allow_inside_kind_of = 1u64 << (crate::common::KindOf::Infantry as u32);

        Self {
            base,
            frames_for_full_heal: 1.0,
        }
    }
}

fn parse_time_for_full_heal(
    _ini: &mut INI,
    data: &mut TunnelContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.frames_for_full_heal = super::parse_duration_frames_real(token)?;
    Ok(())
}

const TUNNEL_CONTAIN_FIELDS: &[FieldParse<TunnelContainModuleData>] = &[FieldParse {
    token: "TimeForFullHeal",
    parse: parse_time_for_full_heal,
}];

/// Tunnel contain module - stores passengers in the player's shared tunnel network
#[derive(Debug)]
pub struct TunnelContain {
    /// Base functionality from OpenContain
    pub base: OpenContain,
    /// Reference to the owning object (the tunnel entrance)
    object: Weak<RwLock<Object>>,
    /// Whether we need to run onBuildComplete logic
    need_to_run_on_build_complete: bool,
    /// Whether this tunnel is currently registered with the TunnelTracker
    is_currently_registered: bool,
}

impl TunnelContain {
    /// Create a new TunnelContain module.
    /// Matches C++ TunnelContain::TunnelContain (TunnelContain.cpp:34-38)
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &TunnelContainModuleData,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object.clone(), &module_data.base)?;

        Ok(Self {
            base,
            object,
            need_to_run_on_build_complete: true,
            is_currently_registered: false,
        })
    }

    /// Check if this is a tunnel container
    pub fn is_tunnel_contain(&self) -> bool {
        true
    }

    /// Add an object to the tunnel network contain list.
    /// Matches C++ TunnelContain::addToContainList (TunnelContain.cpp:46-50)
    pub fn add_to_contain_list(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        let owner = self.get_object()?;
        let owner_read = owner.read().map_err(|_| "Owner lock poisoned")?;

        if let Some(controlling_player) = owner_read.get_controlling_player() {
            let mut player_guard = controlling_player
                .write()
                .map_err(|_| "Player lock poisoned")?;
            if let Some(tunnel_system) = player_guard.get_tunnel_system_mut() {
                tunnel_system.add_to_contain_list(obj.clone())?;
            }
        }

        let _ = self.base.add_to_contain_list(obj);

        Ok(())
    }

    /// Remove object from tunnel network.
    /// Matches C++ TunnelContain::removeFromContain (TunnelContain.cpp:57-88)
    pub fn remove_from_contain(
        &mut self,
        obj: Arc<RwLock<Object>>,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        let owner = self.get_object()?;
        let owner_read = owner.read().map_err(|_| "Owner lock poisoned")?;

        // Trigger onRemoving event for the container
        if let Some(contain) = owner_read.get_contain() {
            let obj_read = obj.read().map_err(|_| "Object lock poisoned")?;
            contain.on_removing(&*obj_read);
        }

        // Trigger onRemovedFrom event for the object being removed
        {
            let mut obj_write = obj.write().map_err(|_| "Object lock poisoned")?;
            obj_write.on_removed_from(owner.clone())?;
        }

        // Remove from tunnel network if still valid
        if let Some(controlling_player) = owner_read.get_controlling_player() {
            let player_read = controlling_player
                .read()
                .map_err(|_| "Player lock poisoned")?;

            if let Some(tunnel_system) = player_read.get_tunnel_system() {
                if !tunnel_system.is_in_container(&obj)? {
                    return Ok(());
                }
            }

            drop(player_read);
            let mut player_write = controlling_player
                .write()
                .map_err(|_| "Player lock poisoned")?;
            if let Some(tunnel_system_mut) = player_write.get_tunnel_system_mut() {
                tunnel_system_mut.remove_from_contain(obj.clone(), expose_stealth_units)?;
            }
        }

        if let Ok(obj_read) = obj.read() {
            self.base.remove_from_contain_list(obj_read.get_id());
        }

        Ok(())
    }

    /// Force all contained objects to exit and damage them.
    /// Matches C++ TunnelContain::harmAndForceExitAllContained (TunnelContain.cpp:95-120)
    pub fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> GameResult<()> {
        let owner = self.get_object()?;
        let owner_read = owner.read().map_err(|_| "Owner lock poisoned")?;

        if let Some(controlling_player) = owner_read.get_controlling_player() {
            let player_read = controlling_player
                .read()
                .map_err(|_| "Player lock poisoned")?;
            let objects: Vec<_> = if let Some(tunnel_system) = player_read.get_tunnel_system() {
                tunnel_system
                    .get_contained_items_list()
                    .iter()
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
            drop(player_read);
            drop(owner_read);

            // Iterate from beginning after each loop to handle cascade deletions
            // (Matches C++ Patch 1.01 fix - November 6, 2003, lines 103-111)
            while let Some(obj) = objects.first().cloned() {
                self.remove_from_contain(obj.clone(), true)?;
                let mut obj_write = obj.write().map_err(|_| "Object lock poisoned")?;
                obj_write.attempt_damage(damage_info)?;
            }
        }

        Ok(())
    }

    /// Kill all contained objects.
    /// Matches C++ TunnelContain::killAllContained (TunnelContain.cpp:126-141)
    pub fn kill_all_contained(&mut self) -> GameResult<()> {
        let owner = self.get_object()?;
        let owner_read = owner.read().map_err(|_| "Owner lock poisoned")?;

        if let Some(controlling_player) = owner_read.get_controlling_player() {
            let player_read = controlling_player
                .read()
                .map_err(|_| "Player lock poisoned")?;
            let objects: Vec<_> = if let Some(tunnel_system) = player_read.get_tunnel_system() {
                tunnel_system
                    .get_contained_items_list()
                    .iter()
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
            drop(player_read);
            drop(owner_read);

            for obj in objects {
                self.remove_from_contain(obj.clone(), true)?;
                let mut obj_write = obj.write().map_err(|_| "Object lock poisoned")?;
                obj_write.kill(None, None);
            }
        }

        Ok(())
    }

    /// Called when an object enters the tunnel.
    /// Matches C++ TunnelContain::onContaining (TunnelContain.cpp:171-186)
    pub fn on_containing(
        &mut self,
        obj: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> GameResult<()> {
        self.base.on_containing(obj.clone(), was_selected)?;

        let mut obj_guard = obj.write().map_err(|_| "Object lock poisoned")?;

        // Objects inside tunnels are held (disabled)
        obj_guard.set_disabled(DISABLED_HELD);

        // Record academy stats
        if let Some(controlling_player) = obj_guard.get_controlling_player() {
            let mut player_write = controlling_player
                .write()
                .map_err(|_| "Player lock poisoned")?;
            player_write
                .get_academy_stats_mut()
                .record_unit_entered_tunnel_network();
        }

        // Handle partition cell maintenance
        obj_guard.handle_partition_cell_maintenance();

        Ok(())
    }

    /// Called when an object exits the tunnel.
    /// Matches C++ TunnelContain::onRemoving (TunnelContain.cpp:189-208)
    pub fn on_removing(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.base.on_removing(obj.clone())?;

        let mut obj_guard = obj.write().map_err(|_| "Object lock poisoned")?;

        // Object is no longer held
        obj_guard.clear_disabled(DISABLED_HELD);

        // Partition registration would happen here when the manager is fully wired.

        // Place object at container position
        let owner = self.get_object()?;
        let owner_read = owner.read().map_err(|_| "Owner lock poisoned")?;
        let position = *owner_read.get_position();
        obj_guard.set_position(&position)?;

        // Show drawable
        if let Some(drawable) = obj_guard.get_drawable() {
            let current_frame = get_current_frame()?;
            let occlusion_delay = obj_guard.get_template().get_occlusion_delay();
            obj_guard.set_safe_occlusion_frame(current_frame + occlusion_delay);

            let mut drawable_write = drawable.write().map_err(|_| "Drawable lock poisoned")?;
            if let Err(err) = drawable_write.set_drawable_hidden(false) {
                log::warn!(
                    "TunnelContain::on_removing failed to unhide drawable for object {}: {}",
                    obj_guard.get_id(),
                    err
                );
            }
        }

        // Play unload sound
        self.base.do_unload_sound();

        Ok(())
    }

    /// Handle selling the tunnel entrance.
    /// Matches C++ TunnelContain::onSelling (TunnelContain.cpp:211-234)
    pub fn on_selling(&mut self) -> GameResult<()> {
        let owner = self.get_object()?;
        let owner_read = owner.read().map_err(|_| "Owner lock poisoned")?;

        if let Some(controlling_player) = owner_read.get_controlling_player() {
            let player_read = controlling_player
                .read()
                .map_err(|_| "Player lock poisoned")?;

            // If this is the last tunnel, kick everyone out (matches C++ lines 222-225)
            if let Some(tunnel_system) = player_read.get_tunnel_system() {
                if tunnel_system.get_tunnel_count() == 1 {
                    drop(player_read);
                    drop(owner_read);
                    self.base.remove_all_contained(false)?;
                }
            }

            // Unregister after the kick out to prevent cave-in kill (matches C++ lines 227-233)
            if self.is_currently_registered {
                let owner_read = owner.read().map_err(|_| "Owner lock poisoned")?;
                let mut player_write = controlling_player
                    .write()
                    .map_err(|_| "Player lock poisoned")?;
                if let Some(tunnel_system_mut) = player_write.get_tunnel_system_mut() {
                    tunnel_system_mut.on_tunnel_destroyed(&*owner_read)?;
                }
                self.is_currently_registered = false;
            }
        }

        Ok(())
    }

    /// Handle capture of the tunnel entrance.
    /// Matches C++ TunnelContain::onCapture (TunnelContain.cpp:416-435).
    pub fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<&Arc<RwLock<crate::player::Player>>>,
        new_owner: Option<&Arc<RwLock<crate::player::Player>>>,
    ) -> GameResult<()> {
        if self.is_currently_registered {
            if let Some(old_owner_arc) = old_owner {
                let mut old_owner_guard =
                    old_owner_arc.write().map_err(|_| "Player lock poisoned")?;
                if let Some(old_tunnel_tracker) = old_owner_guard.get_tunnel_system_mut() {
                    if old_tunnel_tracker.get_contain_count().unwrap_or(0) != 0 {
                        log::warn!(
                            "Tunnel {} captured with passengers still inside; scripted exits may diverge",
                            owner.get_id()
                        );
                    }
                    old_tunnel_tracker.on_tunnel_destroyed(owner)?;
                }
            }

            if let Some(new_owner_arc) = new_owner {
                let mut new_owner_guard =
                    new_owner_arc.write().map_err(|_| "Player lock poisoned")?;
                if let Some(new_tunnel_tracker) = new_owner_guard.get_tunnel_system_mut() {
                    new_tunnel_tracker.on_tunnel_created(owner)?;
                }
            }
        }

        Ok(())
    }

    /// Scatter an exiting unit to a nearby random position.
    /// Matches C++ TunnelContain::scatterToNearbyPosition (TunnelContain.cpp:273-300)
    fn scatter_to_nearby_position(&self, obj: &mut Object) -> GameResult<()> {
        let owner = self.get_object()?;
        let owner_read = owner.read().map_err(|_| "Owner lock poisoned")?;

        // Pick random angle (matches C++ lines 288)
        let angle = get_game_logic_random_value_real(0.0, 2.0 * PI);

        // Calculate scatter radius (matches C++ lines 292-295)
        let min_radius = owner_read.get_geometry_info().get_bounding_circle_radius();
        let max_radius = min_radius + min_radius / 2.0;
        let dist = get_game_logic_random_value_real(min_radius, max_radius);

        let container_pos = *owner_read.get_position();

        // Calculate new position (matches C++ lines 297-299)
        let mut pos = Coord3D::new(
            dist * angle.cos() + container_pos.x,
            dist * angle.sin() + container_pos.y,
            0.0,
        );

        // Get ground height at new position
        if let Ok(terrain) = THE_TERRAIN_LOGIC.read() {
            pos.z = terrain.get_ground_height(pos.x, pos.y, None);
        }

        obj.set_position(&pos)?;

        Ok(())
    }

    /// Get the owning object
    fn get_object(&self) -> GameResult<Arc<RwLock<Object>>> {
        self.object
            .upgrade()
            .ok_or_else(|| "TunnelContain owner object no longer exists".into())
    }
}

impl ContainModuleInterface for TunnelContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj.read() {
                return ContainModuleInterface::is_valid_container_for(self, &*obj_guard, true);
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
        ContainModuleInterface::get_contained_count(&self.base)
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let (_, max) = self.get_usage();
        if max == 0 {
            0
        } else if max == u32::MAX {
            usize::MAX
        } else {
            max as usize
        }
    }

    fn update(
        &mut self,
    ) -> Result<crate::modules::UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        self.base.update().map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_damage(damage_info).map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_die(damage_info).map_err(|e| e.into())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        if let Ok(owner) = self.get_object() {
            if let Ok(owner_read) = owner.read() {
                if let Some(controlling_player) = owner_read.get_controlling_player() {
                    if let Ok(player_read) = controlling_player.read() {
                        if let Some(tunnel_system) = player_read.get_tunnel_system() {
                            return tunnel_system
                                .is_valid_container_for(obj, check_capacity)
                                .unwrap_or(false);
                        }
                    }
                }
            }
        }
        false
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

    fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<&Arc<RwLock<crate::player::Player>>>,
        new_owner: Option<&Arc<RwLock<crate::player::Player>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TunnelContain::on_capture(self, owner, old_owner, new_owner).map_err(|e| e.into())
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.base.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TunnelContain::harm_and_force_exit_all_contained(self, damage_info).map_err(|e| e.into())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TunnelContain::kill_all_contained(self).map_err(|e| e.into())
    }

    fn is_kick_out_on_capture(&self) -> bool {
        false
    }
}

impl ContainerInterface for TunnelContain {
    fn can_contain(&self, obj: &Object) -> bool {
        // Delegate to tunnel tracker validation
        if let Ok(owner) = self.get_object() {
            if let Ok(owner_read) = owner.read() {
                if let Some(controlling_player) = owner_read.get_controlling_player() {
                    if let Ok(player_read) = controlling_player.read() {
                        if let Some(tunnel_system) = player_read.get_tunnel_system() {
                            return tunnel_system
                                .is_valid_container_for(obj, true)
                                .unwrap_or(false);
                        }
                    }
                }
            }
        }
        false
    }

    fn add_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.add_to_contain_list(obj)
    }

    fn remove_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.remove_from_contain(obj, true)
    }

    fn get_usage(&self) -> (u32, u32) {
        if let Ok(owner) = self.get_object() {
            if let Ok(owner_read) = owner.read() {
                if let Some(controlling_player) = owner_read.get_controlling_player() {
                    if let Ok(player_read) = controlling_player.read() {
                        if let Some(tunnel_system) = player_read.get_tunnel_system() {
                            let current = tunnel_system.get_contain_count().unwrap_or(0);
                            let max = tunnel_system.get_contain_max().unwrap_or(-1);
                            let max_u32 = if max < 0 { u32::MAX } else { max as u32 };
                            return (current, max_u32);
                        }
                    }
                }
            }
        }
        (0, 0)
    }
}

/// Helper function to get current game frame
fn get_current_frame() -> GameResult<u32> {
    if let Ok(logic) = crate::system::game_logic::get_game_logic().lock() {
        Ok(logic.get_frame())
    } else {
        Err("Failed to lock game logic".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_contain_is_tunnel() {
        // Would require mock object setup
        // let module_data = TunnelContainModuleData::default();
        // let tunnel = TunnelContain::new(Weak::new(), &module_data).unwrap();
        // assert!(tunnel.is_tunnel_contain());
    }
}
