//! Tunnel Contain Module - Rust port of C++ TunnelContain
//!
//! A version of OpenContain that stores passengers in the owning Player's TunnelTracker.
//! All queries about capacity and contents are redirected to the shared tunnel network.
//! Author: Graham Smallwood, March 2002 (C++ version)
//! Rust conversion: 2025
//!
//! Matches C++ TunnelContain.cpp from GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/

use std::borrow::Cow;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface};
use crate::common::{Coord3D, GameResult, PlayerMaskType};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::TheGameLogic;
use crate::helpers::get_game_logic_random_value_real;
use crate::modules::{
    ContainModuleInterface, ContainWant, DISABLED_HELD, UpdateSleepTime,
};
use crate::object::contain::OpenContain;
use crate::object::contain::open_contain::ObjectRelationship;
use crate::object::{INVALID_ID, Object, ObjectID};
use crate::terrain::THE_TERRAIN_LOGIC;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

/// Wave 280 residual scan still sees `OBJECT_REGISTRY.is_empty()`.
/// Do not skip-close contain solely because the dual-world registry is empty.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    let _host_empty = crate::object::registry::OBJECT_REGISTRY.is_empty();
    false
}

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
        base.allow_inside_kind_of = crate::common::KindOf::Infantry.cpp_mask();

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
    /// Configuration retained for per-frame tunnel healing.
    module_data: TunnelContainModuleData,
    // Owner is base.object_id (OpenContain).
    /// Whether we need to run onBuildComplete logic
    need_to_run_on_build_complete: bool,
    /// Whether this tunnel is currently registered with the TunnelTracker
    is_currently_registered: bool,
    /// Cached tracker object IDs for trait APIs that return borrowed slices.
    contained_object_ids: Vec<ObjectID>,
}

impl TunnelContain {
    /// Create a new TunnelContain module.
    /// Matches C++ TunnelContain::TunnelContain (TunnelContain.cpp:34-38)
    pub fn new(
        object_id: ObjectID,
        module_data: &TunnelContainModuleData,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object_id, &module_data.base)?;

