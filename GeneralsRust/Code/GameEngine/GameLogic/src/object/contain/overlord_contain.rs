//! Overlord Contain Module - Rust port of C++ OverlordContain
//!
//! Specialized container for Overlord tank functionality. Acts as transport normally,
//! but when full it redirects queries to the first passenger (bunker).
//! Author: Graham Smallwood, September 2002 (C++ version)
//! Rust conversion: 2025
//!
//! Matches C++ OverlordContain.cpp from GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface, TransportContain, TransportContainModuleData};
use crate::common::types::BodyDamageType;
use crate::common::{GameResult, KindOf, ObjectID, PlayerMaskType, ThingFactory};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::modules::{
    BodyModuleGuardExt, BodyModuleInterfaceExt, ContainModuleInterface, ContainModuleInterfaceExt,
    ExperienceTrackerExt, UpdateSleepTime,
};
use crate::object::{Object, ObjectId};
use crate::player::Player;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Configuration data for OverlordContain module
/// Matches C++ OverlordContainModuleData (OverlordContain.h:20-33)
#[derive(Debug, Clone)]
pub struct OverlordContainModuleData {
    /// Configuration from parent TransportContain
    pub base: TransportContainModuleData,
    /// List of payload template names that can be loaded
    pub payload_template_names: Vec<String>,
    /// Whether the rider sinks experience from the Overlord
    pub experience_sink_for_rider: bool,
}

impl Default for OverlordContainModuleData {
    fn default() -> Self {
        Self {
            base: Default::default(),
            payload_template_names: Vec::new(),
            experience_sink_for_rider: true, // Matches C++ default
        }
    }
}

impl OverlordContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, OVERLORD_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, OVERLORD_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for OverlordContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        OverlordContainModuleData::parse_from_config(self, config)
    }
}

fn parse_payload_template_name(
    _ini: &mut INI,
    data: &mut OverlordContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.payload_template_names
        .extend(tokens.iter().map(|token| (*token).to_string()));
    Ok(())
}

fn parse_experience_sink_for_rider(
    _ini: &mut INI,
    data: &mut OverlordContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.experience_sink_for_rider = INI::parse_bool(token)?;
    Ok(())
}

const OVERLORD_CONTAIN_FIELDS: &[FieldParse<OverlordContainModuleData>] = &[
    FieldParse {
        token: "PayloadTemplateName",
        parse: parse_payload_template_name,
    },
    FieldParse {
        token: "ExperienceSinkForRider",
        parse: parse_experience_sink_for_rider,
    },
];

/// Overlord contain module - tank with external bunker rider
/// Matches C++ OverlordContain (OverlordContain.h:35-106)
#[derive(Debug)]
pub struct OverlordContain {
    /// Base functionality from TransportContain
    pub base: TransportContain,
    /// Reference to the owning object (the Overlord tank)
    object_id: ObjectID,
    /// Module configuration
    module_data: OverlordContainModuleData,
    /// Whether redirection to bunker is currently active
    redirection_activated: bool,
}

