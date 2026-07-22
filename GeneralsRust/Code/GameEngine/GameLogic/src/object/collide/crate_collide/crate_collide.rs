//! Abstract base class for crate collision behaviors
//!
//! This module implements the base functionality for all crate types that can
//! be collected by units in the game. Each crate type extends this base class
//! to implement its specific behavior when collected.

use super::super::collide_module::{CollideModule, CollideModuleData, CollideModuleInterface};
use super::super::{CollisionError, Coord3D, GameError, GameObject, ObjectStatusMask};
use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::common::*;
use crate::helpers::{TheAudio, TheFXListStore, TheGameLogic, TheInGameUI};
use crate::object::collide::{crate_collide::*, LegacyCollideAdapter};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::player::{player_list, PlayerIndex, PlayerType};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::get_anim2d_collection;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

/// Kind-of mask type for object classification
pub type KindOfMaskType = u64;

/// Configuration data for crate collision modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateCollideModuleData {
    /// Base collision module data
    pub base: CollideModuleData,
    /// The kind(s) of units that can collide with this crate
    pub required_kind_of: KindOfMaskType,
    /// The kind(s) of units that CANNOT collide with this crate
    pub forbidden_kind_of: KindOfMaskType,
    /// This crate cannot be picked up by the player of the dead thing that made it
    pub is_forbid_owner_player: bool,
    /// This crate can be picked up by a building (bypassing AI requirement)
    pub is_building_pickup: bool,
    /// Can this crate only be picked up by a human player?
    pub is_human_only_pickup: bool,
    /// Science required to pick up this crate
    pub pickup_science: ScienceType,
    /// FX list to play when activated
    pub execute_fx: Option<String>, // In real implementation, this would be FXList
    /// Animation template to play at crate location
    pub execution_animation_template: String,
    /// Time to play animation for
    pub execute_animation_display_time_seconds: f32,
    /// Rise animation up while playing
    pub execute_animation_z_rise_per_second: f32,
    /// Animation fades out
    pub execute_animation_fades: bool,
}

impl CrateCollideModuleData {
    pub fn new() -> Self {
        Self {
            base: CollideModuleData::new(),
            required_kind_of: 0,
            forbidden_kind_of: 0,
            is_forbid_owner_player: false,
            is_building_pickup: false,
            is_human_only_pickup: false,
            pickup_science: SCIENCE_INVALID,
            execute_fx: None,
            execution_animation_template: String::new(),
            execute_animation_display_time_seconds: 0.0,
            execute_animation_z_rise_per_second: 0.0,
            execute_animation_fades: true,
        }
    }

    pub fn build_field_parse() -> Vec<FieldParse> {
        vec![
            FieldParse::new("RequiredKindOf", FieldType::UnsignedInt, "required_kind_of"),
            FieldParse::new(
                "ForbiddenKindOf",
                FieldType::UnsignedInt,
                "forbidden_kind_of",
            ),
            FieldParse::new(
                "ForbidOwnerPlayer",
                FieldType::Int,
                "is_forbid_owner_player",
            ),
            FieldParse::new("BuildingPickup", FieldType::Int, "is_building_pickup"),
            FieldParse::new("HumanOnly", FieldType::Int, "is_human_only_pickup"),
            FieldParse::new("PickupScience", FieldType::Science, "pickup_science"),
            FieldParse::new("ExecuteFX", FieldType::String, "execute_fx"),
            FieldParse::new(
                "ExecuteAnimation",
                FieldType::String,
                "execution_animation_template",
            ),
            FieldParse::new(
                "ExecuteAnimationTime",
                FieldType::Real,
                "execute_animation_display_time_seconds",
            ),
            FieldParse::new(
                "ExecuteAnimationZRise",
                FieldType::Real,
                "execute_animation_z_rise_per_second",
            ),
            FieldParse::new(
                "ExecuteAnimationFades",
                FieldType::Int,
                "execute_animation_fades",
            ),
        ]
    }
}

impl Default for CrateCollideModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// Base crate collision module state
#[derive(Debug)]
struct CrateCollideState {
    /// Whether the crate has been collected
    is_collected: bool,
    /// Time when the crate was created
    creation_time: u64,
}