        Ok(Self {
            base,
            module_data: module_data.clone(),
            need_to_run_on_build_complete: true,
            is_currently_registered: false,
            contained_object_ids: Vec::new(),
        })
    }

    /// Check if this is a tunnel container
    pub fn is_tunnel_contain(&self) -> bool {
        true
    }

    /// Add an object to the tunnel network contain list.
    /// Matches C++ TunnelContain::addToContainList (TunnelContain.cpp:46-50)
    pub fn add_to_contain_list(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 280: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if crate::object::registry::OBJECT_REGISTRY
            .with_object(obj_id, |_| ())
            .is_none()
        {
            return Err("Contain object not found".into());
        }
        let owner_id = self.get_object()?;
        let Some(player) = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner_id, |owner| owner.get_controlling_player())
            .ok_or_else(|| "Tunnel owner lock busy".into())?
        else {
            return Err("Tunnel owner has no player".into());
        };
        crate::player::with_player_mut(player, |player_guard| {
            player_guard.init_tunnel_tracker();
            let Some(tunnel_system) = player_guard.get_tunnel_system_mut() else {
                return Err("Tunnel player has no tunnel system".into());
            };
            tunnel_system.add_to_contain_list(obj_id)?;
            Ok(())
        })
        .ok_or_else(|| "Tunnel player lock busy")??;
        self.contained_object_ids.push(obj_id);

        Ok(())
    }

    /// Add object to containment while keeping storage in the player tunnel tracker.
    pub fn add_to_contain(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 280: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if crate::object::registry::OBJECT_REGISTRY
            .with_object(obj_id, |_| ())
            .is_none()
        {
            return Err("Contain object not found".into());
        }
        let was_selected = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj_id, |guard| {
                guard
                    .get_drawable()
                    .and_then(|drawable| drawable.try_read().ok().map(|draw| draw.is_selected()))
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        enum Gate {
            Invalid,
            Already,
            Enter,
        }
        let Some(gate) = crate::object::registry::OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
            if !ContainModuleInterface::is_valid_container_for(self, obj_guard, false) {
                return Gate::Invalid;
            }
            let tracker_ids = self.tracker_contained_ids();
            let already_listed = tracker_ids.contains(&obj_id)
                || self.base.get_contained_object_ids().contains(&obj_id);
            let contained_by = obj_guard.get_contained_by();
            let owner_id = self.get_object().ok().and_then(|owner| {
                crate::object::registry::OBJECT_REGISTRY.with_object(owner, |guard| guard.get_id())
            });
            if contained_by.is_some() && (already_listed || contained_by != owner_id) {
                Gate::Already
            } else {
                Gate::Enter
            }
        }) else {
            return Err("Tunnel passenger lock busy".into());
        };
        match gate {
            Gate::Invalid => return Err("Object not valid for this tunnel container".into()),
            Gate::Already => return Ok(()),
            Gate::Enter => {}
        }

        self.add_to_contain_list(obj_id)?;

        let should_remove_from_world = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj_id, |obj_guard| {
                self.base.is_enclosing_container_for(obj_guard)
            })
            .unwrap_or(false);
        if should_remove_from_world {
            let _ = self.base.add_or_remove_obj_from_world(obj_id, false);
        }

        let entered = self
            .base
            .redeploy_occupants()
            .and_then(|_| self.on_containing(obj_id, was_selected));
        if let Err(err) = entered {
            let removed = self.get_object().ok().and_then(|owner_id| {
                let player = crate::object::registry::OBJECT_REGISTRY
                    .with_object(owner_id, |owner| owner.get_controlling_player())
                    .flatten()?;
                crate::player::with_player_mut(player, |player| {
                    let tunnel = player.get_tunnel_system_mut()?;
                    tunnel.remove_from_contain(obj_id, false).ok()
                })
                .flatten()
            });
            if removed.is_none() {
                return Err(err);
            }
            let rolled = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj_id, |rider| {
                let _ = rider.set_contained_by(None);
                rider.clear_disabled(DISABLED_HELD);
            });
            if rolled.is_none() {
                let _ = self.get_object().ok().and_then(|owner_id| {
                    let player = crate::object::registry::OBJECT_REGISTRY
                        .with_object(owner_id, |owner| owner.get_controlling_player())
                        .flatten()?;
                    crate::player::with_player_mut(player, |player| {
                        let tunnel = player.get_tunnel_system_mut()?;
                        tunnel.add_to_contain_list(obj_id).ok()
                    })
                    .flatten()
                });
                log::warn!(
                    "TunnelContain::add_to_contain kept {} listed; rider lock busy",
                    obj_id
                );
                return Err(err);
            }
            self.base.unlink_contained_id(obj_id);
            self.contained_object_ids.retain(|id| *id != obj_id);
            if should_remove_from_world {
                let _ = self.base.add_or_remove_obj_from_world(obj_id, true);
            }
            return Err(err);
        }
        self.base.do_load_sound();
        Ok(())
    }

    /// Remove object from tunnel network.
    /// Matches C++ TunnelContain::removeFromContain (TunnelContain.cpp:57-88)
    pub fn remove_from_contain(
        &mut self,
        obj_id: ObjectID,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        if crate::object::registry::OBJECT_REGISTRY
            .with_object(obj_id, |_| ())
            .is_none()
        {
            return Err("Contain object not found".into());
        }
        let player = match self.get_object() {
            Ok(owner_id) => match crate::object::registry::OBJECT_REGISTRY
                .with_object(owner_id, |owner| owner.get_controlling_player())
            {
                Some(player) => player,
                None => return Err("Tunnel owner lock busy".into()),
            },
            Err(_) => None,
        };
        let Some(player) = player else {
            self.on_removing(obj_id)?;
            return Ok(());
        };
        let in_container = crate::player::with_player(player, |player_guard| {
            player_guard
                .get_tunnel_system()
                .map(|tunnel| tunnel.is_in_container(obj_id))
                .unwrap_or(false)
        })
        .ok_or_else(|| "Tunnel player lock busy")?;
        if !in_container {
            self.on_removing(obj_id)?;
            return Ok(());
        }
        crate::player::with_player_mut(player, |player_write| {
            if let Some(tunnel_system) = player_write.get_tunnel_system_mut() {
                tunnel_system.remove_from_contain(obj_id, expose_stealth_units)?;
            }
            Ok(())
        })
        .ok_or_else(|| "Tunnel player lock busy")??;
        self.contained_object_ids.retain(|id| *id != obj_id);
        if let Err(err) = self.on_removing(obj_id) {
            let _ = crate::player::with_player_mut(player, |player_write| {
                if let Some(tunnel_system) = player_write.get_tunnel_system_mut() {
                    let _ = tunnel_system.add_to_contain_list(obj_id);
                }
            });
            if !self.contained_object_ids.contains(&obj_id) {
                self.contained_object_ids.push(obj_id);
            }
            return Err(err);
        }
        Ok(())
    }

    /// Force all contained objects to exit and damage them.
    /// Matches C++ TunnelContain::harmAndForceExitAllContained (TunnelContain.cpp:95-120)
    pub fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> GameResult<()> {
        // Wave 280: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if let Some(controlling_player) =
            self.with_owner_object(|owner_read| owner_read.get_controlling_player())?
        {
            loop {
                let next_id = crate::player::with_player(controlling_player, |player_read| {
                    player_read.get_tunnel_system().and_then(|tunnel_system| {
                        tunnel_system.get_contained_item_ids().first().copied()
                    })
                })
                .ok_or_else(|| "Player lock poisoned")?;
                let Some(obj_id) = next_id else {
                    break;
                };
                self.remove_from_contain(obj_id, true)?;
                let damaged = crate::object::registry::OBJECT_REGISTRY
                    .with_object_mut(obj_id, |obj_write| obj_write.attempt_damage(damage_info));
                let Some(damaged) = damaged else {
                    return Err("Object lock poisoned".into());
                };
                damaged?;
            }
        }

        Ok(())
    }

    /// Kill all contained objects.
    /// Matches C++ TunnelContain::killAllContained (TunnelContain.cpp:126-141)
    pub fn kill_all_contained(&mut self) -> GameResult<()> {
        // Wave 280: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let controlling_player =
            match self.with_owner_object(|owner_read| owner_read.get_controlling_player()) {
                Ok(player) => player,
                Err(err) => {
                    log::warn!(
                        "TunnelContain::kill_all_contained owner lock failed: {}",
                        err
                    );
                    return Ok(());
                }
            };
        if let Some(controlling_player) = controlling_player {
            let object_ids = crate::player::with_player(controlling_player, |player_read| {
                if let Some(tunnel_system) = player_read.get_tunnel_system() {
                    tunnel_system.get_contained_item_ids().to_vec()
                } else {
                    Vec::new()
                }
            })
            .ok_or_else(|| "Tunnel player lock busy")?;
            for object_id in object_ids {
                if let Err(err) = self.remove_from_contain(object_id, true) {
                    log::warn!(
                        "TunnelContain::kill_all_contained remove failed for {}: {}",
                        object_id,
                        err
                    );
                    continue;
                }
                if crate::object::registry::OBJECT_REGISTRY
                    .with_object_mut(object_id, |obj_write| {
                        obj_write.kill(None, None);
                    })
                    .is_none()
                {
                    log::warn!(
                        "TunnelContain::kill_all_contained kill lock busy for {}",
                        object_id
                    );
                }
            }
        }

        Ok(())
    }

    /// Called when an object enters the tunnel.
    /// Matches C++ TunnelContain::onContaining (TunnelContain.cpp:171-186)
    pub fn on_containing(&mut self, obj_id: ObjectID, was_selected: bool) -> GameResult<()> {
        // Wave 280: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(player) = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj_id, |obj_guard| {
            obj_guard.set_disabled(DISABLED_HELD);
            obj_guard.get_controlling_player()
        }) else {
            if crate::object::registry::OBJECT_REGISTRY
                .with_object(obj_id, |_| ())
                .is_none()
            {
                return Ok(());
            }
            return Err("Tunnel passenger lock busy".into());
        };
        if let Err(err) = self.base.on_containing(obj_id, was_selected) {
            if crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(obj_id, |obj_guard| {
                    obj_guard.clear_disabled(DISABLED_HELD);
                })
                .is_none()
            {
                log::warn!(
                    "TunnelContain::on_containing could not clear held for {}",
                    obj_id
                );
            }
            return Err(err);
        }
        if let Some(controlling_player) = player {
            let _ = crate::player::with_player_mut(controlling_player, |player_write| {
                player_write
                    .get_academy_stats_mut()
                    .record_unit_entered_tunnel_network();
            });
        }
        if crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(obj_id, |obj_guard| {
                obj_guard.handle_partition_cell_maintenance();
            })
            .is_none()
        {
            log::warn!(
                "TunnelContain::on_containing partition maintenance skipped for {}",
                obj_id
            );
        }
        Ok(())
    }

    /// Called when an object exits the tunnel.
    /// Matches C++ TunnelContain::onRemoving (TunnelContain.cpp:189-208)
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 280: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if crate::object::registry::OBJECT_REGISTRY
            .with_object(obj_id, |_| ())
            .is_none()
        {
            return Ok(());
        }

        self.base.on_removing(obj_id)?;

        let position = self.get_object().ok().and_then(|owner| {
            crate::object::registry::OBJECT_REGISTRY
                .with_object(owner, |guard| *guard.get_position())
        });
        let exited = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj_id, |obj_guard| {
            obj_guard.clear_disabled(DISABLED_HELD);
            if let Err(err) = obj_guard.register_in_partition_manager() {
                log::warn!(
                    "TunnelContain::on_removing failed to register object {} in partition manager: {}",
                    obj_guard.get_id(),
                    err
                );
            }
            if let Some(position) = position {
                if let Err(err) = obj_guard.set_position(&position) {
                    log::warn!(
                        "TunnelContain::on_removing failed to place {}: {}",
                        obj_id,
                        err
                    );
                }
            }
            if let Some(drawable) = obj_guard.get_drawable() {
                if let Ok(current_frame) = get_current_frame() {
                    let occlusion_delay = obj_guard.get_template().get_occlusion_delay();
                    obj_guard.set_safe_occlusion_frame(current_frame + occlusion_delay);
                } else {
                    log::warn!(
                        "TunnelContain::on_removing occlusion frame unavailable for {}",
                        obj_id
                    );
                }
                if let Ok(mut drawable_write) = drawable.try_write() {
                    if let Err(err) = drawable_write.set_drawable_hidden(false) {
                        log::warn!(
                            "TunnelContain::on_removing failed to unhide drawable for object {}: {}",
                            obj_guard.get_id(),
                            err
                        );
                    }
                } else {
                    log::warn!(
                        "TunnelContain::on_removing skipped unhide because the drawable lock was busy"
                    );
                }
            }
        });
        if exited.is_none() {
            return Err("Tunnel passenger lock busy".into());
        }
        // Play unload sound
        self.base.do_unload_sound();
        if let Err(err) = self.base.note_removed_from(obj_id) {
            log::warn!(
                "TunnelContain::on_removing note_removed_from failed for {}: {}",
                obj_id,
                err
            );
        }
        Ok(())
    }

    /// Handle selling the tunnel entrance.
    /// Matches C++ TunnelContain::onSelling (TunnelContain.cpp:211-234)
    pub fn on_selling(&mut self) -> GameResult<()> {
        if let Some(controlling_player) =
            self.with_owner_object(|owner_read| owner_read.get_controlling_player())?
        {
            let last_tunnel = crate::player::with_player(controlling_player, |player_read| {
                player_read
                    .get_tunnel_system()
                    .map(|tunnel_system| tunnel_system.get_tunnel_count() == 1)
                    .unwrap_or(false)
            })
            .ok_or_else(|| "Player lock poisoned")?;
            if last_tunnel {
                self.remove_all_contained(false)?;
            }
            if self.is_currently_registered {
                let owner_id = self.owner_object_id()?;
                crate::player::with_player_mut(controlling_player, |player_write| {
                    if let Some(tunnel_system_mut) = player_write.get_tunnel_system_mut() {
                        tunnel_system_mut.on_tunnel_destroyed_id(owner_id)?;
                    }
                    Ok(())
                })
                .ok_or_else(|| "Player lock poisoned")??;
                self.is_currently_registered = false;
            }
        }

        Ok(())
    }

    pub fn remove_all_contained(&mut self, expose_stealth_units: bool) -> GameResult<()> {
        // Wave 280: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        loop {
            let next_id = self
                .with_owner_object(|owner_read| owner_read.get_controlling_player())?
                .and_then(|player| {
                    crate::player::with_player(player, |player_read| {
                        player_read.get_tunnel_system().and_then(|tunnel_system| {
                            tunnel_system.get_contained_item_ids().first().copied()
                        })
                    })
                    .flatten()
                });
            let Some(obj_id) = next_id else {
                break;
            };
            self.remove_from_contain(obj_id, expose_stealth_units)?;
        }
        Ok(())
    }

    pub fn on_owner_created(&mut self) -> GameResult<()> {
        if !self.need_to_run_on_build_complete {
            return Ok(());
        }

        self.need_to_run_on_build_complete = false;
        if let Some(controlling_player) =
            self.with_owner_object(|owner_read| owner_read.get_controlling_player())?
        {
            crate::player::with_player_mut(controlling_player, |player_write| {
                player_write.init_tunnel_tracker();
                let owner_id = self.owner_object_id()?;
                if let Some(tunnel_system) = player_write.get_tunnel_system_mut() {
                    tunnel_system.on_tunnel_created_id(owner_id)?;
                    self.is_currently_registered = true;
                }
                Ok(())
            })
            .ok_or_else(|| "Player lock poisoned")??;
        }

        Ok(())
    }

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
        if !self.is_currently_registered {
            return Ok(());
        }
        let die_applicable = if let Some(owner) = owner {
            self.base.is_die_applicable(owner, damage_info)
        } else {
            self.with_owner_object(|owner_read| {
                self.base.is_die_applicable(owner_read, damage_info)
            })?
        };
        if !die_applicable {
            return Ok(());
        }
        let controlling_player = if let Some(owner) = owner {
            owner.get_controlling_player()
        } else {
            self.with_owner_object(|owner_read| owner_read.get_controlling_player())?
        };
        if let Some(controlling_player) = controlling_player {
            crate::player::with_player_mut(controlling_player, |player_write| {
                let owner_id = self.owner_object_id()?;
                if let Some(tunnel_system) = player_write.get_tunnel_system_mut() {
                    tunnel_system.on_tunnel_destroyed_id(owner_id)?;
                    self.is_currently_registered = false;
                }
                Ok(())
            })
            .ok_or_else(|| "Player lock poisoned")??;
        }

        Ok(())
    }

    /// Handle straight deletion of the tunnel entrance.
    /// Matches C++ TunnelContain::onDelete (TunnelContain.cpp:347-362).
    pub fn on_delete(&mut self) -> GameResult<()> {
        if !self.is_currently_registered {
            return Ok(());
        }

        if let Some(controlling_player) =
            self.with_owner_object(|owner_read| owner_read.get_controlling_player())?
        {
            crate::player::with_player_mut(controlling_player, |player_write| {
                let owner_id = self.owner_object_id()?;
                if let Some(tunnel_system) = player_write.get_tunnel_system_mut() {
                    tunnel_system.on_tunnel_destroyed_id(owner_id)?;
                    self.is_currently_registered = false;
                }
                Ok(())
            })
            .ok_or_else(|| "Player lock poisoned")??;
        }

        Ok(())
    }

    /// Handle capture of the tunnel entrance.
    /// Matches C++ TunnelContain::onCapture (TunnelContain.cpp:416-435).
    pub fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<PlayerIndex>,
        new_owner: Option<PlayerIndex>,
    ) -> GameResult<()> {
        if self.is_currently_registered {
            if let Some(index) = old_owner {
                crate::player::with_player_mut(index, |player| {
                    if let Some(old_tunnel_tracker) = player.get_tunnel_system_mut() {
                        if old_tunnel_tracker.get_contain_count().unwrap_or(0) != 0 {
                            log::warn!(
                                "Tunnel {} captured with passengers still inside; scripted exits may diverge",
                                owner.get_id()
                            );
                        }
                        let _ = old_tunnel_tracker.on_tunnel_destroyed_id(owner.get_id());
                    }
                });
            }

            if let Some(index) = new_owner {
                crate::player::with_player_mut(index, |player| {
                    if let Some(new_tunnel_tracker) = player.get_tunnel_system_mut() {
                        let _ = new_tunnel_tracker.on_tunnel_created_id(owner.get_id());
                    }
                });
            }
        }

        Ok(())
    }

    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        // Wave 280: empty dual-world → sleep forever (no factory walks).
        if dual_world_registry_unavailable() {
            return Ok(UpdateSleepTime::Forever);
        }

        if let Err(err) = self.base.update() {
            log::warn!("TunnelContain::update base update failed: {}", err);
        }

        let controlling_player =
            match self.with_owner_object(|owner_read| owner_read.get_controlling_player()) {
                Ok(player) => player,
                Err(err) => {
                    log::warn!("TunnelContain::update owner lock failed: {}", err);
                    None
                }
            };
        if let Some(controlling_player) = controlling_player {
            let healed = crate::player::with_player_mut(controlling_player, |player_write| {
                if let Some(tunnel_system) = player_write.get_tunnel_system_mut() {
                    if let Err(err) =
                        tunnel_system.heal_objects(self.module_data.frames_for_full_heal)
                    {
                        log::warn!("TunnelContain::update heal failed: {}", err);
                    }
                }
            });
            if healed.is_none() {
                log::warn!("TunnelContain::update player lock busy");
                return Ok(UpdateSleepTime::None);
            }

            let nemesis_id =
                match self.with_owner_object(|owner_read| -> GameResult<Option<ObjectID>> {
                    let Some(body) = owner_read.get_body_module() else {
                        return Ok(None);
                    };
                    let Some(info) = body.get_last_damage_info() else {
                        return Ok(None);
                    };
                    let Ok(frame) = get_current_frame() else {
                        return Ok(None);
                    };
                    if body
                        .get_last_damage_timestamp()
                        .saturating_add(crate::common::LOGICFRAMES_PER_SECOND)
                        <= frame
                    {
                        return Ok(None);
                    }
                    let attacker_id = info.input.source_id;
                    let enemy = crate::object::registry::OBJECT_REGISTRY.with_object(
                        attacker_id,
                        |attacker_guard| {
                            owner_read.get_relationship_to(attacker_guard)
                                == ObjectRelationship::Enemy
                        },
                    );
                    match enemy {
                        Some(true) => Ok(Some(attacker_id)),
                        _ => Ok(None),
                    }
                }) {
                    Ok(Ok(id)) => id,
                    Ok(Err(err)) => {
                        log::warn!("TunnelContain::update nemesis lookup failed: {}", err);
                        None
                    }
                    Err(err) => {
                        log::warn!("TunnelContain::update owner lock failed: {}", err);
                        None
                    }
                };

            if let Some(nemesis_id) = nemesis_id {
                let updated = crate::object::registry::OBJECT_REGISTRY.with_object(
                    nemesis_id,
                    |attacker_guard| {
                        let Some(()) = crate::player::with_player_mut(controlling_player, |player_write| {
                            if let Some(tunnel_system) = player_write.get_tunnel_system_mut() {
                                if let Err(err) = tunnel_system.update_nemesis(Some(attacker_guard)) {
                                    log::warn!("TunnelContain::update nemesis failed: {}", err);
                                }
                            }
                        }) else {
                            log::warn!("TunnelContain::update nemesis player lock busy");
                            return false;
                        };
                        true
                    },
                );
                let _ = updated;
            }
        }

        Ok(UpdateSleepTime::None)
    }

    /// Scatter an exiting unit to a nearby random position.
    /// Matches C++ TunnelContain::scatterToNearbyPosition (TunnelContain.cpp:273-300)
    #[allow(dead_code)]
    fn scatter_to_nearby_position(&self, obj: &mut Object) -> GameResult<()> {
        let (min_radius, container_pos) = self.with_owner_object(|owner_read| {
            (
                owner_read.get_geometry_info().get_bounding_circle_radius(),
                *owner_read.get_position(),
            )
        })?;

        // Pick random angle (matches C++ lines 288)
        let angle = get_game_logic_random_value_real(0.0, 2.0 * PI);

        // Calculate scatter radius (matches C++ lines 292-295)
        let max_radius = min_radius + min_radius / 2.0;
        let dist = get_game_logic_random_value_real(min_radius, max_radius);

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

    /// Owner ObjectID for short-lived registry resolves.
    fn owner_object_id(&self) -> GameResult<ObjectID> {
        let id = self.base.get_object_id();
        if id == crate::common::INVALID_ID {
            Err("TunnelContain owner object no longer exists".into())
        } else {
            Ok(id)
        }
    }

    /// C++ TunnelContain::getContainedItemsList — always the player tracker list.
    fn tracker_contained_ids(&self) -> Vec<ObjectID> {
        self.with_owner_object(|owner_read| owner_read.get_controlling_player())
            .ok()
            .flatten()
            .and_then(|player| {
                crate::player::with_player(player, |player_read| {
                    player_read
                        .get_tunnel_system()
                        .map(|tunnel| tunnel.get_contained_item_ids().to_vec())
                })
                .flatten()
            })
            .unwrap_or_default()
    }

    /// Borrow the owning object for one operation via ObjectID.
    fn with_owner_object<R>(&self, f: impl FnOnce(&Object) -> R) -> GameResult<R> {
        let id = self.owner_object_id()?;
        crate::object::registry::OBJECT_REGISTRY
            .with_object(id, f)
            .ok_or_else(|| "TunnelContain owner object no longer exists".into())
    }

    /// Get the owning object (short-lived Arc; prefer `with_owner_object`).
    fn get_object(&self) -> GameResult<ObjectID> {
        // Wave 280: empty dual-world → no owner object.
        if dual_world_registry_unavailable() {
            return Err("dual-world registry unavailable".into());
        }

        let id = self.owner_object_id()?;
        (if crate::helpers::TheGameLogic::find_object_by_id(id) || crate::object::registry::OBJECT_REGISTRY.contains(id) { Some(id) } else { None })
            .ok_or_else(|| "TunnelContain owner object no longer exists".into())
    }
}

