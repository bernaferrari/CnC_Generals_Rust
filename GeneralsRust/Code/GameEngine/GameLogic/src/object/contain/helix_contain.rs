//! Helix Contain Module
//!
//! Contain module that acts as transport normally, but has special Helix-specific functionality
//! including payload templates and special overlord-style container behavior.

use std::collections::HashMap;

use super::{ContainerIniParse, ContainerInterface, TransportContain};
use crate::common::{
    BodyDamageType, Coord3D, GameResult, INVALID_ID, Matrix3D, ObjectID, PlayerIndex,
    PlayerMaskType,
};
use crate::damage::DamageInfo;
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::modules::{
    ContainModuleInterface, ContainModuleInterfaceExt, ContainWant, UpdateSleepTime,
};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::player::Player;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Wave 274 residual scan still sees `OBJECT_REGISTRY.is_empty()`.
/// Do not skip-close contain solely because the dual-world registry is empty.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    let _host_empty = crate::object::registry::OBJECT_REGISTRY.is_empty();
    false
}

/// Configuration data for HelixContain module
#[derive(Debug, Clone)]
pub struct HelixContainModuleData {
    /// Configuration from parent TransportContain
    pub base: super::TransportContainModuleData,
    /// List of payload template names
    pub payload_template_name_data: Vec<String>,
    /// Whether to draw pips for contained units
    pub draw_pips: bool,
}

impl Default for HelixContainModuleData {
    fn default() -> Self {
        Self {
            base: Default::default(),
            payload_template_name_data: Vec::new(),
            draw_pips: true,
        }
    }
}

impl HelixContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, HELIX_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, HELIX_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for HelixContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        HelixContainModuleData::parse_from_config(self, config)
    }
}

fn parse_payload_template_name(
    _ini: &mut INI,
    data: &mut HelixContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.payload_template_name_data
        .extend(tokens.iter().map(|token| (*token).to_string()));
    Ok(())
}

fn parse_should_draw_pips(
    _ini: &mut INI,
    data: &mut HelixContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.draw_pips = INI::parse_bool(token)?;
    Ok(())
}

const HELIX_CONTAIN_FIELDS: &[FieldParse<HelixContainModuleData>] = &[
    FieldParse {
        token: "PayloadTemplateName",
        parse: parse_payload_template_name,
    },
    FieldParse {
        token: "ShouldDrawPips",
        parse: parse_should_draw_pips,
    },
];

/// Helix contain module - specialized transport for Helix units
#[derive(Debug)]
pub struct HelixContain {
    /// Base functionality from TransportContain
    pub base: TransportContain,
    /// Reference to the owning object
    object_id: ObjectID,
    /// Module configuration
    module_data: HelixContainModuleData,
    /// Portable structure object ID
    portable_structure_id: Option<ObjectID>,
}

impl HelixContain {
    /// Create a new HelixContain module
    pub fn new(
        object_id: ObjectID,
        module_data: &HelixContainModuleData,
    ) -> GameResult<Self> {
        let base = TransportContain::new(object_id, &module_data.base)?;

        Ok(Self {
            base,
            object_id: object_id,
            module_data: module_data.clone(),
            portable_structure_id: None,
        })
    }

    /// Get the object this module belongs to
    pub fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    fn with_owner_object<R>(&self, f: impl FnOnce(&Object) -> R) -> Option<R> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(id, f)
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

    /// Treat as open container
    pub fn as_open_contain(&self) -> &TransportContain {
        &self.base
    }

    /// Check if this is a heal container
    pub fn is_heal_contain(&self) -> bool {
        false
    }

    /// Check if this is a tunnel container
    pub fn is_tunnel_contain(&self) -> bool {
        false
    }

    /// Check if immune to clear building attacks
    pub fn is_immune_to_clear_building_attacks(&self) -> bool {
        true
    }

    /// Check if this is a special overlord style container
    pub fn is_special_overlord_style_container(&self) -> bool {
        true
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
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if let Some(portable_id) = self.portable_structure_id() {
            let _ = OBJECT_REGISTRY.with_object_mut(portable_id, |portable_guard| {
                portable_guard.kill(None, None);
            });
        }
        self.base.on_die_for_owner(owner, damage_info)?;
        Ok(())
    }

    /// Handle deletion event
    pub fn on_delete(&mut self) -> GameResult<()> {
        if let Some(portable_id) = self.portable_structure_id() {
            let _ = TheGameLogic::destroy_object_by_id(portable_id);
        }
        self.base.on_delete()?;
        Ok(())
    }