/// Abstract base trait for crate collision behaviors
pub trait CrateCollideBehavior: Send + Sync {
    /// Execute the crate's specific behavior when collected
    fn execute_crate_behavior(&mut self, other: &dyn GameObject) -> Result<bool, CollisionError>;

    /// Check if this crate is valid to be executed by the given object
    fn is_valid_to_execute(&self, other: &dyn GameObject) -> bool;
}

/// Base crate collision module implementation
pub struct CrateCollide {
    /// Base collision module
    base_module: CollideModule,
    /// Crate-specific configuration
    module_data: CrateCollideModuleData,
    /// Thread-safe state
    state: Arc<Mutex<CrateCollideState>>,
    /// Handle to the owning object when available
    object_handle: Option<Arc<RwLock<Object>>>,
}

impl fmt::Debug for CrateCollide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CrateCollide")
            .field("object_id", &self.base_module.get_object_id())
            .finish()
    }
}

impl CrateCollide {
    pub fn new(object_id: ObjectId, module_data: CrateCollideModuleData) -> Self {
        Self {
            base_module: CollideModule::new(object_id, module_data.base.clone()),
            module_data,
            state: Arc::new(Mutex::new(CrateCollideState {
                is_collected: false,
                creation_time: TheGameLogic::get_frame() as u64,
            })),
            object_handle: OBJECT_REGISTRY.get_object(object_id),
        }
    }

    /// Legacy helper that constructs the crate from an object handle. Matches the C++ pattern
    /// where crate modules were handed the Thing pointer directly.
    pub fn from_object_handle(
        thing: Arc<RwLock<Object>>,
        module_data: CrateCollideModuleData,
    ) -> Self {
        let object_id = thing
            .read()
            .map(|obj| obj.get_id())
            .unwrap_or(crate::common::INVALID_ID);
        Self {
            base_module: CollideModule::new(object_id, module_data.base.clone()),
            module_data,
            state: Arc::new(Mutex::new(CrateCollideState {
                is_collected: false,
                creation_time: TheGameLogic::get_frame() as u64,
            })),
            object_handle: Some(thing),
        }
    }

    pub fn get_module_data(&self) -> &CrateCollideModuleData {
        &self.module_data
    }

    pub fn get_object(&self) -> Result<Arc<RwLock<Object>>, CollisionError> {
        if let Some(handle) = &self.object_handle {
            return Ok(handle.clone());
        }

        OBJECT_REGISTRY
            .get_object(self.base_module.get_object_id())
            .ok_or_else(|| {
                CollisionError::InvalidObject("crate collide object handle unavailable".to_string())
            })
    }

    pub fn is_collected(&self) -> Result<bool, CollisionError> {
        let state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        Ok(state.is_collected)
    }

    pub fn get_creation_time(&self) -> Result<u64, CollisionError> {
        let state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        Ok(state.creation_time)
    }

    pub fn set_collected(&self, collected: bool) -> Result<(), CollisionError> {
        let mut state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        state.is_collected = collected;
        Ok(())
    }

    /// Check if the given object is valid to execute this crate
    pub fn is_valid_to_execute(&self, other: &dyn GameObject) -> bool {
        // Ground never picks up a crate (handled by None check in caller)

        // Nothing Neutral can pick up any type of crate
        if self.is_neutral_controlled(other) {
            return false;
        }

        let is_building = self.is_kind_of(other, KINDOF_STRUCTURE);
        let valid_building_attempt = self.module_data.is_building_pickup && is_building;

        // Must be a "Unit" type thing with AI, or a valid building
        if !self.has_ai_update_interface(other) && !valid_building_attempt {
            return false;
        }

        // Must match our kindof flags (if any)
        if !self.is_kind_of_multi(
            other,
            self.module_data.required_kind_of,
            self.module_data.forbidden_kind_of,
        ) {
            return false;
        }

        if other.is_effectively_dead() {
            return false;
        }

        // Crates cannot be claimed while in the air, except by buildings
        if self.is_crate_above_terrain() && !valid_building_attempt {
            return false;
        }

        // Check owner player restriction
        if self.module_data.is_forbid_owner_player {
            if let Some(crate_owner) =
                self.get_controlling_player_for_object(self.base_module.get_object_id())
            {
                if crate_owner == other.get_controlling_player() {
                    return false;
                }
            }
        }

        // Human only restriction
        if self.module_data.is_human_only_pickup {
            if !self.is_human_player(other.get_controlling_player()) {
                return false;
            }
        }

        // Science requirement
        if self.module_data.pickup_science != SCIENCE_INVALID {
            if !self.player_has_science(
                other.get_controlling_player(),
                self.module_data.pickup_science,
            ) {
                return false;
            }
        }

        // Cannot be picked up by parachutes
        if self.is_kind_of(other, KINDOF_PARACHUTE) {
            return false;
        }

        true
    }