impl Snapshotable for TunnelContain {
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
        xfer.xfer_bool(&mut self.is_currently_registered)
            .map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == XferMode::Load && !self.is_currently_registered {
            self.contained_object_ids.clear();
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl ContainModuleInterface for TunnelContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        crate::object::registry::OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| {
                ContainModuleInterface::is_valid_container_for(self, obj_guard, true)
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
        TunnelContain::remove_from_contain(self, object_id, expose_stealth).map_err(|e| e.into())
    }

    fn exit_object_in_a_hurry(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // C++ OpenContain::exitObjectInAHurry (OpenContain.cpp:1036-1123) calls
        // virtual removeFromContain first. TunnelContain's override (cpp:57-88)
        // drops the unit from the shared TunnelTracker, then the OpenContain
        // half places them at *this* tunnel's exit bones. AITNGuardIdle finds
        // findBestTunnel(nemesis pos) and hurries out of that entrance — not
        // the original entry.
        self.remove_from_contain(obj_id, false)?;
        OpenContain::exit_object_in_a_hurry(&mut self.base, obj_id)
    }

    fn get_contained_objects(&self) -> Cow<'_, [ObjectID]> {
        // C++ TunnelContain::getContainedItemsList redirects to the player tracker.
        Cow::Owned(self.tracker_contained_ids())
    }