impl OverlordContain {
    /// Create a new OverlordContain module.
    /// Matches C++ OverlordContain::OverlordContain (OverlordContain.cpp:71-78)
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &OverlordContainModuleData,
    ) -> GameResult<Self> {
        let base = TransportContain::new(object.clone(), &module_data.base)?;

        Ok(Self {
            base,
            object_id: object
                .upgrade()
                .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
                .unwrap_or(crate::common::INVALID_ID),
            module_data: module_data.clone(),
            redirection_activated: false,
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

    /// Check if this is a garrisonable container (depends on redirection).
    /// Matches C++ OverlordContain::isGarrisonable (OverlordContain.cpp:238-244)
    pub fn is_garrisonable(&self) -> bool {
        if let Some(redirected) = self.get_redirected_contain() {
            redirected
                .lock()
                .map(|guard| guard.is_garrisonable())
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Check if this is bustable (bunker-buster vulnerable).
    /// Matches C++ OverlordContain::isBustable (OverlordContain.h:51)
    pub fn is_bustable(&self) -> bool {
        false
    }

    /// Check if this is a heal container.
    /// Matches C++ OverlordContain (OverlordContain.h:52)
    pub fn is_heal_contain(&self) -> bool {
        false
    }

    /// Check if this is a tunnel container.
    /// Matches C++ OverlordContain (OverlordContain.h:53)
    pub fn is_tunnel_contain(&self) -> bool {
        false
    }

    /// Check if immune to clear building attacks.
    /// Matches C++ OverlordContain (OverlordContain.h:54)
    pub fn is_immune_to_clear_building_attacks(&self) -> bool {
        true
    }

    /// Check if this is a special overlord style container.
    /// Matches C++ OverlordContain (OverlordContain.h:55)
    pub fn is_special_overlord_style_container(&self) -> bool {
        true
    }

    /// Check if passenger is allowed to fire.
    /// Matches C++ OverlordContain::isPassengerAllowedToFire (OverlordContain.h:56)
    pub fn is_passenger_allowed_to_fire(&self, id: Option<ObjectId>) -> bool {
        if let Some(obj_id) = id {
            if let Some(passenger) = TheGameLogic::find_object_by_id(obj_id) {
                if let Ok(passenger_guard) = passenger.read() {
                    if !passenger_guard.is_kind_of(KindOf::Infantry)
                        && !passenger_guard.is_kind_of(KindOf::PortableStructure)
                    {
                        return false;
                    }
                }
            }
        }

        if let Some(owner) = self.get_object() {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.get_contained_by().is_some() {
                    return false;
                }
            }
        }

        self.base.is_passenger_allowed_to_fire(id)
    }

    /// Handle death of the Overlord.
    /// Matches C++ OverlordContain::onDie (OverlordContain.cpp:185-205)
    pub fn on_die(&mut self, damage_info: Option<&DamageInfo>) -> GameResult<()> {
        // Do you mean me the Overlord, or my behavior of passing stuff on to my passengers?
        if self.get_redirected_contain().is_none() {
            return self.base.on_die(damage_info);
        }

        // Everything is fine if I am empty or carrying a regular guy. If I have a redirected
        // contain set up, then I need to handle the order of death explicitly, or things will
        // become confused when I stop redirecting in the middle of the process.
        // So this is an extend that lets me control the order of death.

        self.deactivate_redirected_contain()?;

        if let Some(rider) = self.base.base.get_contained_items_list()?.first() {
            if let Ok(mut rider_guard) = rider.write() {
                rider_guard.kill(None, None);
            }
        }

        self.base.on_die(damage_info)
    }

    /// Handle deletion of the Overlord.
    /// Matches C++ OverlordContain::onDelete (OverlordContain.cpp:208-224)
    pub fn on_delete(&mut self) -> GameResult<()> {
        // Do you mean me the Overlord, or my behavior of passing stuff on to my passengers?
        if self.get_redirected_contain().is_none() {
            return self.base.on_delete();
        }

        // Without my throwing the redirect switch, teardown deletion will get confused
        // and fire off a bunch of asserts
        if let Some(redirected) = self.get_redirected_contain() {
            if let Ok(mut guard) = redirected.lock() {
                let _ = guard.remove_all_contained(false);
            }
        }

        self.deactivate_redirected_contain()?;
        self.base.base.remove_all_contained(false)?;

        self.base.on_delete()
    }

    /// Handle capture event.
    /// Matches C++ OverlordContain::onCapture (OverlordContain.cpp:227-235)
    pub fn on_capture(
        &mut self,
        _owner: &Object,
        _old_owner: Option<&Arc<RwLock<Player>>>,
        new_owner: Option<&Arc<RwLock<Player>>>,
    ) -> GameResult<()> {
        if self.base.base.get_contain_count() < 1 {
            return Ok(());
        }

        // Need to capture our specific rider. He will then kick passengers out if he is a Transport
        if let (Some(rider), Some(new_owner_arc)) = (
            self.base.base.get_contained_items_list()?.first(),
            new_owner,
        ) {
            if let Ok(mut rider_guard) = rider.write() {
                if let Ok(new_owner_guard) = new_owner_arc.read() {
                    let default_team = new_owner_guard.get_default_team();
                    rider_guard.set_team(default_team)?;
                }
            }
        }

        Ok(())
    }

    /// Handle object creation event
    /// Matches C++ OverlordContain::onObjectCreated
    pub fn on_object_created(&mut self) -> GameResult<()> {
        self.create_payload()
    }

    /// Called when this object starts containing another object.
    /// Matches C++ OverlordContain::onContaining (OverlordContain.h:65)
    pub fn on_containing(&mut self, obj_id: ObjectID, was_selected: bool) -> GameResult<()> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        if let Some(redirected) = self.get_redirected_contain() {
            self.base.base.on_containing(obj_id, was_selected)?;
            if let Ok(mut guard) = redirected.lock() {
                let _ = guard.on_containing(obj_id, was_selected);
            }
            return Ok(());
        }

        self.base.on_containing(obj_id, was_selected)?;

        let is_portable = obj
            .read()
            .map(|guard| guard.is_kind_of(KindOf::PortableStructure))
            .unwrap_or(false);

        if is_portable {
            self.activate_redirected_contain()?;

            if self.module_data.experience_sink_for_rider {
                if let (Some(owner), Ok(obj_guard)) = (self.get_object(), obj.read()) {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(tracker) = obj_guard.get_experience_tracker() {
                            tracker.set_experience_sink(owner_guard.get_id());
                        }
                    }
                }
            }

            if let Some(owner) = self.get_object() {
                if let Ok(owner_guard) = owner.read() {
                    if owner_guard.is_stealthed() {
                        if let Ok(obj_guard) = obj.read() {
                            if let Some(stealth) = obj_guard.get_stealth() {
                                if let Ok(mut stealth_guard) = stealth.lock() {
                                    let _ = stealth_guard.receive_grant(true, 0, 0);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Kill all contained passengers (not the portable turret).
    /// Matches C++ OverlordContain::killAllContained
    pub fn kill_all_contained(&mut self) -> GameResult<()> {
        if let Some(redirected) = self.get_redirected_contain() {
            if let Ok(guard) = redirected.lock() {
                for obj_id in guard.get_contained_objects() {
                    if let Some(obj) = TheGameLogic::find_object_by_id(*obj_id) {
                        if let Ok(mut obj_guard) = obj.write() {
                            obj_guard.kill(None, None);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Iterate contained list and invoke callback.
    /// Matches C++ OverlordContain::iterateContained
    pub fn iterate_contained<F>(&self, mut func: F, reverse: bool) -> GameResult<()>
    where
        F: FnMut(Arc<RwLock<Object>>) -> GameResult<()>,
    {
        if let Some(redirected) = self.get_redirected_contain() {
            if let Ok(guard) = redirected.lock() {
                let mut objs: Vec<Arc<RwLock<Object>>> = guard
                    .get_contained_objects()
                    .iter()
                    .filter_map(|id| TheGameLogic::find_object_by_id(*id))
                    .collect();

                if reverse {
                    objs.reverse();
                }

                for obj in objs {
                    func(obj)?;
                }

                return Ok(());
            }
        }

        self.base.base.iterate_contained(func, reverse)
    }

    /// Called when removing an object from containment.
    /// Matches C++ OverlordContain::onRemoving (OverlordContain.h:66)
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        if let Some(redirected) = self.get_redirected_contain() {
            self.base.base.on_removing(obj_id)?;
            if let Ok(mut guard) = redirected.lock() {
                let _ = guard.on_removing(obj_id);
            }
        } else {
            self.base.on_removing(obj_id)?;
        }

        // Deactivate redirection when becoming empty
        if self.base.base.get_contain_count() == 0 {
            self.deactivate_redirected_contain()?;
        }

        Ok(())
    }

    /// Get contained items list.
    /// Matches C++ OverlordContain::getContainedItemsList
    pub fn get_contained_items_list(&self) -> GameResult<Vec<Arc<RwLock<Object>>>> {
        if let Some(redirected) = self.get_redirected_contain() {
            if let Ok(guard) = redirected.lock() {
                let items: Vec<_> = guard
                    .get_contained_objects()
                    .iter()
                    .filter_map(|obj_id| TheGameLogic::find_object_by_id(*obj_id))
                    .collect();
                return Ok(items);
            }
        }

        self.base.base.get_contained_items_list()
    }

    /// Add object to contain list.
    pub fn add_to_contain_list(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        if let Some(redirected) = self.get_redirected_contain() {
            if let Ok(mut guard) = redirected.lock() {
                if let Ok(obj_guard) = obj.read() {
                    guard.add_to_contain_list(&*obj_guard)?;
                }
            }
            return Ok(());
        }

        self.base.add_to_contain_list(
            obj.read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
        )
    }

    /// Add object to containment.
    pub fn add_to_contain(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        if let Some(redirected) = self.get_redirected_contain() {
            if let Ok(mut guard) = redirected.lock() {
                if let Ok(obj_guard) = obj.read() {
                    let _ = guard.add_to_contain(&*obj_guard);
                }
            }
            return Ok(());
        }

        self.base.add_to_contain(
            obj.read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
        )
    }

    /// Remove object from containment.
    pub fn remove_from_contain(
        &mut self,
        obj: Arc<RwLock<Object>>,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        if let Some(redirected) = self.get_redirected_contain() {
            if let Ok(mut guard) = redirected.lock() {
                if let Ok(obj_guard) = obj.read() {
                    guard.release_object(obj_guard.get_id()).map_err(
                        |e: String| -> Box<dyn std::error::Error + Send + Sync> { e.into() },
                    )?;
                }
            }
            return Ok(());
        }

        self.base.remove_from_contain(
            obj.read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            expose_stealth_units,
        )
    }

    /// Remove all contained objects.
    pub fn remove_all_contained(&mut self, expose_stealth_units: bool) -> GameResult<()> {
        if let Some(redirected) = self.get_redirected_contain() {
            let ids = if let Ok(guard) = redirected.lock() {
                guard.get_contained_objects().to_vec()
            } else {
                Vec::new()
            };

            for obj_id in ids {
                if let Some(obj) = TheGameLogic::find_object_by_id(obj_id) {
                    self.remove_from_contain(obj, expose_stealth_units)?;
                }
            }
            return Ok(());
        }

        self.base.base.remove_all_contained(expose_stealth_units)
    }

    /// Check if this container is valid for the given object.
    /// Matches C++ OverlordContain::isValidContainerFor (OverlordContain.h:68)
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        if let Some(redirected) = self.get_redirected_contain() {
            redirected
                .lock()
                .map(|guard| guard.is_valid_container_for(obj, check_capacity))
                .unwrap_or(false)
        } else {
            self.base.is_valid_container_for(obj, check_capacity)
        }
    }

    /// Get current containment count.
    /// Matches C++ OverlordContain::getContainCount (OverlordContain.h:80)
    pub fn get_contain_count(&self) -> u32 {
        if let Some(redirected) = self.get_redirected_contain() {
            redirected
                .lock()
                .map(|guard| guard.get_contain_count())
                .unwrap_or(0)
        } else {
            self.base.base.get_contain_count()
        }
    }

    /// Get maximum containment capacity.
    /// Matches C++ OverlordContain::getContainMax (OverlordContain.h:81)
    pub fn get_contain_max(&self) -> i32 {
        if let Some(redirected) = self.get_redirected_contain() {
            redirected
                .lock()
                .map(|guard| guard.get_contain_max())
                .unwrap_or(super::CONTAIN_MAX_UNKNOWN)
        } else {
            self.base.get_contain_max()
        }
    }

    /// Check if this is an enclosing container for the given object.
    /// Matches C++ OverlordContain::isEnclosingContainerFor
    pub fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        if let Ok(items) = self.base.base.get_contained_items_list() {
            if let Some(rider) = items.first() {
                if let Ok(rider_guard) = rider.read() {
                    if rider_guard.get_id() == obj.get_id() {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Get container pips to show in UI.
    /// Matches C++ OverlordContain::getContainerPipsToShow
    pub fn get_container_pips_to_show(&self) -> (i32, i32, bool) {
        if let Some(redirected) = self.get_redirected_contain() {
            if let Ok(guard) = redirected.lock() {
                let max = guard.get_contain_max();
                let total = if max < 0 { 0 } else { max };
                let full = guard.get_contain_count() as i32;
                return (total, full, true);
            }
        }

        (0, 0, false)
    }

    /// Check if displayed on control bar.
    /// Matches C++ OverlordContain::isDisplayedOnControlBar (OverlordContain.h:74)
    pub fn is_displayed_on_control_bar(&self) -> bool {
        if let Some(redirected) = self.get_redirected_contain() {
            redirected
                .lock()
                .map(|guard| guard.is_displayed_on_control_bar())
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Check if kick out on capture.
    /// Matches C++ OverlordContain::isKickOutOnCapture (OverlordContain.cpp:247-253)
    pub fn is_kick_out_on_capture(&self) -> bool {
        if let Some(redirected) = self.get_redirected_contain() {
            redirected
                .lock()
                .map(|guard| guard.is_kick_out_on_capture())
                .unwrap_or(false)
        } else {
            false // Me the Overlord doesn't want to
        }
    }

    /// Get the first rider object.
    /// Matches C++ OverlordContain::friend_getRider (OverlordContain.h:85)
    pub fn friend_get_rider(&self) -> Option<Arc<RwLock<Object>>> {
        self.base
            .base
            .get_contained_items_list()
            .ok()
            .and_then(|list| list.first().cloned())
    }

    /// Flash selected for visible contained units.
    /// Matches C++ OverlordContain::clientVisibleContainedFlashAsSelected (OverlordContain.h:89)
    pub fn client_visible_contained_flash_as_selected(&self) -> GameResult<()> {
        if let Ok(items) = self.base.base.get_contained_items_list() {
            for item in items {
                if let Ok(item_guard) = item.read() {
                    if !item_guard.is_kind_of(KindOf::PortableStructure) {
                        continue;
                    }
                    if let Some(drawable) = item_guard.get_drawable() {
                        if let Ok(mut drawable_guard) = drawable.write() {
                            drawable_guard.flash_as_selected();
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Create initial payload.
    /// Matches C++ OverlordContain::createPayload (OverlordContain.cpp:95-136)
    pub fn create_payload(&mut self) -> GameResult<()> {
        if self.base.is_payload_created() {
            return Ok(());
        }

        let owner = self
            .get_object()
            .ok_or("Overlord object no longer exists")?;
        let owner_guard = owner.read().map_err(|_| "Owner lock poisoned")?;

        // Get thing factory and create payload objects
        if let Ok(factory) = ThingFactory::get() {
            if let Some(contain) = owner_guard.get_contain() {
                contain.enable_load_sounds(false)?;

                for template_name in &self.module_data.payload_template_names {
                    if let Some(template) = ThingFactory::find_template(template_name) {
                        if let Some(team) = owner_guard.get_team() {
                            if let Ok(team_ref) = team.read() {
                                if let Ok(payload) = factory.new_object(template, &*team_ref) {
                                    if let Ok(payload_ref) = payload.read() {
                                        if contain.is_valid_container_for(&*payload_ref, true) {
                                            contain.add_to_contain(&*payload_ref);
                                        } else {
                                            return Err(format!(
                                                "OverlordContain::createPayload: {} is full or not valid for payload {}",
                                                owner_guard.get_name(),
                                                template_name
                                            )
                                            .into());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                contain.enable_load_sounds(true)?;
            }
        }

        self.base.set_payload_created(true);
        Ok(())
    }

    /// Handle body damage state changes.
    /// Matches C++ OverlordContain::onBodyDamageStateChange (OverlordContain.cpp:140-153)
    pub fn on_body_damage_state_change(
        &mut self,
        _damage_info: &DamageInfo,
        _old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) -> GameResult<()> {
        // I can't use any convenience functions, as they will all get routed to the bunker I may carry.
        // I want just me.
        // Oh, and I don't want this function trying to do death. That is more complicated and will
        // be handled on my death.
        if new_state != BodyDamageType::Rubble && self.base.base.get_contain_count() == 1 {
            if let Ok(items) = self.base.base.get_contained_items_list() {
                if let Some(rider) = items.first() {
                    if let Ok(rider_guard) = rider.read() {
                        if let Some(body) = rider_guard.get_body_module() {
                            body.set_damage_state(new_state);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Get redirected contain interface (bunker inside).
    /// Matches C++ OverlordContain::getRedirectedContain (OverlordContain.cpp:157-175)
    fn get_redirected_contain(&self) -> Option<Arc<Mutex<dyn ContainModuleInterface>>> {
        // Naturally, I cannot use a redirectible convenience function
        // to answer if I am redirecting yet.

        // If I am empty, say no.
        if self.base.base.get_contain_count() < 1 {
            return None;
        }

        // Shut off early to allow death to happen without my bunker having
        // trouble finding me to say goodbye as messages get sucked up the pipe to him.
        if !self.redirection_activated {
            return None;
        }

        // Get the first rider
        if let Ok(items) = self.base.base.get_contained_items_list() {
            if let Some(rider) = items.first() {
                if let Ok(rider_guard) = rider.read() {
                    return rider_guard.get_contain();
                }
            }
        }

        None // Or say no if they have no contain
    }

    /// Activate redirection to contained bunker.
    /// Matches C++ OverlordContain::activateRedirectedContain (OverlordContain.h:99)
    fn activate_redirected_contain(&mut self) -> GameResult<()> {
        if self.base.base.get_contain_count() == 1 {
            self.redirection_activated = true;
        }
        Ok(())
    }

    /// Deactivate redirection to contained bunker.
    /// Matches C++ OverlordContain::deactivateRedirectedContain (OverlordContain.h:100)
    fn deactivate_redirected_contain(&mut self) -> GameResult<()> {
        self.redirection_activated = false;
        Ok(())
    }

    /// Update method called once per frame.
    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        // Create payload if not already created
        if !self.base.is_payload_created() {
            self.create_payload()?;
        }

        self.base.update()
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        let mut state = self.base.save_state()?;

        state.insert(
            "redirection_activated".to_string(),
            vec![if self.redirection_activated { 1 } else { 0 }],
        );
        Ok(state)
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        self.base.load_state(state)?;

        if let Some(data) = state.get("redirection_activated") {
            self.redirection_activated = data.get(0).copied().unwrap_or(0) != 0;
        }

        Ok(())
    }
    /// Post-process after loading
    pub fn load_post_process(&mut self) -> GameResult<()> {
        self.base.load_post_process()
    }
}

impl Snapshotable for OverlordContain {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(&self.base, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_bool(&mut self.redirection_activated)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl ContainModuleInterface for OverlordContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj.read() {
                return self.is_valid_container_for(&*obj_guard, true);
            }
        }
        false
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = TheGameLogic::find_object_by_id(object_id)
            .ok_or_else(|| format!("Contain object {} not found", object_id))?;
        self.add_object(obj).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = match TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return Ok(()),
        };
        self.remove_object(obj).map_err(|e| e.to_string())
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        ContainModuleInterface::get_contained_objects(&self.base)
    }

    fn get_contained_count(&self) -> usize {
        self.get_contain_count() as usize
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let max = self.get_contain_max();
        if max < 0 {
            usize::MAX
        } else {
            max as usize
        }
    }

    fn get_container_pips_to_show(&self) -> (i32, i32, bool) {
        self.get_container_pips_to_show()
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

    fn on_owner_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OverlordContain::on_object_created(self).map_err(|e| e.into())
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        OverlordContain::update(self).map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.base.on_damage(damage_info).map_err(|e| e.into())
    }

    fn on_body_damage_state_change(
        &mut self,
        damage_info: &DamageInfo,
        old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OverlordContain::on_body_damage_state_change(self, damage_info, old_state, new_state)
            .map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OverlordContain::on_die(self, damage_info).map_err(|e| e.into())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        OverlordContain::is_valid_container_for(self, obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain_object(obj.get_id()).map_err(|e| e.into())
    }

    fn add_to_contain_list(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_id = obj.get_id();
        let obj = TheGameLogic::find_object_by_id(object_id)
            .ok_or_else(|| format!("Contain object {} not found", object_id))?;
        OverlordContain::add_to_contain_list(self, obj).map_err(|e| e.into())
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.base.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_containing(
        &mut self,
        obj_id: ObjectID,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        OverlordContain::on_containing(self, obj_id, was_selected).map_err(|e| e.into())
    }

    fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<&Arc<RwLock<Player>>>,
        new_owner: Option<&Arc<RwLock<Player>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OverlordContain::on_capture(self, owner, old_owner, new_owner).map_err(|e| e.into())
    }

    fn on_removing(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        OverlordContain::on_removing(self, obj_id).map_err(|e| e.into())
    }

    fn is_special_overlord_style_container(&self) -> bool {
        self.is_special_overlord_style_container()
    }

    fn remove_all_contained(
        &mut self,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OverlordContain::remove_all_contained(self, expose_stealth).map_err(|e| e.into())
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .base
            .harm_and_force_exit_all_contained(damage_info)
            .map_err(|e| e.into())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OverlordContain::kill_all_contained(self).map_err(|e| e.into())
    }

    fn is_displayed_on_control_bar(&self) -> bool {
        OverlordContain::is_displayed_on_control_bar(self)
    }

    fn is_garrisonable(&self) -> bool {
        OverlordContain::is_garrisonable(self)
    }

    fn is_heal_contain(&self) -> bool {
        OverlordContain::is_heal_contain(self)
    }

    fn is_immune_to_clear_building_attacks(&self) -> bool {
        OverlordContain::is_immune_to_clear_building_attacks(self)
    }

    fn is_kick_out_on_capture(&self) -> bool {
        OverlordContain::is_kick_out_on_capture(self)
    }

    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        OverlordContain::is_passenger_allowed_to_fire(self, id)
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.base.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
    }

    fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        OverlordContain::is_enclosing_container_for(self, obj)
    }

    fn client_visible_contained_flash_as_selected(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OverlordContain::client_visible_contained_flash_as_selected(self).map_err(|e| e.into())
    }

    fn friend_get_rider(&self) -> Option<ObjectID> {
        self.friend_get_rider()
            .and_then(|rider| rider.read().ok().map(|guard| guard.get_id()))
    }
}

impl ContainerInterface for OverlordContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.is_valid_container_for(obj, true)
    }

    fn add_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.add_to_contain(obj)
    }

    fn remove_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.remove_from_contain(obj, false)
    }

    fn get_usage(&self) -> (u32, u32) {
        let current = self.get_contain_count();
        let max = match self.get_contain_max() {
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
    use game_engine::common::system::{XferBlockSize, XferMode, XferStatus};
    use std::io;

    struct RecordingXfer {
        bytes: Vec<u8>,
    }

    impl RecordingXfer {
        fn new() -> Self {
            Self { bytes: Vec::new() }
        }
    }

    impl Xfer for RecordingXfer {
        fn get_xfer_mode(&self) -> XferMode {
            XferMode::Save
        }

        fn get_identifier(&self) -> &str {
            "overlord-contain-test"
        }

        fn set_options(&mut self, _options: u32) {}

        fn clear_options(&mut self, _options: u32) {}

        fn get_options(&self) -> u32 {
            0
        }

        fn open(&mut self, _identifier: &str) -> Result<(), XferStatus> {
            Ok(())
        }

        fn close(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
            Ok(0)
        }

        fn end_block(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn skip(&mut self, _data_size: i32) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_snapshot(
            &mut self,
            _snapshot: &mut game_engine::system::Snapshot,
        ) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_ascii_string(&mut self, _ascii_string_data: &mut String) -> io::Result<()> {
            Ok(())
        }

        fn xfer_unicode_string(&mut self, _unicode_string_data: &mut String) -> io::Result<()> {
            Ok(())
        }

        unsafe fn xfer_implementation(
            &mut self,
            data: *mut u8,
            data_size: usize,
        ) -> io::Result<()> {
            let bytes = unsafe { std::slice::from_raw_parts(data, data_size) };
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
    }

    #[test]
    fn test_overlord_module_data_defaults() {
        let module_data = OverlordContainModuleData::default();
        assert_eq!(module_data.experience_sink_for_rider, true);
        assert!(module_data.payload_template_names.is_empty());
    }

    #[test]
    fn test_overlord_is_special_container() {
        // Would require mock object setup
        // let overlord = OverlordContain::new(...).unwrap();
        // assert!(overlord.is_special_overlord_style_container());
    }

    #[test]
    fn overlord_payload_created_uses_inherited_transport_state_like_cpp() {
        let mut contain = OverlordContain::new(Weak::new(), &OverlordContainModuleData::default())
            .expect("overlord contain constructs");
        contain.base.set_payload_created(true);

        let state = contain.save_state().expect("overlord saves state");

        assert_eq!(state.get("payload_created"), Some(&vec![1]));
        assert_eq!(state.get("redirection_activated"), Some(&vec![0]));
    }

    #[test]
    fn xfer_writes_transport_state_before_overlord_redirection_like_cpp() {
        let mut contain = OverlordContain::new(Weak::new(), &OverlordContainModuleData::default())
            .expect("overlord contain constructs");
        contain.base.set_payload_created(true);
        contain.redirection_activated = true;

        let mut xfer = RecordingXfer::new();
        Snapshotable::xfer(&mut contain, &mut xfer).expect("overlord xfer succeeds");

        assert_eq!(xfer.bytes[0], 1, "OverlordContain xfer version");
        assert_eq!(xfer.bytes[1], 1, "delegated TransportContain xfer version");
        let redirection_bytes: [u8; 4] = xfer.bytes[xfer.bytes.len() - 4..]
            .try_into()
            .expect("redirection bool bytes");
        assert_eq!(
            u32::from_le_bytes(redirection_bytes),
            1,
            "C++ xfers m_redirectionActivated after TransportContain state"
        );
    }
}