    /// Apply sabotage feedback effects
    pub fn do_sabotage_feedback_fx(
        &self,
        other: &dyn GameObject,
        victim_type: SabotageVictimType,
    ) -> Result<(), CollisionError> {
        // In a real implementation, this would:
        // 1. Play appropriate sound effects based on victim type
        // 2. Apply visual feedback (flashing, etc.)
        // 3. Handle special cases like fake buildings

        let mut sound_to_play = match victim_type {
            SabotageVictimType::FakeBuilding => {
                // No additional feedback needed
                return Ok(());
            }
            SabotageVictimType::CommandCenter | SabotageVictimType::Superweapon => {
                TheAudio::get_misc_audio()
                    .sabotage_reset_timer_building
                    .clone()
            }
            SabotageVictimType::DropZone | SabotageVictimType::SupplyCenter => {
                TheAudio::get_misc_audio().money_withdraw.clone()
            }
            _ => TheAudio::get_misc_audio()
                .sabotage_shut_down_building
                .clone(),
        };
        let position = other.get_position();
        sound_to_play.set_position(&(position.x, position.y, position.z));
        if let Some(audio) = TheAudio::get() {
            audio.add_audio_event(&sound_to_play);
        }

        // Flash the object as selected
        self.flash_object_as_selected(other)?;

        Ok(())
    }

    /// Execute the standard crate collection sequence
    pub fn execute_standard_collection<T: CrateCollideBehavior>(
        &self,
        behavior: &mut T,
        other: &dyn GameObject,
    ) -> Result<bool, CollisionError> {
        if !self.is_valid_to_execute(other) {
            return Ok(false);
        }

        let succeeded = behavior.execute_crate_behavior(other)?;
        self.finish_execution_attempt(other, succeeded)?;
        Ok(succeeded)
    }

    /// Runs the common C++ post-attempt sequence after a valid crate collide.
    ///
    /// C++ plays configured execution animation for every valid execution attempt,
    /// even if the derived behavior returns false. FX and destruction only happen
    /// on successful behavior execution.
    pub fn finish_execution_attempt(
        &self,
        other: &dyn GameObject,
        succeeded: bool,
    ) -> Result<(), CollisionError> {
        if succeeded {
            self.finalize_collection(other)
        } else {
            self.play_execution_animation()
        }
    }

    /// Runs the common C++ post-collection sequence after a derived crate behavior succeeds.
    pub fn finalize_collection(&self, other: &dyn GameObject) -> Result<(), CollisionError> {
        let crate_position = self.get_object_position()?;

        if let Some(ref fx_name) = self.module_data.execute_fx {
            self.play_fx_at_object(fx_name, other)?;
        }

        self.set_collected(true)?;
        self.destroy_crate_object()?;
        self.play_execution_animation_at(&crate_position)?;
        Ok(())
    }

    pub fn play_execution_animation(&self) -> Result<(), CollisionError> {
        let crate_position = self.get_object_position()?;
        self.play_execution_animation_at(&crate_position)
    }

    fn play_execution_animation_at(&self, position: &Coord3D) -> Result<(), CollisionError> {
        if !self.module_data.execution_animation_template.is_empty() {
            self.play_world_animation(
                &self.module_data.execution_animation_template,
                position,
                self.module_data.execute_animation_display_time_seconds,
                self.module_data.execute_animation_z_rise_per_second,
                self.module_data.execute_animation_fades,
            )?;
        }

        Ok(())
    }

    // Helper methods that would interface with the game engine
    fn is_neutral_controlled(&self, _other: &dyn GameObject) -> bool {
        _other.get_controlling_player() == PlayerId::NEUTRAL
    }