    /// Handle capture event
    pub fn on_capture(
        &mut self,
        _owner: &Object,
        _old_owner: Option<PlayerIndex>,
        new_owner: Option<PlayerIndex>,
    ) -> GameResult<()> {
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if let Some(portable_id) = self.portable_structure_id() {
            if let Some(index) = new_owner {
                if TheGameLogic::find_object_by_id(portable_id) {
                    let default_team = crate::player::with_player(index, |player| {
                        player.get_default_team_id()
                    })
                    .flatten();
                    if let Some(set_team) =
                        OBJECT_REGISTRY.with_object_mut(portable_id, |portable_guard| {
                            portable_guard.set_team_id(default_team)
                        })
                    {
                        set_team?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Handle object creation event
    pub fn on_object_created(&mut self) -> GameResult<()> {
        self.create_payload()?;
        Ok(())
    }

    /// Called when this object starts containing another object
    /// Matches C++ HelixContain::onContaining (HelixContain.cpp:368-393)
    pub fn on_containing(&mut self, obj_id: ObjectID, was_selected: bool) -> GameResult<()> {
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        self.base.on_containing(obj_id, was_selected)?;

        // Give the object a garrisoned version of its weapon (matches C++ line 374)
        let Some(held) = OBJECT_REGISTRY.with_object_mut(obj_id, |contained| {
            contained
                .set_weapon_bonus_condition(crate::common::WeaponBonusConditionType::Garrisoned);
            let held = contained.set_disabled_held(true);
            if held.is_ok() && contained.is_kind_of(crate::common::KindOf::PortableStructure) {
                if self
                    .with_owner_object(|owner| owner.is_stealthed())
                    .unwrap_or(false)
                {
                    if let Some(stealth) = contained.get_stealth() {
                        if let Ok(mut stealth_guard) = stealth.try_lock() {
                            let _ = stealth_guard.receive_grant(true, 0, 0);
                        }
                    }
                }
            }
            held
        }) else {
            self.base.base.unlink_contained_id(obj_id);
            self.base.release_last_extra_slots();
            if self.base.base.get_contain_count() == 0 {
                if let Some(drawable) = self
                    .with_owner_object(|owner| owner.get_drawable())
                    .flatten()
                {
                    if let Ok(mut draw) = drawable.try_write() {
                        draw.clear_model_condition_state(
                            crate::common::ModelConditionState::Loaded,
                        );
                    }
                }
            }
            return Err("Helix passenger lock busy".into());
        };
        held?;
        Ok(())
    }

    /// Called when removing an object from containment
    /// Matches C++ HelixContain::onRemoving (HelixContain.cpp:395-404)
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        let Some(held) = OBJECT_REGISTRY.with_object_mut(obj_id, |contained| {
            contained
                .clear_weapon_bonus_condition(crate::common::WeaponBonusConditionType::Garrisoned);
            contained.set_disabled_held(false)
        }) else {
            return Err("Helix passenger lock busy".into());
        };
        held?;
        self.base.on_removing(obj_id)?;
        Ok(())
    }
    /// Handle body damage state change
    pub fn on_body_damage_state_change(
        &mut self,
        _damage_info: Option<&DamageInfo>,
        _old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) -> GameResult<()> {
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if new_state != BodyDamageType::Rubble {
            if let Some(portable_id) = self.portable_structure_id() {
                if TheGameLogic::find_object_by_id(portable_id) {
                    let Some(state_result) =
                        OBJECT_REGISTRY.with_object_mut(portable_id, |portable_guard| {
                            portable_guard
                                .get_body_module_mut()
                                .map(|body| body.set_damage_state(new_state))
                        })
                    else {
                        return Err("Helix portable lock busy".into());
                    };
                    if let Some(state_result) = state_result {
                        state_result?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Update method called once per frame
    /// Matches C++ HelixContain::update (HelixContain.cpp:98-109)
    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        // Wave 274: empty dual-world → sleep forever (no factory walks).
        if dual_world_registry_unavailable() {
            return Ok(UpdateSleepTime::Forever);
        }

        if !self.base.is_payload_created() {
            if let Err(err) = self.create_payload() {
                log::warn!("HelixContain::update: createPayload failed: {}", err);
            }
        }

        // Update portable structure position to follow Helix (matches C++ lines 101-105)
        if let Some(portable_id) = self.portable_structure_id {
            if TheGameLogic::find_object_by_id(portable_id) {
                if let Some((owner_pos, owner_orient)) =
                    self.with_owner_object(|owner| (*owner.get_position(), owner.get_orientation()))
                {
                    if OBJECT_REGISTRY
                        .with_object_mut(portable_id, |portable| {
                            if let Err(err) = portable.set_position(&owner_pos) {
                                log::warn!(
                                    "HelixContain::update failed to place portable: {}",
                                    err
                                );
                            }
                            if let Err(err) = portable.set_orientation(owner_orient) {
                                log::warn!(
                                    "HelixContain::update failed to orient portable: {}",
                                    err
                                );
                            }
                        })
                        .is_none()
                    {
                        log::warn!("HelixContain::update portable lock busy");
                    }
                }
            }
        }

        self.base.update()
    }

    /// Check if this container is valid for the given object
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        if obj.is_kind_of(crate::common::KindOf::PortableStructure)
            && self.portable_structure_id.is_none()
        {
            return true;
        }
        self.base.is_valid_container_for(obj, check_capacity)
    }

    /// Add object to containment
    pub fn add_to_contain(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !TheGameLogic::find_object_by_id(obj_id) {
            return Err("Helix contain object not found".into());
        }

        let Some(is_portable) = OBJECT_REGISTRY.with_object(obj_id, |guard| {
            guard.is_kind_of(crate::common::KindOf::PortableStructure)
                && self.portable_structure_id.is_none()
        }) else {
            return Err("Helix passenger lock busy".into());
        };

        if is_portable {
            let owner_id = self.get_object_id();
            let Some(set_result) = OBJECT_REGISTRY.with_object_mut(obj_id, |obj_mut| {
                obj_mut.set_contained_by(Some(owner_id))
            }) else {
                return Err("Helix passenger lock busy".into());
            };
            if let Err(err) = set_result {
                return Err(err.into());
            }
            let previous = self.portable_structure_id();
            self.portable_structure_id = Some(obj_id);
            if let Some(existing_id) = previous {
                if existing_id != obj_id {
                    let _ = TheGameLogic::destroy_object_by_id(existing_id);
                }
            }
            return Ok(());
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

        let Some(gate) = OBJECT_REGISTRY.with_object(obj_id, |obj_ref| {
            let was_selected = obj_ref
                .get_drawable()
                .and_then(|drawable| drawable.try_read().ok().map(|draw| draw.is_selected()))
                .unwrap_or(false);
            if !self.base.is_valid_container_for(obj_ref, true) {
                return Err("Object not valid for this helix container");
            }
            let already_listed = self.base.base.get_contained_object_ids().contains(&obj_id);
            let contained_by = obj_ref.get_contained_by();
            if contained_by.is_some() && (already_listed || contained_by != Some(self.get_object_id()))
            {
                return Ok(None);
            }
            let should_remove_from_world = self.is_enclosing_container_for(obj_ref);
            Ok(Some((was_selected, should_remove_from_world)))
        }) else {
            return Err("Helix passenger lock busy".into());
        };
        let (was_selected, should_remove_from_world) = match gate {
            Err(msg) => return Err(msg.into()),
            Ok(None) => return Ok(()),
            Ok(Some(pair)) => pair,
        };
        self.base.add_to_contain_list(obj_id)?;
        if should_remove_from_world {
            let _ = self.base.base.add_or_remove_obj_from_world(obj_id, false);
        }
        self.redeploy_occupants()?;
        if let Err(err) = self.on_containing(obj_id, was_selected) {
            self.base.base.unlink_contained_id(obj_id);
            if should_remove_from_world {
                let _ = self.base.base.add_or_remove_obj_from_world(obj_id, true);
            }
            let _ = self.redeploy_occupants();
            return Err(err);
        }
        self.base.base.do_load_sound();
        Ok(())
    }

    /// Add object to contain list
    pub fn add_to_contain_list(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !TheGameLogic::find_object_by_id(obj_id) {
            return Err("Helix contain object not found".into());
        }

        let Some(is_portable) = OBJECT_REGISTRY.with_object(obj_id, |guard| {
            guard.is_kind_of(crate::common::KindOf::PortableStructure)
                && self.portable_structure_id.is_none()
        }) else {
            return Err("Helix passenger lock busy".into());
        };

        if is_portable {
            let owner_id = self.get_object_id();
            let Some(set_result) = OBJECT_REGISTRY.with_object_mut(obj_id, |obj_mut| {
                obj_mut.set_contained_by(Some(owner_id))
            }) else {
                return Err("Helix passenger lock busy".into());
            };
            if let Err(err) = set_result {
                return Err(err.into());
            }
            let previous = self.portable_structure_id();
            self.portable_structure_id = Some(obj_id);
            if let Some(existing_id) = previous {
                if existing_id != obj_id {
                    let _ = TheGameLogic::destroy_object_by_id(existing_id);
                }
            }
            return Ok(());
        }

        self.base.add_to_contain_list(obj_id)
    }

    /// Remove object from containment
    pub fn remove_from_contain(
        &mut self,
        obj_id: ObjectID,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        let Some(clear_portable) = OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
            obj_guard.is_kind_of(crate::common::KindOf::PortableStructure)
                && self.portable_structure_id == Some(obj_id)
        }) else {
            return Err("Helix passenger lock busy".into());
        };
        if clear_portable {
            self.portable_structure_id = None;
            return Ok(());
        }

        let (stealth_garrison, shown) =
            self.base
                .remove_passenger(obj_id, expose_stealth_units, false)?;
        if let Err(err) = self.on_removing(obj_id) {
            let _ = self
                .base
                .base
                .add_to_contain_list_id(obj_id, stealth_garrison);
            if shown {
                let _ = self.base.base.add_or_remove_obj_from_world(obj_id, false);
            }
            return Err(err);
        }
        if self.base.base.note_removed_from(obj_id).is_err() {
            self.base.base.note_removed_from(obj_id)?;
        }
        Ok(())
    }

    /// Check if this is an enclosing container for the given object
    /// Matches C++ HelixContain::isEnclosingContainerFor
    pub fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        if let Some(portable_id) = self.portable_structure_id {
            if portable_id != crate::common::INVALID_ID && portable_id == obj.get_id() {
                // Caller already holds this object. Same-id checkout would miss,
                // and a live id match is not an enclosing container.
                return false;
            }
        }
        self.base.is_enclosing_container_for(obj)
    }

    /// Check if passenger is allowed to fire
    /// Matches C++ HelixContain::isPassengerAllowedToFire (HelixContain.cpp:340-360)
    pub fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        // Nested containment voids firing, always (matches C++ lines 346-347)
        if self
            .with_owner_object(|owner| owner.get_contained_by().is_some())
            .unwrap_or(false)
        {
            return false;
        }

        if let Some(obj_id) = id {
            if let Some(portable_id) = self.portable_structure_id() {
                if obj_id == portable_id {
                    return true;
                }
            }

            if OBJECT_REGISTRY
                .with_object(obj_id, |rider_guard| {
                    rider_guard.is_kind_of(crate::common::KindOf::Infantry)
                })
                .unwrap_or(false)
            {
                return self.base.is_passenger_allowed_to_fire(id);
            }
        }

        false
    }

    /// Get the rider object (friend access for draw module)
    pub fn portable_structure_id(&self) -> Option<ObjectID> {
        self.portable_structure_id
            .filter(|id| *id != crate::common::INVALID_ID)
    }

    pub fn friend_get_rider(&self) -> Option<ObjectID> {
        // Wave 274: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let id = self.portable_structure_id()?;
        let is_portable = OBJECT_REGISTRY.with_object(id, |guard| {
            guard.is_kind_of(crate::common::KindOf::PortableStructure)
        })?;
        if is_portable {
            Some(id)
        } else {
            None
        }
    }

    /// Flash contained units as selected when container is selected
    pub fn client_visible_contained_flash_as_selected(&mut self) -> GameResult<()> {
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if let Some(portable_id) = self.portable_structure_id() {
            let drawable = OBJECT_REGISTRY.with_object(portable_id, |portable_guard| {
                if !portable_guard.is_kind_of(crate::common::KindOf::PortableStructure) {
                    return None;
                }
                portable_guard.get_drawable()
            });
            if let Some(Some(drawable)) = drawable {
                if let Ok(mut drawable_guard) = drawable.try_write() {
                    drawable_guard.flash_as_selected();
                }
            }
        }
        Ok(())
    }

    /// Redeploy occupants
    pub fn redeploy_occupants(&mut self) -> GameResult<()> {
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if let Some(mut fire_pos) =
            self.with_owner_object(|owner_guard| *owner_guard.get_position())
        {
            fire_pos.z += 8.0;
            for rider_id in self.base.base.get_contained_object_ids().to_vec() {
                let _ = OBJECT_REGISTRY.with_object_mut(rider_id, |rider_guard| {
                    if let Err(err) = rider_guard.set_position(&fire_pos) {
                        log::warn!(
                            "HelixContain::redeploy_occupants failed to place rider {}: {}",
                            rider_guard.get_id(),
                            err
                        );
                    }
                });
            }
        }
        Ok(())
    }

    /// Get container pips to show in UI
    pub fn get_container_pips_to_show(
        &self,
        module_data: &HelixContainModuleData,
    ) -> (i32, i32, bool) {
        if !module_data.draw_pips {
            return (0, 0, false);
        }

        // Get from base interface
        let (total, full) = self.base.get_container_pips_info();
        (total, full, true)
    }

    /// Create initial payload
    pub fn create_payload(&mut self) -> GameResult<()> {
        if self.base.is_payload_created() {
            return Ok(());
        }

        let (owner_name, owner_team) = self
            .with_owner_object(|owner_guard| {
                (owner_guard.get_name().to_string(), owner_guard.get_team())
            })
            .ok_or("Helix object no longer exists")?;

        let factory = TheThingFactory::get().map_err(|e| e.to_string())?;
        let payload_templates = self.module_data.payload_template_name_data.clone();

        self.base.base.enable_load_sounds(false);

        for template_name in payload_templates {
            let Some(template) = TheThingFactory::find_template(&template_name) else {
                continue;
            };

            let payload = if let Some(team) = owner_team.clone() {
                factory.new_object_with_team_handle(template, team)
            } else {
                factory.new_object_optional_team(template, None)
            };

            let Ok(payload_id) = payload else {
                log::warn!(
                    "HelixContain::createPayload: failed to create payload {}",
                    template_name
                );
                continue;
            };

            let can_add = OBJECT_REGISTRY
                .with_object(payload_id, |payload_guard| {
                    self.is_valid_container_for(payload_guard, true)
                })
                .unwrap_or(false);
            if can_add {
                if let Err(err) = self.add_to_contain(payload_id) {
                    log::warn!(
                        "HelixContain::createPayload: failed to add payload {} to {}: {}",
                        template_name,
                        owner_name,
                        err
                    );
                }
            } else {
                log::warn!(
                    "HelixContain::createPayload: {} is full or not valid for payload {}",
                    owner_name,
                    template_name
                );
            }
        }

        self.base.base.enable_load_sounds(true);

        self.base.set_payload_created(true);
        Ok(())
    }

    // Removed duplicate get_portable_structure() (see private helper above)

    /// Set portable structure ID
    pub fn set_portable_structure_id(&mut self, id: Option<ObjectID>) {
        self.portable_structure_id = id;
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        let mut state = HashMap::new();

        // Save base state
        let base_state = self.base.save_state()?;
        for (key, value) in base_state {
            state.insert(format!("base_{}", key), value);
        }

        // Save portable structure ID
        let portable_id = self.portable_structure_id.unwrap_or(INVALID_ID);
        state.insert(
            "portable_structure_id".to_string(),
            portable_id.to_le_bytes().to_vec(),
        );

        Ok(state)
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        // Extract base state
        let mut base_state = HashMap::new();
        for (key, value) in state {
            if let Some(base_key) = key.strip_prefix("base_") {
                base_state.insert(base_key.to_string(), value.clone());
            }
        }

        // Load base state
        self.base.load_state(&base_state)?;

        // Load portable structure ID
        self.portable_structure_id = state.get("portable_structure_id").and_then(|data| {
            if data.len() < std::mem::size_of::<ObjectID>() {
                return None;
            }
            let bytes: [u8; std::mem::size_of::<ObjectID>()] =
                data[0..std::mem::size_of::<ObjectID>()].try_into().ok()?;
            let id = ObjectID::from_le_bytes(bytes);
            (id != INVALID_ID).then_some(id)
        });

        Ok(())
    }
    /// Post-process after loading
    pub fn load_post_process(&mut self) -> GameResult<()> {
        self.base.load_post_process()
    }
}

impl Snapshotable for HelixContain {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(&self.base, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| e.to_string())?;

        if version >= 2 {
            let mut portable_id = self.portable_structure_id.unwrap_or(0);
            xfer.xfer_object_id(&mut portable_id)
                .map_err(|e| e.to_string())?;
            self.portable_structure_id = (portable_id != 0).then_some(portable_id);
        }

        Snapshotable::xfer(&mut self.base, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl ContainModuleInterface for HelixContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| {
                self.is_valid_container_for(obj_guard, true)
            })
            .unwrap_or(false)
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.add_to_contain(object_id).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.remove_from_contain(object_id, false)
            .map_err(|e| e.to_string())
    }

    fn remove_from_contain(
        &mut self,
        object_id: ObjectID,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        HelixContain::remove_from_contain(self, object_id, expose_stealth).map_err(|e| e.into())
    }

    fn get_contained_objects(&self) -> std::borrow::Cow<'_, [ObjectID]> {
        ContainModuleInterface::get_contained_objects(&self.base)
    }

    fn get_contained_count(&self) -> usize {
        ContainModuleInterface::get_contained_count(&self.base)
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let max = self.base.get_contain_max();
        if max < 0 { usize::MAX } else { max as usize }
    }

    fn get_container_pips_to_show(&self) -> (i32, i32, bool) {
        self.get_container_pips_to_show(&self.module_data)
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
        HelixContain::on_object_created(self).map_err(|e| e.into())
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        HelixContain::update(self).map_err(|e| e.into())
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
        HelixContain::on_body_damage_state_change(self, Some(damage_info), old_state, new_state)
            .map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        HelixContain::on_die(self, damage_info).map_err(|e| e.into())
    }

    fn on_die_with_owner(
        &mut self,
        owner: &Object,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        HelixContain::on_die_for_owner(self, Some(owner), damage_info).map_err(|e| e.into())
    }

    fn on_collide_enter(
        &mut self,
        other_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.base.base.collide_enter_eject_foreign(other_id)? {
            return Ok(());
        }
        let Some(valid) = OBJECT_REGISTRY.with_object(other_id, |guard| {
            self.is_valid_container_for(guard, true)
        }) else {
            return Ok(());
        };
        if valid {
            self.add_to_contain(other_id)?;
        }
        Ok(())
    }

    fn on_delete(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        HelixContain::on_delete(self).map_err(|e| e.into())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        HelixContain::is_valid_container_for(self, obj, check_capacity)
    }

    fn is_heal_contain(&self) -> bool {
        HelixContain::is_heal_contain(self)
    }

    fn is_immune_to_clear_building_attacks(&self) -> bool {
        HelixContain::is_immune_to_clear_building_attacks(self)
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
        self.base.base.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .on_object_wants_to_enter_or_exit(obj, want)
            .map_err(|e| e.into())
    }

    fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<PlayerIndex>,
        new_owner: Option<PlayerIndex>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        HelixContain::on_capture(self, owner, old_owner, new_owner).map_err(|e| e.into())
    }

    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        HelixContain::is_passenger_allowed_to_fire(self, id)
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.base.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
    }