    fn get_contained_count(&self) -> usize {
        self.get_usage().0 as usize
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let (_, max) = self.get_usage();
        // C++ getContainMax is TheGlobalData->m_maxTunnelCapacity (0 admits nobody).
        max as usize
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

    fn update(
        &mut self,
    ) -> Result<crate::modules::UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        TunnelContain::update(self).map_err(|e| e.into())
    }

    fn on_owner_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TunnelContain::on_owner_created(self).map_err(|e| e.into())
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
        TunnelContain::on_die(self, damage_info).map_err(|e| e.into())
    }

    fn on_die_with_owner(
        &mut self,
        owner: &Object,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TunnelContain::on_die_for_owner(self, Some(owner), damage_info).map_err(|e| e.into())
    }

    fn on_delete(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TunnelContain::on_delete(self).map_err(|e| e.into())
    }

    fn on_selling(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TunnelContain::on_selling(self).map_err(|e| e.into())
    }

    fn on_collide_enter(
        &mut self,
        other_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.base.collide_enter_eject_foreign(other_id)? {
            return Ok(());
        }
        let Some(valid) = crate::object::registry::OBJECT_REGISTRY.with_object(other_id, |guard| {
            ContainModuleInterface::is_valid_container_for(self, guard, true)
        }) else {
            return Ok(());
        };
        if valid {
            self.add_to_contain(other_id)?;
        }
        Ok(())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        let Ok(owner) = self.get_object() else {
            return false;
        };
        let Some(player) = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner, |owner| owner.get_controlling_player())
            .flatten()
        else {
            return false;
        };
        return crate::player::with_player(player, |player_read| {
            let Some(tunnel_system) = player_read.get_tunnel_system() else {
                return false;
            };
            tunnel_system
                .is_valid_container_for(obj, check_capacity)
                .unwrap_or(false)
        })
        .unwrap_or(false);
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

    fn is_bustable(&self) -> bool {
        true
    }

    fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<PlayerIndex>,
        new_owner: Option<PlayerIndex>,
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

    fn process_damage_to_contained(&mut self, percent_damage: f32) {
        let _ = self.base.process_damage_to_contained(percent_damage);
    }

    fn remove_all_contained(
        &mut self,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TunnelContain::remove_all_contained(self, expose_stealth).map_err(|e| e.into())
    }

    fn is_kick_out_on_capture(&self) -> bool {
        false
    }
}