    fn is_kind_of(&self, _other: &dyn GameObject, _kind: u64) -> bool {
        let Some(handle) = _other.as_object_handle() else {
            return false;
        };
        let Ok(guard) = handle.read() else {
            return false;
        };
        if _kind == KINDOF_STRUCTURE {
            return guard.is_kind_of(KindOf::Structure);
        }
        if _kind == KINDOF_PARACHUTE {
            return guard.test_status(ObjectStatusTypes::Parachuting);
        }
        false
    }

    fn is_kind_of_multi(&self, _other: &dyn GameObject, _required: u64, _forbidden: u64) -> bool {
        let Some(handle) = _other.as_object_handle() else {
            return _required == 0 && _forbidden == 0;
        };
        let Ok(guard) = handle.read() else {
            return _required == 0 && _forbidden == 0;
        };
        guard.is_kind_of_multi(_required, _forbidden)
    }

    fn has_ai_update_interface(&self, _other: &dyn GameObject) -> bool {
        let Some(handle) = _other.as_object_handle() else {
            return false;
        };
        let Ok(guard) = handle.read() else {
            return false;
        };
        guard.get_ai_update_interface().is_some()
    }

    fn is_crate_above_terrain(&self) -> bool {
        let Ok(handle) = self.get_object() else {
            return false;
        };
        handle
            .read()
            .map(|guard| guard.is_above_terrain())
            .unwrap_or(false)
    }

    fn get_controlling_player_for_object(&self, _object_id: ObjectId) -> Option<PlayerId> {
        OBJECT_REGISTRY
            .with_object(_object_id, |obj| obj.get_player_id())
            .flatten()
    }

    fn is_human_player(&self, _player_id: PlayerId) -> bool {
        let index = _player_id.value() as PlayerIndex;
        let Ok(list) = player_list().read() else {
            return false;
        };
        let Some(player) = list.get_player(index) else {
            return false;
        };
        let Ok(guard) = player.read() else {
            return false;
        };
        guard.get_player_type() == PlayerType::Human
    }

    fn player_has_science(&self, _player_id: PlayerId, _science: ScienceType) -> bool {
        if _science == SCIENCE_INVALID {
            return true;
        }

        let index = _player_id.value() as PlayerIndex;
        let Ok(list) = player_list().read() else {
            return false;
        };
        let Some(player) = list.get_player(index) else {
            return false;
        };
        let Ok(guard) = player.read() else {
            return false;
        };
        guard.has_science(_science)
    }

    fn flash_object_as_selected(&self, _other: &dyn GameObject) -> Result<(), CollisionError> {
        let Some(handle) = _other.as_object_handle() else {
            return Ok(());
        };
        if let Ok(guard) = handle.read() {
            if let Some(drawable) = guard.get_drawable() {
                if let Ok(mut draw_guard) = drawable.write() {
                    draw_guard.flash_as_selected();
                }
            }
        }
        Ok(())
    }

    fn play_fx_at_object(
        &self,
        _fx_name: &str,
        _other: &dyn GameObject,
    ) -> Result<(), CollisionError> {
        let fx = TheFXListStore::find_fx_list(_fx_name);
        let Some(fx) = fx else {
            return Ok(());
        };
        if let Some(handle) = _other.as_object_handle() {
            let _ = fx.do_fx_obj(&handle, None);
        } else {
            let pos = _other.get_position();
            let world_pos = crate::common::Coord3D::new(pos.x, pos.y, pos.z);
            let _ = fx.do_fx_at_position(&world_pos);
        }
        Ok(())
    }

    fn play_world_animation(
        &self,
        _template: &str,
        _position: &Coord3D,
        _duration: f32,
        _z_rise: f32,
        _fades: bool,
    ) -> Result<(), CollisionError> {
        if !TheGameLogic::get_draw_icon_ui() {
            return Ok(());
        };
        let Some(collection) = get_anim2d_collection() else {
            return Ok(());
        };
        let collection_guard = collection.read();
        if collection_guard
            .find_template(&AsciiString::from(_template))
            .is_none()
        {
            return Ok(());
        }

        let world_pos = crate::common::Coord3D::new(_position.x, _position.y, _position.z);
        TheInGameUI::add_world_animation(_template, &world_pos, _fades, _duration, _z_rise);
        Ok(())
    }

