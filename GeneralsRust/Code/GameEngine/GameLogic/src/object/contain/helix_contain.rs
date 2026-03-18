//! Helix Contain Module
//!
//! Contain module that acts as transport normally, but has special Helix-specific functionality
//! including payload templates and special overlord-style container behavior.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface, TransportContain};
use crate::common::{BodyDamageType, GameResult, ObjectID, PlayerMaskType, ThingFactory};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::modules::{
    ContainModuleInterface, ContainModuleInterfaceExt, ContainWant, UpdateSleepTime,
};
use crate::object::Object;
use crate::player::Player;
use game_engine::common::ini::{FieldParse, INIError, INI};

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
    object: Weak<RwLock<Object>>,
    /// Module configuration
    module_data: HelixContainModuleData,
    /// Portable structure object ID
    portable_structure_id: Option<ObjectID>,
    /// Whether payload has been created
    payload_created: bool,
}

impl HelixContain {
    /// Create a new HelixContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &HelixContainModuleData,
    ) -> GameResult<Self> {
        let base = TransportContain::new(object.clone(), &module_data.base)?;

        Ok(Self {
            base,
            object,
            module_data: module_data.clone(),
            portable_structure_id: None,
            payload_created: false,
        })
    }

    /// Get the object this module belongs to
    pub fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.object.upgrade()
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
        if let Some(portable) = self.get_portable_structure() {
            if let Ok(mut portable_guard) = portable.write() {
                portable_guard.kill(None, None);
            }
        }
        self.base.on_die(damage_info)?;
        Ok(())
    }

    /// Handle deletion event
    pub fn on_delete(&mut self) -> GameResult<()> {
        if let Some(portable) = self.get_portable_structure() {
            if let Ok(portable_guard) = portable.read() {
                let _ = TheGameLogic::destroy_object(&*portable_guard);
            }
        }
        self.base.on_delete()?;
        Ok(())
    }

    /// Handle capture event
    pub fn on_capture(
        &mut self,
        _owner: &Object,
        _old_owner: Option<&Arc<RwLock<Player>>>,
        new_owner: Option<&Arc<RwLock<Player>>>,
    ) -> GameResult<()> {
        if let Some(portable) = self.get_portable_structure() {
            if let (Ok(mut portable_guard), Some(new_owner_arc)) = (portable.write(), new_owner) {
                if let Ok(new_owner_guard) = new_owner_arc.read() {
                    let default_team = new_owner_guard.get_default_team();
                    portable_guard.set_team(default_team)?;
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
    pub fn on_containing(
        &mut self,
        obj: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> GameResult<()> {
        self.base.on_containing(obj.clone(), was_selected)?;

        // Give the object a garrisoned version of its weapon (matches C++ line 374)
        if let Ok(mut contained) = obj.write() {
            contained
                .set_weapon_bonus_condition(crate::common::WeaponBonusConditionType::Garrisoned);
            contained.set_disabled_held(true)?;

            // Handle stealth sharing for portable structures when owner is stealthed
            if contained.is_kind_of(crate::common::KindOf::PortableStructure) {
                if let Some(owner_obj) = self.get_object() {
                    if let Ok(owner) = owner_obj.read() {
                        if owner.is_stealthed() {
                            if let Some(stealth) = contained.get_stealth() {
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

    /// Called when removing an object from containment
    /// Matches C++ HelixContain::onRemoving (HelixContain.cpp:395-404)
    pub fn on_removing(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.base.on_removing(obj.clone())?;

        // Give the object back a regular weapon (matches C++ line 401)
        if let Ok(mut contained) = obj.write() {
            contained
                .clear_weapon_bonus_condition(crate::common::WeaponBonusConditionType::Garrisoned);
            contained.set_disabled_held(false)?;
        }

        Ok(())
    }

    /// Handle body damage state change
    pub fn on_body_damage_state_change(
        &mut self,
        _damage_info: Option<&DamageInfo>,
        _old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) -> GameResult<()> {
        if new_state != BodyDamageType::Rubble {
            if let Some(portable) = self.get_portable_structure() {
                if let Ok(portable_guard) = portable.read() {
                    if let Some(body) = portable_guard.get_body_module() {
                        if let Ok(mut body_guard) = body.lock() {
                            let _ = body_guard.set_damage_state(new_state);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Update method called once per frame
    /// Matches C++ HelixContain::update (HelixContain.cpp:98-109)
    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        // Update portable structure position to follow Helix (matches C++ lines 101-105)
        if let Some(_portable_id) = self.portable_structure_id {
            if let Some(portable_obj) = self.get_portable_structure() {
                if let Some(owner_obj) = self.get_object() {
                    if let Ok(owner) = owner_obj.read() {
                        let owner_pos = *owner.get_position();
                        let owner_orient = owner.get_orientation();
                        drop(owner);

                        if let Ok(mut portable) = portable_obj.write() {
                            if let Err(err) = portable.set_position(&owner_pos) {
                                log::warn!(
                                    "HelixContain::update failed to place portable structure {}: {}",
                                    portable.get_id(),
                                    err
                                );
                            }
                            if let Err(err) = portable.set_orientation(owner_orient) {
                                log::warn!(
                                    "HelixContain::update failed to orient portable structure {}: {}",
                                    portable.get_id(),
                                    err
                                );
                            }
                        }
                    }
                }
            }
        }

        self.base.update()
    }

    /// Get portable structure object
    /// Matches C++ HelixContain::getPortableStructure (HelixContain.cpp:189-192)
    fn get_portable_structure(&self) -> Option<Arc<RwLock<Object>>> {
        if let Some(id) = self.portable_structure_id {
            TheGameLogic::find_object_by_id(id)
        } else {
            None
        }
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
    pub fn add_to_contain(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        let (is_portable, obj_id) = match obj.read() {
            Ok(guard) => (
                guard.is_kind_of(crate::common::KindOf::PortableStructure)
                    && self.portable_structure_id.is_none(),
                guard.get_id(),
            ),
            Err(_) => (false, 0),
        };

        if is_portable {
            if let Some(existing) = self.get_portable_structure() {
                if let Ok(existing_guard) = existing.read() {
                    let _ = TheGameLogic::destroy_object(&*existing_guard);
                }
            }

            self.portable_structure_id = Some(obj_id);
            if let Some(owner_obj) = self.get_object() {
                if let Ok(mut obj_mut) = obj.write() {
                    let _ = obj_mut.set_contained_by(Some(owner_obj));
                }
            }

            return Ok(());
        }

        self.base.add_to_contain(obj)
    }

    /// Add object to contain list
    pub fn add_to_contain_list(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        let (is_portable, obj_id) = match obj.read() {
            Ok(guard) => (
                guard.is_kind_of(crate::common::KindOf::PortableStructure)
                    && self.portable_structure_id.is_none(),
                guard.get_id(),
            ),
            Err(_) => (false, 0),
        };

        if is_portable {
            if let Some(existing) = self.get_portable_structure() {
                if let Ok(existing_guard) = existing.read() {
                    let _ = TheGameLogic::destroy_object(&*existing_guard);
                }
            }

            self.portable_structure_id = Some(obj_id);
            if let Some(owner_obj) = self.get_object() {
                if let Ok(mut obj_mut) = obj.write() {
                    let _ = obj_mut.set_contained_by(Some(owner_obj));
                }
            }

            return Ok(());
        }

        self.base.add_to_contain_list(obj)
    }

    /// Remove object from containment
    pub fn remove_from_contain(
        &mut self,
        obj: Arc<RwLock<Object>>,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        if let Ok(obj_guard) = obj.read() {
            if obj_guard.is_kind_of(crate::common::KindOf::PortableStructure) {
                if let Some(portable_id) = self.portable_structure_id {
                    if obj_guard.get_id() == portable_id {
                        self.portable_structure_id = None;
                        return Ok(());
                    }
                }
            }
        }

        self.base.remove_from_contain(obj, expose_stealth_units)
    }

    /// Check if this is an enclosing container for the given object
    /// Matches C++ HelixContain::isEnclosingContainerFor
    pub fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        if let Some(portable_id) = self.portable_structure_id {
            if portable_id == obj.get_id() {
                if let Some(portable) = TheGameLogic::find_object_by_id(portable_id) {
                    if let Ok(portable_guard) = portable.read() {
                        if portable_guard.get_id() == obj.get_id() {
                            return false;
                        }
                    }
                }
            }
        }
        self.base.is_enclosing_container_for(obj)
    }

    /// Check if passenger is allowed to fire
    /// Matches C++ HelixContain::isPassengerAllowedToFire (HelixContain.cpp:340-360)
    pub fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        // Nested containment voids firing, always (matches C++ lines 346-347)
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if owner.get_contained_by().is_some() {
                    return false;
                }
            }
        }

        if let Some(obj_id) = id {
            if let Some(portable_id) = self.portable_structure_id {
                if obj_id == portable_id {
                    return true;
                }
            }

            if let Some(rider) = TheGameLogic::find_object_by_id(obj_id) {
                if let Ok(rider_guard) = rider.read() {
                    if rider_guard.is_kind_of(crate::common::KindOf::Infantry) {
                        return self.base.is_passenger_allowed_to_fire(id);
                    }
                }
            }
        }

        false
    }

    /// Get the rider object (friend access for draw module)
    pub fn friend_get_rider(&self) -> Option<Arc<RwLock<Object>>> {
        if let Some(portable) = self.get_portable_structure() {
            if let Ok(portable_guard) = portable.read() {
                if portable_guard.is_kind_of(crate::common::KindOf::PortableStructure) {
                    return Some(portable.clone());
                }
            }
        }
        None
    }

    /// Flash contained units as selected when container is selected
    pub fn client_visible_contained_flash_as_selected(&mut self) -> GameResult<()> {
        if let Some(portable) = self.get_portable_structure() {
            if let Ok(portable_guard) = portable.read() {
                if portable_guard.is_kind_of(crate::common::KindOf::PortableStructure) {
                    if let Some(drawable) = portable_guard.get_drawable() {
                        if let Ok(mut drawable_guard) = drawable.write() {
                            drawable_guard.flash_as_selected();
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Redeploy occupants
    pub fn redeploy_occupants(&mut self) -> GameResult<()> {
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner_guard) = owner_obj.read() {
                let mut fire_pos = *owner_guard.get_position();
                fire_pos.z += 8.0;
                if let Ok(items) = self.base.base.get_contained_items_list() {
                    for rider in items {
                        if let Ok(mut rider_guard) = rider.write() {
                            if let Err(err) = rider_guard.set_position(&fire_pos) {
                                log::warn!(
                                    "HelixContain::redeploy_occupants failed to place rider {}: {}",
                                    rider_guard.get_id(),
                                    err
                                );
                            }
                        }
                    }
                }
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
        if self.payload_created {
            return Ok(());
        }

        let owner = self.get_object().ok_or("Helix object no longer exists")?;
        let owner_guard = owner.read().map_err(|_| "Owner lock poisoned")?;

        if let Ok(factory) = ThingFactory::get() {
            if let Some(contain) = owner_guard.get_contain() {
                contain.enable_load_sounds(false)?;

                for template_name in &self.module_data.payload_template_name_data {
                    if let Some(template) = ThingFactory::find_template(template_name) {
                        if let Some(team) = owner_guard.get_team() {
                            if let Ok(team_ref) = team.read() {
                                if let Ok(payload) = factory.new_object(template, &*team_ref) {
                                    if let Ok(payload_ref) = payload.read() {
                                        if contain.is_valid_container_for(&*payload_ref, true) {
                                            contain.add_to_contain(&*payload_ref);
                                        } else {
                                            return Err(format!(
                                                "HelixContain::createPayload: {} is full or not valid for payload {}",
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

        self.payload_created = true;
        Ok(())
    }

    /// Parse initial payload from INI data
    fn parse_initial_payload(&mut self, _template_names: &[String]) -> GameResult<()> {
        // Implementation would parse and create payload objects
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
        if let Some(id) = self.portable_structure_id {
            state.insert(
                "portable_structure_id".to_string(),
                id.to_le_bytes().to_vec(),
            );
        }

        state.insert(
            "payload_created".to_string(),
            vec![if self.payload_created { 1 } else { 0 }],
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
        if let Some(data) = state.get("portable_structure_id") {
            if data.len() >= std::mem::size_of::<ObjectID>() {
                let bytes: [u8; std::mem::size_of::<ObjectID>()] = data
                    [0..std::mem::size_of::<ObjectID>()]
                    .try_into()
                    .map_err(|_| "Invalid portable_structure_id data")?;
                self.portable_structure_id = Some(u32::from_le_bytes(bytes));
            }
        }

        if let Some(data) = state.get("payload_created") {
            self.payload_created = data.get(0).copied().unwrap_or(0) != 0;
        }

        Ok(())
    }
    /// Post-process after loading
    pub fn load_post_process(&mut self) -> GameResult<()> {
        self.base.load_post_process()
    }
}

impl ContainModuleInterface for HelixContain {
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
        self.add_to_contain(obj).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = match TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return Ok(()),
        };
        self.remove_from_contain(obj, false)
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
        let max = self.base.get_contain_max();
        if max < 0 {
            usize::MAX
        } else {
            max as usize
        }
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

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        HelixContain::on_die(self, damage_info).map_err(|e| e.into())
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
        old_owner: Option<&Arc<RwLock<Player>>>,
        new_owner: Option<&Arc<RwLock<Player>>>,
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
        obj: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        HelixContain::on_containing(self, obj, was_selected).map_err(|e| e.into())
    }

    fn is_special_overlord_style_container(&self) -> bool {
        self.is_special_overlord_style_container()
    }

    fn on_removing(
        &mut self,
        obj: Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        HelixContain::on_removing(self, obj).map_err(|e| e.into())
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

    fn client_visible_contained_flash_as_selected(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        HelixContain::client_visible_contained_flash_as_selected(self).map_err(|e| e.into())
    }

    fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        HelixContain::is_enclosing_container_for(self, obj)
    }

    fn friend_get_rider(&self) -> Option<ObjectID> {
        self.friend_get_rider()
            .and_then(|rider| rider.read().ok().map(|guard| guard.get_id()))
    }
}

impl ContainerInterface for HelixContain {
    fn can_contain(&self, obj: &Object) -> bool {
        ContainerInterface::can_contain(&self.base, obj)
    }

    fn add_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.base.add_object(obj.clone())?;
        self.on_containing(obj, false)
    }

    fn remove_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.on_removing(obj.clone())?;
        self.base.remove_object(obj)
    }

    fn get_usage(&self) -> (u32, u32) {
        self.base.get_usage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // Create a dummy helix contain for testing
        // In real implementation, would need proper object reference

        // Test that pips are disabled when draw_pips is false
        // (total, full, should_show) = get_container_pips_to_show()
        // Should return (0, 0, false) when draw_pips is false
    }
}