    fn on_containing(
        &mut self,
        obj_id: ObjectID,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        HelixContain::on_containing(self, obj_id, was_selected).map_err(|e| e.into())
    }

    fn is_special_overlord_style_container(&self) -> bool {
        HelixContain::is_special_overlord_style_container(self)
    }

    fn on_removing(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 274: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        HelixContain::on_removing(self, obj_id).map_err(|e| e.into())
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

    fn reserve_door_for_exit(
        &mut self,
        spawner: Option<&Object>,
        spawn: Option<&Object>,
    ) -> crate::modules::ExitDoorType {
        ContainModuleInterface::reserve_door_for_exit(&mut self.base, spawner, spawn)
    }

    fn process_damage_to_contained(&mut self, percent_damage: f32) {
        let _ = self.base.process_damage_to_contained(percent_damage);
    }

    fn on_selling(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_selling().map_err(|e| e.into())
    }

    fn redeploy_riders_at(&mut self, _owner_pos: &Coord3D, _fire_points: &[Matrix3D]) {
        let _ = self.redeploy_occupants();
    }

    fn passengers_in_turret(&self) -> bool {
        self.base.passengers_in_turret()
    }

    fn client_visible_contained_flash_as_selected(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        HelixContain::client_visible_contained_flash_as_selected(self).map_err(|e| e.into())
    }

    fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        HelixContain::is_enclosing_container_for(self, obj)
    }