    fn get_object_position(&self) -> Result<Coord3D, CollisionError> {
        let handle = self.get_object()?;
        let guard = handle
            .read()
            .map_err(|_| CollisionError::InvalidObject("Crate object lock poisoned".to_string()))?;
        let pos = *guard.get_position();
        Ok(Coord3D::new(pos.x, pos.y, pos.z))
    }

    fn destroy_crate_object(&self) -> Result<(), CollisionError> {
        let object_id = self.base_module.get_object_id();
        let _ = TheGameLogic::destroy_object_by_id(object_id);
        Ok(())
    }
}

impl CollideModuleInterface for CrateCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        // Base crate collision handling
        if let Some(other_obj) = other {
            if self.is_valid_to_execute(other_obj) {
                // Derived classes should override execute_crate_behavior
                // This is handled by the specific crate implementations
            }
        }
        Ok(())
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        self.is_valid_to_execute(other)
    }
}

// Constants for kind-of flags (would be defined elsewhere in a real implementation)
const KINDOF_STRUCTURE: u64 = 1 << 0;
const KINDOF_PARACHUTE: u64 = 1 << 1;

impl LegacyCollideAdapter for CrateCollide {
    fn legacy_on_collide(
        &mut self,
        other: Arc<RwLock<Object>>,
        loc: &Coord3D,
        normal: &Coord3D,
    ) -> Result<(), GameError> {
        self.on_collide(Some(&other), loc, normal)
            .map_err(GameError::from)
    }

    fn legacy_would_like_to_collide_with(
        &self,
        other: Arc<RwLock<Object>>,
    ) -> Result<bool, GameError> {
        Ok(self.is_valid_to_execute(&other))
    }
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{DefaultThingTemplate, ObjectStatusMaskType};
    use crate::helpers::TheInGameUI;
    use crate::system::game_logic::get_game_logic;
    use game_engine::common::ini::ini_animation::ensure_anim2d_collection;
    use std::sync::OnceLock;