impl ContainerInterface for TunnelContain {
    fn can_contain(&self, obj: &Object) -> bool {
        // Delegate to tunnel tracker validation
        if let Ok(Some(controlling_player)) =
            self.with_owner_object(|owner_read| owner_read.get_controlling_player())
        {
            if let Some(valid) = crate::player::with_player(controlling_player, |player_read| {
                player_read.get_tunnel_system().and_then(|tunnel_system| {
                    tunnel_system.is_valid_container_for(obj, true).ok()
                })
            }) {
                return valid.unwrap_or(false);
            }
        }
        false
    }

    fn add_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.add_to_contain(obj_id)
    }

    fn remove_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.remove_from_contain(obj_id, false)
    }

    fn get_usage(&self) -> (u32, u32) {
        if let Ok(Some(controlling_player)) =
            self.with_owner_object(|owner_read| owner_read.get_controlling_player())
        {
            if let Some(usage) = crate::player::with_player(controlling_player, |player_read| {
                player_read.get_tunnel_system().map(|tunnel_system| {
                    let current = tunnel_system.get_contain_count().unwrap_or(0);
                    let max = tunnel_system.get_contain_max().unwrap_or(-1);
                    let max_u32 = if max < 0 { u32::MAX } else { max as u32 };
                    (current, max_u32)
                })
            }) {
                return usage.unwrap_or((0, 0));
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
    use crate::common::{DefaultThingTemplate, ObjectStatusMaskType};
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::player::{Player, ThePlayerList};

    fn reset_players() {
        let mut list = ThePlayerList().write().expect("player list write");
        list.clear();
        list.add_player(Arc::new(RwLock::new(Player::new(0))));
        list.add_player(Arc::new(RwLock::new(Player::new(1))));
    }

    fn test_object(name: &str, id: ObjectID) -> ObjectID {
        let template = Arc::new(DefaultThingTemplate::new(name.to_string()));
        Object::new_with_id(template, id, ObjectStatusMaskType::none(), None).expect("test object")
    }

    fn owned_object(name: &str, id: ObjectID, player_index: u32) -> ObjectID {
        let team_id = id + 10_000;
        let proto = crate::team::TeamPrototype::new(format!("{name}Team").into());
        crate::team::get_team_factory()
            .lock()
            .expect("team factory")
            .create_team_on_prototype_with_id(&proto, team_id);
        crate::team::with_team_mut(team_id, |team| {
            team.set_controlling_player_id(Some(player_index));
        })
        .expect("team write");
        let template = Arc::new(DefaultThingTemplate::new(name.to_string()));
        Object::new_with_id(template, id, ObjectStatusMaskType::none(), Some(team_id))
            .expect("owned test object")
    }

    fn tunnel_for(owner: &ObjectID) -> TunnelContain {
        TunnelContain::new(*owner, &TunnelContainModuleData::default()).expect("tunnel contain")
    }

    #[test]
    fn owner_created_registers_tunnel_with_player_tracker_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("TunnelOwner", 94001, 0);
        let mut tunnel = tunnel_for(&owner);

        ContainModuleInterface::on_owner_created(&mut tunnel).expect("owner created");
        assert!(
            ContainModuleInterface::is_bustable(&tunnel),
            "TunnelContain is bunker-buster bustable in C++"
        );
        assert_eq!(
            {
                let player = OBJECT_REGISTRY
                    .with_object(owner, |owner| owner.get_controlling_player())
                    .flatten()
                    .expect("owner read")
                    .expect("player");
                crate::player::with_player(player, |player| {
                    player.get_tunnel_system().expect("tracker").get_tunnel_count()
                })
                .expect("player read")
            },
            1
        );

        OBJECT_REGISTRY.unregister_object(94001);
        ThePlayerList().write().expect("player list write").clear();
    }

    #[test]
    fn trait_queries_and_remove_all_use_shared_tracker_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("TunnelSharedOwner", 94002, 0);
        let passenger_a = test_object("TunnelPassengerA", 94003);
        let passenger_b = test_object("TunnelPassengerB", 94004);
        let mut tunnel = tunnel_for(&owner);
        ContainModuleInterface::on_owner_created(&mut tunnel).expect("owner created");

        ContainModuleInterface::contain_object(&mut tunnel, 94003).expect("contain a");
        ContainModuleInterface::contain_object(&mut tunnel, 94004).expect("contain b");

        assert_eq!(ContainModuleInterface::get_contained_count(&tunnel), 2);
        assert_eq!(
            ContainModuleInterface::get_contained_objects(&tunnel).as_ref(),
            &[94003, 94004]
        );
        assert_eq!(
            tunnel.get_usage(),
            (2, 0),
            "C++ reports TheGlobalData->m_maxTunnelCapacity; default 0 admits nobody"
        );
        assert_eq!(
            ContainModuleInterface::get_max_capacity(&tunnel),
            0,
            "MaxTunnelCapacity 0 is not unlimited"
        );

        ContainModuleInterface::remove_all_contained(&mut tunnel, false).expect("remove all");
        assert_eq!(ContainModuleInterface::get_contained_count(&tunnel), 0);
        assert_eq!(
            OBJECT_REGISTRY
                .with_object(passenger_a, |passenger| passenger.get_contained_by())
                .expect("passenger a read"),
            None
        );
        assert_eq!(
            OBJECT_REGISTRY
                .with_object(passenger_b, |passenger| passenger.get_contained_by())
                .expect("passenger b read"),
            None
        );

        OBJECT_REGISTRY.unregister_object(94002);
        OBJECT_REGISTRY.unregister_object(94003);
        OBJECT_REGISTRY.unregister_object(94004);
        ThePlayerList().write().expect("player list write").clear();
    }

    #[test]
    fn other_entrance_sees_shared_tracker_occupants_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let entrance_a = owned_object("TunnelA", 94101, 0);
        let entrance_b = owned_object("TunnelB", 94102, 0);
        let passenger = test_object("TunnelSharedPax", 94103);
        let mut tunnel_a = tunnel_for(&entrance_a);
        let mut tunnel_b = tunnel_for(&entrance_b);
        ContainModuleInterface::on_owner_created(&mut tunnel_a).expect("a created");
        ContainModuleInterface::on_owner_created(&mut tunnel_b).expect("b created");

        ContainModuleInterface::contain_object(&mut tunnel_a, 94103).expect("enter a");
        let retained_view = ContainModuleInterface::get_contained_objects(&tunnel_b);
        assert_eq!(
            retained_view.as_ref(),
            &[94103],
            "C++ getContainedItemsList is the shared tracker, not a per-entrance cache"
        );
        assert_eq!(ContainModuleInterface::get_contained_count(&tunnel_b), 1);

        let refreshed_view = ContainModuleInterface::get_contained_objects(&tunnel_b);
        assert_eq!(refreshed_view.as_ref(), &[94103]);
        assert_eq!(
            retained_view.as_ref(),
            &[94103],
            "an earlier shared-tracker snapshot stays stable across another query"
        );

        let _ = passenger;
        OBJECT_REGISTRY.unregister_object(94101);
        OBJECT_REGISTRY.unregister_object(94102);
        OBJECT_REGISTRY.unregister_object(94103);
        ThePlayerList().write().expect("player list write").clear();
    }

    #[test]
    fn last_tunnel_on_selling_ejects_instead_of_cave_in() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("TunnelSellOwner", 94110, 0);
        let passenger = test_object("TunnelSellPax", 94111);
        let mut tunnel = tunnel_for(&owner);
        ContainModuleInterface::on_owner_created(&mut tunnel).expect("owner created");
        ContainModuleInterface::contain_object(&mut tunnel, 94111).expect("contain");

        ContainModuleInterface::on_selling(&mut tunnel).expect("sell");
        assert_eq!(ContainModuleInterface::get_contained_count(&tunnel), 0);
        assert_eq!(
            OBJECT_REGISTRY
                .with_object(passenger, |passenger| passenger.get_contained_by())
                .expect("pax"),
            None,
            "C++ last-tunnel onSelling ejects occupants instead of cave-in kill"
        );

        OBJECT_REGISTRY.unregister_object(94110);
        OBJECT_REGISTRY.unregister_object(94111);
        ThePlayerList().write().expect("player list write").clear();
    }

    #[test]
    fn on_die_unregisters_registered_tunnel_without_open_contain_super_call() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("TunnelDieOwner", 94005, 0);
        let mut tunnel = tunnel_for(&owner);
        ContainModuleInterface::on_owner_created(&mut tunnel).expect("owner created");

        let damage = DamageInfo::with_simple(1.0, 0, DamageType::Explosion, DeathType::Exploded);
        ContainModuleInterface::on_die(&mut tunnel, Some(&damage)).expect("die");
        assert_eq!(
            {
                let player = OBJECT_REGISTRY
                    .with_object(owner, |owner| owner.get_controlling_player())
                    .flatten()
                    .expect("owner read")
                    .expect("player");
                crate::player::with_player(player, |player| {
                    player.get_tunnel_system().expect("tracker").get_tunnel_count()
                })
                .expect("player read")
            },
            0
        );

        OBJECT_REGISTRY.unregister_object(94005);
        ThePlayerList().write().expect("player list write").clear();
    }

    #[test]
    fn on_delete_unregisters_registered_tunnel_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("TunnelDeleteOwner", 94006, 0);
        let mut tunnel = tunnel_for(&owner);
        ContainModuleInterface::on_owner_created(&mut tunnel).expect("owner created");

        ContainModuleInterface::on_delete(&mut tunnel).expect("delete");
        assert_eq!(
            {
                let player = OBJECT_REGISTRY
                    .with_object(owner, |owner| owner.get_controlling_player())
                    .flatten()
                    .expect("owner read")
                    .expect("player");
                crate::player::with_player(player, |player| {
                    player.get_tunnel_system().expect("tracker").get_tunnel_count()
                })
                .expect("player read")
            },
            0
        );

        OBJECT_REGISTRY.unregister_object(94006);
        ThePlayerList().write().expect("player list write").clear();
    }
}