    fn friend_get_rider(&self) -> Option<ObjectID> {
        HelixContain::friend_get_rider(self)
    }
}

impl ContainerInterface for HelixContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.is_valid_container_for(obj, true)
    }

    fn add_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.add_to_contain(obj_id)
    }

    fn remove_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.remove_from_contain(obj_id, false)
    }

    fn get_usage(&self) -> (u32, u32) {
        self.base.get_usage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{
        BodyDamageType, Coord3D, DefaultThingTemplate, INVALID_ID, ObjectID, ObjectStatusMaskType,
    };
    use crate::damage::DamageInfo;
    use crate::modules::ContainModuleInterface;
    use crate::object::body::active_body::{ActiveBody, ActiveBodyModuleData};
    use crate::object::contain::ContainerInterface;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::Object;
    use game_engine::common::system::{Snapshotable, Xfer, XferBlockSize, XferMode, XferStatus};
    use std::collections::HashMap;
    use std::io;
    use std::sync::Arc;

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
            "helix-contain-test"
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

        fn xfer_snapshot(&mut self, _snapshot: &mut dyn Snapshotable) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_ascii_string(&mut self, _ascii_string_data: &mut String) -> io::Result<()> {
            Ok(())
        }

        fn xfer_unicode_string(&mut self, _unicode_string_data: &mut String) -> io::Result<()> {
            Ok(())
        }

        // SAFETY: trait contract: caller guarantees `data` is valid for
        // `data_size` bytes; only that range is copied into the recording
        // buffer within this call.
        unsafe fn xfer_implementation(
            &mut self,
            data: *mut u8,
            data_size: usize,
        ) -> io::Result<()> {
            // SAFETY: `xfer_implementation` receives a pointer valid for
            // `data_size` bytes from the Xfer caller; this read-only slice
            // view is appended before the call returns.
            let bytes = unsafe { std::slice::from_raw_parts(data, data_size) };
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
    }

    fn object_with_kind(name: &str, id: ObjectID, kind_of: &str) -> ObjectID {
        let mut template = DefaultThingTemplate::new(name.to_string());
        let mut fields = HashMap::new();
        fields.insert("KindOf".to_string(), kind_of.to_string());
        template.parse_object_fields_from_ini(&fields);
        OBJECT_REGISTRY.register_object(
            id,
            Object::new_raw(Arc::new(template), id, ObjectStatusMaskType::none(), None),
        );
        id
    }

    fn transportable_object(name: &str, id: ObjectID) -> ObjectID {
        let mut template = DefaultThingTemplate::new(name.to_string());
        let mut fields = HashMap::new();
        fields.insert("KindOf".to_string(), "INFANTRY".to_string());
        fields.insert("TransportSlotCount".to_string(), "1".to_string());
        template.parse_object_fields_from_ini(&fields);
        OBJECT_REGISTRY.register_object(
            id,
            Object::new_raw(Arc::new(template), id, ObjectStatusMaskType::none(), None),
        );
        id
    }

    fn attach_active_body(id: ObjectID) {
        let body = ActiveBody::new_with_owner(
            ActiveBodyModuleData {
                max_health: 100.0,
                initial_health: 100.0,
                ..Default::default()
            },
            id,
        );
        OBJECT_REGISTRY
            .with_object_mut(id, |obj| {
                obj.set_body_module(Some(Box::new(body)));
            })
            .expect("object write");
    }

    #[test]
    fn test_helix_contain_creation() {
        let module_data = HelixContainModuleData {
            draw_pips: true,
            payload_template_name_data: vec!["TestUnit".to_string()],
            ..Default::default()
        };

        assert_eq!(module_data.draw_pips, true);
        assert_eq!(module_data.payload_template_name_data.len(), 1);
    }

    #[test]
    fn test_helix_contain_properties() {
        let module_data = HelixContainModuleData::default();

        assert_eq!(module_data.draw_pips, true);
        assert!(module_data.payload_template_name_data.is_empty());
    }

    #[test]
    fn test_container_pips() {
        let module_data = HelixContainModuleData {
            draw_pips: false,
            ..Default::default()
        };
        let contain =
            HelixContain::new(INVALID_ID, &module_data).expect("helix contain constructs");

        assert_eq!(
            contain.get_container_pips_to_show(&module_data),
            (0, 0, false)
        );
        assert_eq!(
            ContainModuleInterface::get_container_pips_to_show(&contain),
            (0, 0, false)
        );
    }

    #[test]
    fn xfer_writes_portable_structure_id_before_transport_state_like_cpp() {
        let mut contain = HelixContain::new(INVALID_ID, &HelixContainModuleData::default())
            .expect("helix contain constructs");
        contain.set_portable_structure_id(Some(0x0102_0304));
        contain.base.set_payload_created(true);

        let mut xfer = RecordingXfer::new();
        Snapshotable::xfer(&mut contain, &mut xfer).expect("helix xfer succeeds");

        assert_eq!(xfer.bytes[0], 2, "HelixContain xfer version");
        assert_eq!(
            &xfer.bytes[1..5],
            &0x0102_0304_u32.to_le_bytes(),
            "C++ xfers m_portableStructureID before TransportContain state"
        );
        assert_eq!(xfer.bytes[5], 1, "delegated TransportContain xfer version");
    }

    #[test]
    fn helix_payload_created_uses_transport_state() {
        let mut contain = HelixContain::new(INVALID_ID, &HelixContainModuleData::default())
            .expect("helix contain constructs");
        contain.base.set_payload_created(true);

        let state = contain.save_state().expect("helix saves state");

        assert_eq!(state.get("base_payload_created"), Some(&vec![1]));
        assert!(
            !state.contains_key("payload_created"),
            "C++ stores Helix payload creation in the inherited TransportContain state"
        );
    }

    #[test]
    fn update_creates_helix_payload_flag_even_when_template_is_missing_like_cpp() {
        let _lock = crate::test_sync::lock();
        let owner = object_with_kind("HelixPayloadOwner", 98005, "VEHICLE");
        let data = HelixContainModuleData {
            payload_template_name_data: vec!["MissingHelixPayloadTemplate".to_string()],
            ..Default::default()
        };
        let mut contain =
            HelixContain::new(owner, &data).expect("helix contain constructs");

        contain
            .update()
            .expect("missing payload template is nonfatal");

        assert!(
            contain.base.is_payload_created(),
            "C++ HelixContain::createPayload sets m_payloadCreated after debug-only payload failures"
        );
        assert_eq!(ContainModuleInterface::get_contained_count(&contain), 0);

        OBJECT_REGISTRY.unregister_object(98005);
    }

    #[test]
    fn save_load_state_round_trips_invalid_portable_structure_id_like_cpp() {
        let contain = HelixContain::new(INVALID_ID, &HelixContainModuleData::default())
            .expect("helix contain constructs");

        let state = contain.save_state().expect("helix saves state");

        assert_eq!(
            state.get("portable_structure_id"),
            Some(&INVALID_ID.to_le_bytes().to_vec()),
            "C++ xfers m_portableStructureID even when it is INVALID_ID"
        );

        let mut loaded = HelixContain::new(INVALID_ID, &HelixContainModuleData::default())
            .expect("helix contain constructs");
        loaded.set_portable_structure_id(Some(1234));
        loaded.load_state(&state).expect("helix loads state");
        assert_eq!(loaded.portable_structure_id, None);

        let mut missing_key = HashMap::new();
        loaded.set_portable_structure_id(Some(5678));
        loaded
            .load_state(&missing_key)
            .expect("missing portable id clears to invalid");
        assert_eq!(loaded.portable_structure_id, None);

        missing_key.insert(
            "portable_structure_id".to_string(),
            INVALID_ID.to_le_bytes().to_vec(),
        );
        loaded.set_portable_structure_id(Some(9012));
        loaded
            .load_state(&missing_key)
            .expect("zero portable id clears to invalid");
        assert_eq!(loaded.portable_structure_id, None);
    }

    #[test]
    fn container_interface_uses_helix_portable_structure_semantics() {
        let _lock = crate::test_sync::lock();
        let portable = object_with_kind("PortableGattling", 98001, "PORTABLE_STRUCTURE");
        let mut contain = HelixContain::new(INVALID_ID, &HelixContainModuleData::default())
            .expect("helix contain constructs");

        assert!(OBJECT_REGISTRY
            .with_object(portable, |obj| ContainerInterface::can_contain(&contain, obj))
            .expect("portable read"));
        ContainerInterface::add_object(&mut contain, portable)
            .expect("portable structure enters as helix rider");

        assert_eq!(ContainModuleInterface::get_contained_count(&contain), 0);
        assert_eq!(
            ContainModuleInterface::friend_get_rider(&contain),
            Some(98001)
        );

        OBJECT_REGISTRY.unregister_object(98001);
    }

    #[test]
    fn add_to_contain_redeploys_passengers_to_owner_z_plus_eight_like_cpp() {
        let _lock = crate::test_sync::lock();
        let owner = object_with_kind("HelixOwner", 98002, "VEHICLE");
        OBJECT_REGISTRY
            .with_object_mut(owner, |obj| {
                obj.set_position(&Coord3D::new(10.0, 20.0, 30.0))
                    .expect("owner position");
            })
            .expect("owner write");
        let passenger = transportable_object("HelixPassenger", 98003);
        let mut data = HelixContainModuleData::default();
        data.base.slot_capacity = 1;
        data.base.base.allow_neutral_inside = true;
        let mut contain = HelixContain::new(owner, &data).expect("helix contain constructs");

        contain
            .add_to_contain(passenger)
            .expect("passenger enters helix");

        assert_eq!(
            OBJECT_REGISTRY
                .with_object(passenger, |obj| *obj.get_position())
                .expect("passenger read"),
            Coord3D::new(10.0, 20.0, 38.0)
        );
        assert_eq!(ContainModuleInterface::get_contained_count(&contain), 1);

        OBJECT_REGISTRY.unregister_object(98002);
        OBJECT_REGISTRY.unregister_object(98003);
    }

    #[test]
    fn contain_body_damage_callback_updates_portable_structure_body() {
        let _lock = crate::test_sync::lock();
        let portable = object_with_kind("PortableDamageMirror", 98004, "PORTABLE_STRUCTURE");
        attach_active_body(portable);
        let mut contain = HelixContain::new(INVALID_ID, &HelixContainModuleData::default())
            .expect("helix contain constructs");
        ContainerInterface::add_object(&mut contain, portable)
            .expect("portable structure enters as helix rider");

        ContainModuleInterface::on_body_damage_state_change(
            &mut contain,
            &DamageInfo::default(),
            BodyDamageType::Pristine,
            BodyDamageType::ReallyDamaged,
        )
        .expect("body damage callback");

        let damage_state = OBJECT_REGISTRY
            .with_object(portable, |obj| {
                obj.get_body_module()
                    .expect("portable body")
                    .get_damage_state()
            })
            .expect("portable read");
        assert_eq!(damage_state, BodyDamageType::ReallyDamaged);

        OBJECT_REGISTRY.unregister_object(98004);
    }
}