    static CRATE_COLLIDE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn crate_collide_test_guard() -> std::sync::MutexGuard<'static, ()> {
        CRATE_COLLIDE_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("crate collide test lock")
    }

    fn set_logic_frame(frame: u64) {
        get_game_logic()
            .lock()
            .expect("game logic lock")
            .set_current_frame(frame);
    }

    struct LogicFrameReset;

    impl Drop for LogicFrameReset {
        fn drop(&mut self) {
            set_logic_frame(0);
            TheGameLogic::set_draw_icon_ui(true);
            let _ = TheInGameUI::take_world_animations();
        }
    }

    fn register_anim2d_template(name: &str) {
        let collection = ensure_anim2d_collection();
        collection.write().new_template(AsciiString::from(name));
    }

    #[test]
    fn crate_collide_records_current_logic_frame_on_create() {
        let _guard = crate_collide_test_guard();
        set_logic_frame(321);
        let _frame_reset = LogicFrameReset;

        let collide = CrateCollide::new(7, CrateCollideModuleData::default());

        assert_eq!(collide.get_creation_time().expect("creation time"), 321);
    }

    #[test]
    fn crate_collide_from_object_handle_records_current_logic_frame() {
        let _guard = crate_collide_test_guard();
        set_logic_frame(654);
        let _frame_reset = LogicFrameReset;
        let template = Arc::new(DefaultThingTemplate::new("CrateFrameTest".to_string()));
        let object = Object::new_with_id(template, 8, ObjectStatusMaskType::none(), None)
            .expect("crate object");

        let collide = CrateCollide::from_object_handle(object, CrateCollideModuleData::default());

        assert_eq!(collide.get_creation_time().expect("creation time"), 654);
        OBJECT_REGISTRY.unregister_object(8);
    }

    #[test]
    fn crate_collide_execution_animation_queues_ingame_ui_world_animation_like_cpp() {
        let _guard = crate_collide_test_guard();
        set_logic_frame(777);
        let _frame_reset = LogicFrameReset;
        TheGameLogic::set_draw_icon_ui(true);
        let _ = TheInGameUI::take_world_animations();
        register_anim2d_template("CratePickupAnimTest");

        let template = Arc::new(DefaultThingTemplate::new("CrateAnimObjectTest".to_string()));
        let object = Object::new_with_id(template, 9008, ObjectStatusMaskType::none(), None)
            .expect("crate object");
        {
            let mut guard = object.write().expect("object lock");
            guard
                .set_position(&crate::common::Coord3D::new(10.0, 20.0, 3.0))
                .expect("set position");
        }
        let mut data = CrateCollideModuleData::default();
        data.execution_animation_template = "CratePickupAnimTest".to_string();
        data.execute_animation_display_time_seconds = 2.5;
        data.execute_animation_z_rise_per_second = 4.0;
        data.execute_animation_fades = true;
        let collide = CrateCollide::from_object_handle(object, data);

        collide
            .play_execution_animation()
            .expect("execution animation");

        let animations = TheInGameUI::take_world_animations();
        assert_eq!(animations.len(), 1);
        assert_eq!(animations[0].animation_name, "CratePickupAnimTest");
        assert_eq!(
            animations[0].position,
            crate::common::Coord3D::new(10.0, 20.0, 3.0)
        );
        assert!(animations[0].fade_on_expire);
        assert_eq!(animations[0].duration_seconds, 2.5);
        assert_eq!(animations[0].z_rise_per_second, 4.0);
        assert_eq!(animations[0].created_frame, 777);
        OBJECT_REGISTRY.unregister_object(9008);
    }

    #[test]
    fn crate_collide_execution_animation_allows_zero_duration_like_cpp() {
        let _guard = crate_collide_test_guard();
        set_logic_frame(778);
        let _frame_reset = LogicFrameReset;
        TheGameLogic::set_draw_icon_ui(true);
        let _ = TheInGameUI::take_world_animations();
        register_anim2d_template("CratePickupZeroDurationAnimTest");

        let template = Arc::new(DefaultThingTemplate::new(
            "CrateZeroDurationAnimObjectTest".to_string(),
        ));
        let object = Object::new_with_id(template, 9010, ObjectStatusMaskType::none(), None)
            .expect("crate object");
        let mut data = CrateCollideModuleData::default();
        data.execution_animation_template = "CratePickupZeroDurationAnimTest".to_string();
        data.execute_animation_display_time_seconds = 0.0;
        let collide = CrateCollide::from_object_handle(object, data);

        collide
            .play_execution_animation()
            .expect("execution animation");

        let animations = TheInGameUI::take_world_animations();
        assert_eq!(animations.len(), 1);
        assert_eq!(
            animations[0].animation_name,
            "CratePickupZeroDurationAnimTest"
        );
        assert_eq!(animations[0].duration_seconds, 0.0);
        assert_eq!(animations[0].created_frame, 778);
        OBJECT_REGISTRY.unregister_object(9010);
    }

    #[test]
    fn crate_collide_execution_animation_respects_draw_icon_ui_gate_like_cpp() {
        let _guard = crate_collide_test_guard();
        let _frame_reset = LogicFrameReset;
        TheGameLogic::set_draw_icon_ui(false);
        let _ = TheInGameUI::take_world_animations();
        register_anim2d_template("CratePickupDrawIconDisabledTest");

        let template = Arc::new(DefaultThingTemplate::new(
            "CrateDrawIconDisabledObjectTest".to_string(),
        ));
        let object = Object::new_with_id(template, 9009, ObjectStatusMaskType::none(), None)
            .expect("crate object");
        let mut data = CrateCollideModuleData::default();
        data.execution_animation_template = "CratePickupDrawIconDisabledTest".to_string();
        data.execute_animation_display_time_seconds = 1.0;
        let collide = CrateCollide::from_object_handle(object, data);

        collide
            .play_execution_animation()
            .expect("execution animation");

        assert!(TheInGameUI::take_world_animations().is_empty());
        OBJECT_REGISTRY.unregister_object(9009);
    }
}

impl game_engine::common::system::Snapshotable for CrateCollide {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base_module.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // C++ parity: versioned xfer entry point (current version 1).
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        self.base_module.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base_module.load_post_process()
    }
}
