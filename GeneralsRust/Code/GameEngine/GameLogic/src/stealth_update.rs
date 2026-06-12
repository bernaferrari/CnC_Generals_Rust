use std::any::Any;
use std::sync::{Arc, Mutex};

use crate::common::{
    Bool, CommandSourceType, Int, KindOf, ObjectID, ObjectStatusMaskType, ObjectStatusTypes, Real,
    UnsignedInt, FROM_CENTER_2D,
};
use crate::helpers::{
    game_client_random_value_real, game_logic_random_value, TheGameLogic, ThePartitionManager,
};
use crate::modules::{StealthUpdate as StealthUpdateTrait, UPDATE_SLEEP_NONE};
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::{Object, ObjectScriptStatusBit};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::xfer::XferMode;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData, NameKeyType, StealthDisguiseControlInterface, Thing,
};
use log::{trace, warn};
use std::f32::consts::PI;

/// Handle type exposed to gameplay systems for interacting with stealth state.
pub type StealthUpdateHandle = Arc<Mutex<StealthController>>;

const STEALTH_NOT_WHILE_ATTACKING: u32 = 0x00000001;
const STEALTH_NOT_WHILE_MOVING: u32 = 0x00000002;
const STEALTH_NOT_WHILE_USING_ABILITY: u32 = 0x00000004;
const STEALTH_NOT_WHILE_FIRING_PRIMARY: u32 = 0x00000008;
const STEALTH_NOT_WHILE_FIRING_SECONDARY: u32 = 0x00000010;
const STEALTH_NOT_WHILE_FIRING_TERTIARY: u32 = 0x00000020;
const STEALTH_ONLY_WITH_BLACK_MARKET: u32 = 0x00000040;
const STEALTH_NOT_WHILE_TAKING_DAMAGE: u32 = 0x00000080;
const STEALTH_NOT_WHILE_RIDERS_ATTACKING: u32 = 0x00000100;
const STEALTH_NOT_WHILE_FIRING_WEAPON: u32 = STEALTH_NOT_WHILE_FIRING_PRIMARY
    | STEALTH_NOT_WHILE_FIRING_SECONDARY
    | STEALTH_NOT_WHILE_FIRING_TERTIARY;
const NEVER_FRAME: UnsignedInt = u32::MAX;

/// Stealth configuration ported from the legacy StealthUpdateModuleData.
#[derive(Debug, Clone)]
pub struct StealthUpdateModuleData {
    module_tag_name_key: NameKeyType,
    hint_detectable_states: ObjectStatusMaskType,
    required_status: ObjectStatusMaskType,
    forbidden_status: ObjectStatusMaskType,
    stealth_speed: Real,
    friendly_opacity_min: Real,
    friendly_opacity_max: Real,
    reveal_distance_from_target: Real,
    disguise_transition_frames: UnsignedInt,
    disguise_reveal_transition_frames: UnsignedInt,
    pulse_frames: UnsignedInt,
    stealth_delay_frames: UnsignedInt,
    black_market_check_frames: UnsignedInt,
    stealth_level_mask: u32,
    innate_stealth: Bool,
    team_disguised: Bool,
    use_rider_stealth: Bool,
    order_idle_enemies_to_attack: Bool,
    granted_by_special_power: Bool,
    disguise_fx: Option<String>,
    disguise_reveal_fx: Option<String>,
    enemy_detection_eva_event: Option<String>,
    own_detection_eva_event: Option<String>,
    raw_stealth_forbidden_conditions: Option<String>,
}

impl Default for StealthUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            hint_detectable_states: ObjectStatusMaskType::none(),
            required_status: ObjectStatusMaskType::none(),
            forbidden_status: ObjectStatusMaskType::none(),
            stealth_speed: 0.0,
            friendly_opacity_min: 0.5,
            friendly_opacity_max: 1.0,
            reveal_distance_from_target: 0.0,
            disguise_transition_frames: 0,
            disguise_reveal_transition_frames: 0,
            pulse_frames: 30,
            stealth_delay_frames: u32::MAX,
            black_market_check_frames: 0,
            stealth_level_mask: 0,
            innate_stealth: true,
            team_disguised: false,
            use_rider_stealth: false,
            order_idle_enemies_to_attack: false,
            granted_by_special_power: false,
            disguise_fx: None,
            disguise_reveal_fx: None,
            enemy_detection_eva_event: None,
            own_detection_eva_event: None,
            raw_stealth_forbidden_conditions: None,
        }
    }
}

impl StealthUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STEALTH_UPDATE_FIELDS)
    }

    pub fn hint_detectable_states(&self) -> ObjectStatusMaskType {
        self.hint_detectable_states
    }

    pub fn required_status(&self) -> ObjectStatusMaskType {
        self.required_status
    }

    pub fn forbidden_status(&self) -> ObjectStatusMaskType {
        self.forbidden_status
    }

    pub fn stealth_delay_frames(&self) -> UnsignedInt {
        self.stealth_delay_frames
    }

    pub fn innate_stealth(&self) -> Bool {
        self.innate_stealth
    }

    pub fn set_hint_detectable_states_from_tokens(
        &mut self,
        tokens: &[&str],
    ) -> Result<(), String> {
        self.hint_detectable_states = parse_status_tokens(tokens)?;
        Ok(())
    }

    pub fn set_required_status_from_tokens(&mut self, tokens: &[&str]) -> Result<(), String> {
        self.required_status = parse_status_tokens(tokens)?;
        Ok(())
    }

    pub fn set_forbidden_status_from_tokens(&mut self, tokens: &[&str]) -> Result<(), String> {
        self.forbidden_status = parse_status_tokens(tokens)?;
        Ok(())
    }

    pub fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl ModuleData for StealthUpdateModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for StealthUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Concrete stealth controller implementing the runtime interface.
pub struct StealthController {
    data: Arc<StealthUpdateModuleData>,
    object_id: ObjectID,
    is_stealthed: bool,
    stealth_allowed_frame: UnsignedInt,
    detection_expires_frame: UnsignedInt,
    next_black_market_check_frame: UnsignedInt,
    frames_granted: UnsignedInt,
    enabled: Bool,
    pulse_phase_rate: Real,
    pulse_phase: Real,
    disguise_as_player_index: Int,
    disguise_as_template_name: Option<String>,
    disguise_transition_frames: UnsignedInt,
    disguise_halfpoint_reached: Bool,
    transitioning_to_disguise: Bool,
    disguised: Bool,
    xfer_restore_disguise: Bool,
}

impl std::fmt::Debug for StealthController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StealthController")
            .field("object_id", &self.object_id)
            .field("is_stealthed", &self.is_stealthed)
            .field("stealth_allowed_frame", &self.stealth_allowed_frame)
            .field("detection_expires_frame", &self.detection_expires_frame)
            .field("frames_granted", &self.frames_granted)
            .field("enabled", &self.enabled)
            .field("pulse_phase_rate", &self.pulse_phase_rate)
            .field("pulse_phase", &self.pulse_phase)
            .field("disguise_as_player_index", &self.disguise_as_player_index)
            .field("disguise_as_template_name", &self.disguise_as_template_name)
            .field(
                "disguise_transition_frames",
                &self.disguise_transition_frames,
            )
            .field(
                "disguise_halfpoint_reached",
                &self.disguise_halfpoint_reached,
            )
            .field("transitioning_to_disguise", &self.transitioning_to_disguise)
            .field("disguised", &self.disguised)
            .field("xfer_restore_disguise", &self.xfer_restore_disguise)
            .finish()
    }
}

impl StealthController {
    fn new(data: Arc<StealthUpdateModuleData>, object_id: ObjectID) -> Self {
        let now = TheGameLogic::get_frame();
        let stealth_delay = data.stealth_delay_frames;
        let starts_enabled = !data.team_disguised;
        Self {
            data,
            object_id,
            is_stealthed: false,
            stealth_allowed_frame: now.saturating_add(stealth_delay),
            detection_expires_frame: 0,
            next_black_market_check_frame: 0,
            frames_granted: 0,
            enabled: starts_enabled,
            pulse_phase_rate: 0.2,
            pulse_phase: game_client_random_value_real(0.0, PI),
            disguise_as_player_index: -1,
            disguise_as_template_name: None,
            disguise_transition_frames: 0,
            disguise_halfpoint_reached: false,
            transitioning_to_disguise: false,
            disguised: false,
            xfer_restore_disguise: false,
        }
    }

    fn try_with_object<F, R>(&self, mut f: F) -> Result<R, StealthUpdateError>
    where
        F: FnMut(&mut Object) -> Result<R, StealthUpdateError>,
    {
        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return Err(StealthUpdateError::new(format!(
                "object {} unavailable for StealthUpdate",
                self.object_id
            )));
        };

        let mut guard = object
            .write()
            .map_err(|_| StealthUpdateError::new("failed to lock object for stealth update"))?;
        f(&mut guard)
    }

    fn current_status(&self) -> Result<ObjectStatusMaskType, StealthUpdateError> {
        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return Err(StealthUpdateError::new(format!(
                "object {} unavailable for status query",
                self.object_id
            )));
        };

        let guard = object
            .read()
            .map_err(|_| StealthUpdateError::new("failed to read object status"))?;
        Ok(guard.get_status_bits())
    }

    fn enforce_required_status(&self) -> Result<bool, StealthUpdateError> {
        let status = self.current_status()?;
        let has_required = status.contains(self.data.required_status());
        let forbidden_hit = status.intersects(self.data.forbidden_status());
        Ok(has_required && !forbidden_hit)
    }

    fn set_status_flag(
        &self,
        mask: ObjectStatusMaskType,
        set: bool,
    ) -> Result<(), StealthUpdateError> {
        self.try_with_object(|object| {
            object.set_status(mask, set);
            Ok(())
        })
    }

    pub fn update_stealth(&mut self, _frame_time: f32) -> Result<(), StealthUpdateError> {
        if !self.enabled {
            return Ok(());
        }

        let now = TheGameLogic::get_frame();
        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return Ok(());
        };

        if self.frames_granted > 0 {
            self.frames_granted = self.frames_granted.saturating_sub(1);
            if let Ok(obj_guard) = object.read() {
                if let Some(ai) = obj_guard.get_ai() {
                    if ai
                        .try_lock()
                        .map(|guard| {
                            guard.get_last_command_source() == CommandSourceType::FromPlayer
                        })
                        .unwrap_or(false)
                    {
                        let _ = self.receive_grant(false, 0, now);
                        return Ok(());
                    }
                }
            }
            if self.frames_granted == 0 {
                let _ = self.receive_grant(false, 0, now);
                return Ok(());
            }
        }

        let (allowed, reveal_too_close) = if let Ok(obj_guard) = object.read() {
            (
                self.allowed_to_stealth_runtime(&obj_guard, now),
                self.is_too_close_to_current_target(&obj_guard),
            )
        } else {
            (false, false)
        };

        if reveal_too_close {
            self.mark_as_detected();
        }

        if allowed {
            if now >= self.stealth_allowed_frame {
                self.set_status_flag(ObjectStatusMaskType::STEALTHED, true)?;
                self.is_stealthed = true;
            }
        } else {
            self.stealth_allowed_frame = now.saturating_add(self.data.stealth_delay_frames);
            self.set_status_flag(ObjectStatusMaskType::STEALTHED, false)?;
            self.is_stealthed = false;
        }

        let detected = self.detection_expires_frame > now;
        self.set_status_flag(ObjectStatusMaskType::DETECTED, detected)?;

        Ok(())
    }

    pub fn is_stealthed(&self) -> bool {
        self.is_stealthed
    }

    pub fn is_temporary_grant(&self) -> bool {
        self.frames_granted > 0
    }

    pub fn disguise_as_template(
        &mut self,
        template_name: Option<String>,
        _current_frame: UnsignedInt,
    ) {
        if let Some(template_name) = template_name {
            self.disguise_as_template_name = Some(template_name);
            self.disguise_as_player_index = OBJECT_REGISTRY
                .get_object(self.object_id)
                .and_then(|object| {
                    object
                        .read()
                        .ok()
                        .and_then(|guard| guard.get_controlling_player_id())
                })
                .map(|player_id| player_id as Int)
                .unwrap_or(0);
            self.enabled = true;
            self.transitioning_to_disguise = true;
            self.disguise_transition_frames = self.data.disguise_transition_frames;
            self.disguise_halfpoint_reached = false;
            TheGameLogic::set_wake_frame(self.object_id, UPDATE_SLEEP_NONE);
        } else if self.disguised {
            self.disguise_as_template_name = None;
            self.disguise_as_player_index = 0;
            self.disguise_transition_frames = self.data.disguise_reveal_transition_frames;
            self.transitioning_to_disguise = false;
            self.disguise_halfpoint_reached = false;
        }
    }

    /// Get the stealth level mask (StealthForbiddenConditions bitmask)
    pub fn get_stealth_level(&self) -> UnsignedInt {
        self.data.stealth_level_mask
    }

    pub fn begin_stealth(&mut self) -> Result<(), StealthUpdateError> {
        if !self.enforce_required_status()? {
            return Err(StealthUpdateError::new(
                "stealth prerequisites not satisfied",
            ));
        }

        self.enabled = true;
        self.stealth_allowed_frame = TheGameLogic::get_frame();
        self.set_status_flag(ObjectStatusMaskType::CAN_STEALTH, true)?;
        self.set_status_flag(ObjectStatusMaskType::STEALTHED, true)?;
        self.is_stealthed = true;
        trace!(
            "Object {} entered stealth (delay {} frames)",
            self.object_id,
            self.data.stealth_delay_frames()
        );
        Ok(())
    }

    pub fn end_stealth(&mut self) -> Result<(), StealthUpdateError> {
        self.set_status_flag(ObjectStatusMaskType::STEALTHED, false)?;
        self.set_status_flag(ObjectStatusMaskType::DETECTED, false)?;
        self.is_stealthed = false;
        Ok(())
    }

    pub fn mark_as_detected(&mut self) {
        let now = TheGameLogic::get_frame();
        self.detection_expires_frame = now.saturating_add(self.data.stealth_delay_frames);
        self.set_detected_status();
    }

    pub fn mark_as_detected_for(&mut self, num_frames: UnsignedInt) {
        if num_frames == 0 {
            self.mark_as_detected();
            return;
        }

        let now = TheGameLogic::get_frame();
        let detection_expires_frame = now.saturating_add(num_frames);
        if self.detection_expires_frame < detection_expires_frame {
            self.detection_expires_frame = detection_expires_frame;
        }

        self.set_detected_status();
    }

    fn set_detected_status(&mut self) {
        if let Err(err) = self.set_status_flag(ObjectStatusMaskType::DETECTED, true) {
            warn!(
                "Failed to mark object {} as detected: {}",
                self.object_id, err
            );
        }
        self.is_stealthed = true;
    }

    /// Apply a temporary stealth grant (simplified parity with C++).
    pub fn receive_grant(
        &mut self,
        grant: Bool,
        frames: UnsignedInt,
        current_frame: UnsignedInt,
    ) -> Result<(), StealthUpdateError> {
        if self.data.team_disguised {
            return Ok(());
        }

        if grant {
            self.enabled = true;
            self.is_stealthed = true;
            self.frames_granted = frames;
            self.stealth_allowed_frame = current_frame;
            self.set_status_flag(ObjectStatusMaskType::CAN_STEALTH, true)?;
            self.set_status_flag(ObjectStatusMaskType::STEALTHED, true)?;
        } else {
            self.enabled = false;
            self.is_stealthed = false;
            self.frames_granted = 0;
            self.stealth_allowed_frame = NEVER_FRAME;
            self.set_status_flag(ObjectStatusMaskType::CAN_STEALTH, false)?;
            self.set_status_flag(ObjectStatusMaskType::STEALTHED, false)?;
            self.set_status_flag(ObjectStatusMaskType::DETECTED, false)?;
            if let Some(obj) = OBJECT_REGISTRY.get_object(self.object_id) {
                if let Ok(obj_guard) = obj.read() {
                    if let Some(drawable) = obj_guard.get_drawable() {
                        if let Ok(mut draw_guard) = drawable.write() {
                            draw_guard.set_effective_opacity(1.0, None);
                        }
                    }
                }
            }
        }

        if let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) {
            if let Ok(obj_guard) = object.read() {
                if let Some(contain) = obj_guard.get_contain() {
                    if let Ok(contain_guard) = contain.lock() {
                        if let Some(rider_id) = contain_guard.friend_get_rider() {
                            if let Some(rider) = OBJECT_REGISTRY.get_object(rider_id) {
                                if let Ok(rider_guard) = rider.write() {
                                    if let Some(stealth) = rider_guard.get_stealth() {
                                        if let Ok(mut stealth_guard) = stealth.lock() {
                                            let _ = stealth_guard.receive_grant(
                                                grant,
                                                frames,
                                                current_frame,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn allowed_to_stealth(&self, object: &Object) -> bool {
        let status = object.get_status_bits();
        status.contains(self.data.required_status())
            && !status.intersects(self.data.forbidden_status())
    }

    fn allowed_to_stealth_runtime(&mut self, object: &Object, current_frame: UnsignedInt) -> bool {
        let flags = self.data.stealth_level_mask;
        let status = object.get_status_bits();

        if (flags & STEALTH_NOT_WHILE_ATTACKING) != 0
            && status.contains(ObjectStatusMaskType::IS_FIRING_WEAPON)
        {
            return false;
        }

        if (flags & STEALTH_NOT_WHILE_USING_ABILITY) != 0
            && status.contains(ObjectStatusMaskType::IS_USING_ABILITY)
        {
            return false;
        }

        if (flags & STEALTH_ONLY_WITH_BLACK_MARKET) != 0
            && self.next_black_market_check_frame < current_frame
        {
            self.next_black_market_check_frame = current_frame
                .saturating_add(self.data.black_market_check_frames)
                .saturating_add(game_logic_random_value(0, 10));

            if !self.check_black_market_available(object) {
                return false;
            }
        }

        if !status.contains(ObjectStatusMaskType::CAN_STEALTH) {
            return false;
        }

        if (flags & STEALTH_NOT_WHILE_TAKING_DAMAGE) != 0 {
            if let Some(body) = object.get_body_module() {
                if let Ok(body_guard) = body.lock() {
                    let last_damage = body_guard.get_last_damage_timestamp();
                    if last_damage != NEVER_FRAME && last_damage >= current_frame.saturating_sub(1)
                    {
                        if let Some(info) = body_guard.get_last_damage_info() {
                            if info.input.damage_type != crate::damage::DamageType::Healing {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                }
            }
        }

        if self.data.required_status.any() && !status.contains(self.data.required_status()) {
            return false;
        }

        if status.intersects(self.data.forbidden_status()) {
            return false;
        }

        if (flags & STEALTH_NOT_WHILE_FIRING_WEAPON) != 0
            && status.contains(ObjectStatusMaskType::IS_FIRING_WEAPON)
        {
            if (flags & STEALTH_NOT_WHILE_FIRING_WEAPON) == STEALTH_NOT_WHILE_FIRING_WEAPON {
                return false;
            }
            let last_frame = current_frame.saturating_sub(1);
            if (flags & STEALTH_NOT_WHILE_FIRING_PRIMARY) != 0 {
                if object
                    .get_weapon_in_weapon_slot(crate::weapon::WeaponSlotType::Primary)
                    .map(|weapon| weapon.get_last_shot_frame() >= last_frame)
                    .unwrap_or(false)
                {
                    return false;
                }
            }
            if (flags & STEALTH_NOT_WHILE_FIRING_SECONDARY) != 0 {
                if object
                    .get_weapon_in_weapon_slot(crate::weapon::WeaponSlotType::Secondary)
                    .map(|weapon| weapon.get_last_shot_frame() >= last_frame)
                    .unwrap_or(false)
                {
                    return false;
                }
            }
            if (flags & STEALTH_NOT_WHILE_FIRING_TERTIARY) != 0 {
                if object
                    .get_weapon_in_weapon_slot(crate::weapon::WeaponSlotType::Tertiary)
                    .map(|weapon| weapon.get_last_shot_frame() >= last_frame)
                    .unwrap_or(false)
                {
                    return false;
                }
            }
        }

        if let Some(container) = object.get_container() {
            if let Ok(container_guard) = container.read() {
                if let Some(contain) = container_guard.get_contain() {
                    if let Ok(contain_guard) = contain.lock() {
                        if !contain_guard.is_garrisonable() {
                            return false;
                        }
                    }
                }
            }
        }

        if (flags & STEALTH_NOT_WHILE_RIDERS_ATTACKING) != 0 {
            if let Some(contain) = object.get_contain() {
                if let Ok(contain_guard) = contain.lock() {
                    if contain_guard.is_passenger_allowed_to_fire(None) {
                        for rider_id in contain_guard.get_contained_objects() {
                            let Some(rider_obj) = OBJECT_REGISTRY.get_object(*rider_id) else {
                                continue;
                            };
                            let Ok(rider_guard) = rider_obj.read() else {
                                continue;
                            };
                            let rider_status = rider_guard.get_status_bits();
                            if rider_status.contains(ObjectStatusMaskType::IS_ATTACKING)
                                || rider_status.contains(ObjectStatusMaskType::IS_FIRING_WEAPON)
                            {
                                return false;
                            }
                        }
                    }
                }
            }
        }

        if (flags & STEALTH_NOT_WHILE_MOVING) != 0 {
            if let Some(physics) = object.get_physics() {
                if let Ok(physics_guard) = physics.lock() {
                    if physics_guard.get_velocity().length() > self.data.stealth_speed {
                        return false;
                    }
                }
            }
        }

        if object.test_script_status_bit(ObjectScriptStatusBit::ScriptUnstealthed) {
            return false;
        }

        true
    }

    fn is_too_close_to_current_target(&self, object: &Object) -> bool {
        if self.data.reveal_distance_from_target <= 0.0 {
            return false;
        }
        let Some(victim) = object.get_current_victim() else {
            return false;
        };
        let Ok(victim_guard) = victim.read() else {
            return false;
        };
        let reveal_dist = self.data.reveal_distance_from_target;
        let dist_sq =
            ThePartitionManager::get_distance_squared(object, &victim_guard, FROM_CENTER_2D);
        dist_sq <= reveal_dist * reveal_dist
    }

    fn check_black_market_available(&self, owner: &Object) -> bool {
        let Some(player) = owner.get_controlling_player() else {
            return false;
        };

        let mut has_black_market = false;
        if let Ok(player_guard) = player.read() {
            let _ = player_guard.iterate_objects(|object_arc| {
                if has_black_market {
                    return Ok(());
                }

                let Ok(object_guard) = object_arc.read() else {
                    return Ok(());
                };

                if object_guard.is_effectively_dead() {
                    return Ok(());
                }
                let status = object_guard.get_status_bits();
                if status.contains(ObjectStatusMaskType::UNDER_CONSTRUCTION)
                    || status.contains(ObjectStatusMaskType::SOLD)
                {
                    return Ok(());
                }

                let template_name = object_guard.get_template_name().to_ascii_lowercase();
                let matches_template = template_name.contains("blackmarket")
                    || template_name.contains("black_market")
                    || template_name.contains("black-market");
                let matches_kind = object_guard.is_kind_of(KindOf::CashGenerator)
                    && template_name.contains("market");

                if matches_template || matches_kind {
                    has_black_market = true;
                }

                Ok(())
            });
        }

        has_black_market
    }
}

impl StealthUpdateTrait for StealthController {
    fn update_stealth(
        &mut self,
        frame_time: f32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Self::update_stealth(self, frame_time).map_err(|err| Box::new(err) as _)
    }

    fn is_stealthed(&self) -> bool {
        Self::is_stealthed(self)
    }

    fn begin_stealth(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Self::begin_stealth(self).map_err(|err| Box::new(err) as _)
    }

    fn end_stealth(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Self::end_stealth(self).map_err(|err| Box::new(err) as _)
    }

    fn allowed_to_stealth(&self, object: &Object) -> bool {
        Self::allowed_to_stealth(self, object)
    }

    fn mark_as_detected(&mut self) {
        Self::mark_as_detected(self);
    }
}

/// Module wrapper that binds the controller to the object module system.
pub struct StealthUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<StealthUpdateModuleData>,
    controller: StealthUpdateHandle,
    object_id: ObjectID,
    next_call_frame_and_phase: UnsignedInt,
}

impl StealthUpdateModule {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<StealthUpdateModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let controller = Arc::new(Mutex::new(StealthController::new(data.clone(), object_id)));
        Self {
            module_name_key,
            data,
            controller,
            object_id,
            next_call_frame_and_phase: 0,
        }
    }

    fn register_with_object(&self) {
        if let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) {
            if let Ok(mut guard) = object.write() {
                guard.set_stealth_module(self.controller.clone());
            }
        }
    }
}

impl Module for StealthUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        self.register_with_object();
        if self.data.innate_stealth() {
            if let Ok(guard) = self.controller.lock() {
                let _ = guard.set_status_flag(ObjectStatusMaskType::CAN_STEALTH, true);
            }
        }
    }

    fn get_stealth_disguise_control_interface(
        &mut self,
    ) -> Option<&mut dyn StealthDisguiseControlInterface> {
        Some(self)
    }
}

impl StealthDisguiseControlInterface for StealthUpdateModule {
    fn disguise_as_template(&mut self, template_name: Option<String>, current_frame: u32) {
        if let Ok(mut controller) = self.controller.lock() {
            controller.disguise_as_template(template_name, current_frame);
        }
    }
}

impl Snapshotable for StealthUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| e.to_string())?;
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)?;

        let controller = self
            .controller
            .lock()
            .map_err(|_| "StealthUpdateModule: controller lock poisoned".to_string())?;

        let mut stealth_allowed_frame = controller.stealth_allowed_frame;
        xfer.xfer_unsigned_int(&mut stealth_allowed_frame)
            .map_err(|e| e.to_string())?;
        let mut detection_expires_frame = controller.detection_expires_frame;
        xfer.xfer_unsigned_int(&mut detection_expires_frame)
            .map_err(|e| e.to_string())?;
        let mut enabled = controller.enabled;
        xfer.xfer_bool(&mut enabled).map_err(|e| e.to_string())?;
        let mut pulse_phase_rate = controller.pulse_phase_rate;
        xfer.xfer_real(&mut pulse_phase_rate)
            .map_err(|e| e.to_string())?;
        let mut pulse_phase = controller.pulse_phase;
        xfer.xfer_real(&mut pulse_phase)
            .map_err(|e| e.to_string())?;
        let mut disguise_as_player_index = controller.disguise_as_player_index;
        xfer.xfer_int(&mut disguise_as_player_index)
            .map_err(|e| e.to_string())?;
        let mut disguise_as_template_name = controller
            .disguise_as_template_name
            .clone()
            .unwrap_or_default();
        xfer.xfer_ascii_string(&mut disguise_as_template_name)
            .map_err(|e| e.to_string())?;
        let mut disguise_transition_frames = controller.disguise_transition_frames;
        xfer.xfer_unsigned_int(&mut disguise_transition_frames)
            .map_err(|e| e.to_string())?;
        let mut disguise_halfpoint_reached = controller.disguise_halfpoint_reached;
        xfer.xfer_bool(&mut disguise_halfpoint_reached)
            .map_err(|e| e.to_string())?;
        let mut transitioning_to_disguise = controller.transitioning_to_disguise;
        xfer.xfer_bool(&mut transitioning_to_disguise)
            .map_err(|e| e.to_string())?;
        let mut disguised = controller.disguised;
        xfer.xfer_bool(&mut disguised).map_err(|e| e.to_string())?;
        if version >= 2 {
            let mut frames_granted = controller.frames_granted;
            xfer.xfer_unsigned_int(&mut frames_granted)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        let mut controller = self
            .controller
            .lock()
            .map_err(|_| "StealthUpdateModule: controller lock poisoned".to_string())?;

        xfer.xfer_unsigned_int(&mut controller.stealth_allowed_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut controller.detection_expires_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut controller.enabled)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut controller.pulse_phase_rate)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut controller.pulse_phase)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut controller.disguise_as_player_index)
            .map_err(|e| e.to_string())?;
        let mut disguise_as_template_name = controller
            .disguise_as_template_name
            .clone()
            .unwrap_or_default();
        xfer.xfer_ascii_string(&mut disguise_as_template_name)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            controller.disguise_as_template_name = if disguise_as_template_name.is_empty() {
                None
            } else {
                Some(disguise_as_template_name)
            };
        }
        xfer.xfer_unsigned_int(&mut controller.disguise_transition_frames)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut controller.disguise_halfpoint_reached)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut controller.transitioning_to_disguise)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut controller.disguised)
            .map_err(|e| e.to_string())?;
        if version >= 2 {
            xfer.xfer_unsigned_int(&mut controller.frames_granted)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut controller = self
            .controller
            .lock()
            .map_err(|_| "StealthUpdateModule: controller lock poisoned".to_string())?;
        if controller.disguised {
            controller.xfer_restore_disguise = true;
        }
        Ok(())
    }
}

/// Internal stealth error mirroring the legacy failure points.
#[derive(Debug)]
pub struct StealthUpdateError(String);

impl StealthUpdateError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for StealthUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for StealthUpdateError {}

fn parse_status_tokens(tokens: &[&str]) -> Result<ObjectStatusMaskType, String> {
    let normalized: Vec<&str> = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();
    if normalized.is_empty() {
        return Ok(ObjectStatusMaskType::none());
    }
    ObjectStatusMaskType::parse_tokens(normalized)
}

fn parse_to_string(tokens: &[&str]) -> Option<String> {
    let combined = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect::<Vec<&str>>();
    if combined.is_empty() {
        None
    } else {
        Some(combined.join(" "))
    }
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| *token != "=")
}

fn parse_hint_detectable_conditions(
    _ini: &mut INI,
    data: &mut StealthUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.set_hint_detectable_states_from_tokens(tokens)
        .map_err(|_| INIError::InvalidData)
}

fn parse_required_status(
    _ini: &mut INI,
    data: &mut StealthUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.set_required_status_from_tokens(tokens)
        .map_err(|_| INIError::InvalidData)
}

fn parse_forbidden_status(
    _ini: &mut INI,
    data: &mut StealthUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.set_forbidden_status_from_tokens(tokens)
        .map_err(|_| INIError::InvalidData)
}

fn parse_duration_field(field: &mut UnsignedInt, tokens: &[&str]) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    *field = INI::parse_duration_unsigned_int(value)?;
    Ok(())
}

fn parse_real_field(field: &mut Real, tokens: &[&str]) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    *field = INI::parse_real(value)?;
    Ok(())
}

fn parse_percent_field(field: &mut Real, tokens: &[&str]) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    *field = INI::parse_percent_to_real(value)?;
    Ok(())
}

fn parse_bool_field(field: &mut Bool, tokens: &[&str]) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    *field = INI::parse_bool(value)?;
    Ok(())
}

fn parse_fx_field(field: &mut Option<String>, tokens: &[&str]) -> Result<(), INIError> {
    *field = parse_to_string(tokens);
    Ok(())
}

fn parse_eva_field(field: &mut Option<String>, tokens: &[&str]) -> Result<(), INIError> {
    *field = parse_to_string(tokens);
    Ok(())
}

fn parse_stealth_level_conditions(
    _ini: &mut INI,
    data: &mut StealthUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.raw_stealth_forbidden_conditions = parse_to_string(tokens);
    let mut mask = 0u32;
    for token in tokens.iter().copied().filter(|token| *token != "=") {
        if let Ok(bits) = INI::parse_unsigned_int(token) {
            mask |= bits;
            continue;
        }

        let normalized = token.trim().trim_start_matches('+').to_ascii_uppercase();
        match normalized.as_str() {
            "ATTACKING" => mask |= STEALTH_NOT_WHILE_ATTACKING,
            "MOVING" => mask |= STEALTH_NOT_WHILE_MOVING,
            "USING_ABILITY" => mask |= STEALTH_NOT_WHILE_USING_ABILITY,
            "FIRING_PRIMARY" => mask |= STEALTH_NOT_WHILE_FIRING_PRIMARY,
            "FIRING_SECONDARY" => mask |= STEALTH_NOT_WHILE_FIRING_SECONDARY,
            "FIRING_TERTIARY" => mask |= STEALTH_NOT_WHILE_FIRING_TERTIARY,
            "NO_BLACK_MARKET" => mask |= STEALTH_ONLY_WITH_BLACK_MARKET,
            "TAKING_DAMAGE" => mask |= STEALTH_NOT_WHILE_TAKING_DAMAGE,
            "RIDERS_ATTACKING" => mask |= STEALTH_NOT_WHILE_RIDERS_ATTACKING,
            _ => {}
        }
    }
    data.stealth_level_mask = mask;
    Ok(())
}

fn parse_move_threshold_speed(
    _ini: &mut INI,
    data: &mut StealthUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_real_field(&mut data.stealth_speed, tokens)
}

fn parse_reveal_distance(
    _ini: &mut INI,
    data: &mut StealthUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_real_field(&mut data.reveal_distance_from_target, tokens)
}

fn parse_disguise_transition_time(
    _ini: &mut INI,
    data: &mut StealthUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_duration_field(&mut data.disguise_transition_frames, tokens)
}

fn parse_disguise_reveal_transition_time(
    _ini: &mut INI,
    data: &mut StealthUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_duration_field(&mut data.disguise_reveal_transition_frames, tokens)
}

fn parse_stealth_delay(
    _ini: &mut INI,
    data: &mut StealthUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_duration_field(&mut data.stealth_delay_frames, tokens)
}

fn parse_pulse_frequency(
    _ini: &mut INI,
    data: &mut StealthUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_duration_field(&mut data.pulse_frames, tokens)
}

fn parse_black_market_delay(
    _ini: &mut INI,
    data: &mut StealthUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_duration_field(&mut data.black_market_check_frames, tokens)
}

const STEALTH_UPDATE_FIELDS: &[FieldParse<StealthUpdateModuleData>] = &[
    FieldParse {
        token: "StealthDelay",
        parse: parse_stealth_delay,
    },
    FieldParse {
        token: "MoveThresholdSpeed",
        parse: parse_move_threshold_speed,
    },
    FieldParse {
        token: "StealthForbiddenConditions",
        parse: parse_stealth_level_conditions,
    },
    FieldParse {
        token: "HintDetectableConditions",
        parse: parse_hint_detectable_conditions,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_required_status,
    },
    FieldParse {
        token: "ForbiddenStatus",
        parse: parse_forbidden_status,
    },
    FieldParse {
        token: "FriendlyOpacityMin",
        parse: |_, data, tokens| parse_percent_field(&mut data.friendly_opacity_min, tokens),
    },
    FieldParse {
        token: "FriendlyOpacityMax",
        parse: |_, data, tokens| parse_percent_field(&mut data.friendly_opacity_max, tokens),
    },
    FieldParse {
        token: "PulseFrequency",
        parse: parse_pulse_frequency,
    },
    FieldParse {
        token: "DisguisesAsTeam",
        parse: |_, data, tokens| parse_bool_field(&mut data.team_disguised, tokens),
    },
    FieldParse {
        token: "RevealDistanceFromTarget",
        parse: parse_reveal_distance,
    },
    FieldParse {
        token: "OrderIdleEnemiesToAttackMeUponReveal",
        parse: |_, data, tokens| parse_bool_field(&mut data.order_idle_enemies_to_attack, tokens),
    },
    FieldParse {
        token: "DisguiseFX",
        parse: |_, data, tokens| parse_fx_field(&mut data.disguise_fx, tokens),
    },
    FieldParse {
        token: "DisguiseRevealFX",
        parse: |_, data, tokens| parse_fx_field(&mut data.disguise_reveal_fx, tokens),
    },
    FieldParse {
        token: "DisguiseTransitionTime",
        parse: parse_disguise_transition_time,
    },
    FieldParse {
        token: "DisguiseRevealTransitionTime",
        parse: parse_disguise_reveal_transition_time,
    },
    FieldParse {
        token: "InnateStealth",
        parse: |_, data, tokens| parse_bool_field(&mut data.innate_stealth, tokens),
    },
    FieldParse {
        token: "UseRiderStealth",
        parse: |_, data, tokens| parse_bool_field(&mut data.use_rider_stealth, tokens),
    },
    FieldParse {
        token: "EnemyDetectionEvaEvent",
        parse: |_, data, tokens| parse_eva_field(&mut data.enemy_detection_eva_event, tokens),
    },
    FieldParse {
        token: "OwnDetectionEvaEvent",
        parse: |_, data, tokens| parse_eva_field(&mut data.own_detection_eva_event, tokens),
    },
    FieldParse {
        token: "BlackMarketCheckDelay",
        parse: parse_black_market_delay,
    },
    FieldParse {
        token: "GrantedBySpecialPower",
        parse: |_, data, tokens| parse_bool_field(&mut data.granted_by_special_power, tokens),
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::snapshot::Snapshot;
    use game_engine::common::system::xfer::{XferBlockSize, XferStatus};
    use std::sync::RwLock;

    fn parse_field(data: &mut StealthUpdateModuleData, token: &str, values: &[&str]) {
        let field = STEALTH_UPDATE_FIELDS
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, data, values).expect("field parses");
    }

    struct MemoryXfer {
        mode: XferMode,
        buffer: Vec<u8>,
        position: usize,
        identifier: String,
    }

    impl MemoryXfer {
        fn save() -> Self {
            Self {
                mode: XferMode::Save,
                buffer: Vec::new(),
                position: 0,
                identifier: String::new(),
            }
        }

        fn load(buffer: Vec<u8>) -> Self {
            Self {
                mode: XferMode::Load,
                buffer,
                position: 0,
                identifier: String::new(),
            }
        }
    }

    impl Xfer for MemoryXfer {
        fn get_xfer_mode(&self) -> XferMode {
            self.mode
        }

        fn get_identifier(&self) -> &str {
            &self.identifier
        }

        fn set_options(&mut self, _options: u32) {}

        fn clear_options(&mut self, _options: u32) {}

        fn get_options(&self) -> u32 {
            0
        }

        fn open(&mut self, identifier: &str) -> Result<(), XferStatus> {
            self.identifier = identifier.to_string();
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

        fn skip(&mut self, data_size: i32) -> Result<(), XferStatus> {
            self.position = self.position.saturating_add(data_size.max(0) as usize);
            Ok(())
        }

        fn xfer_snapshot(&mut self, _snapshot: &mut Snapshot) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> std::io::Result<()> {
            match self.mode {
                XferMode::Save | XferMode::Crc => {
                    let bytes = ascii_string_data.as_bytes();
                    if bytes.len() > u8::MAX as usize {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "ASCII string too long",
                        ));
                    }
                    let mut len = bytes.len() as u8;
                    self.xfer_unsigned_byte(&mut len)?;
                    self.buffer.extend_from_slice(bytes);
                    Ok(())
                }
                XferMode::Load => {
                    let mut len = 0u8;
                    self.xfer_unsigned_byte(&mut len)?;
                    let end = self.position.saturating_add(len as usize);
                    if end > self.buffer.len() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "ASCII string beyond buffer",
                        ));
                    }
                    let bytes = &self.buffer[self.position..end];
                    *ascii_string_data = String::from_utf8(bytes.to_vec()).map_err(|_| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid UTF-8")
                    })?;
                    self.position = end;
                    Ok(())
                }
                _ => Ok(()),
            }
        }

        fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> std::io::Result<()> {
            self.xfer_ascii_string(unicode_string_data)
        }

        unsafe fn xfer_implementation(
            &mut self,
            data: *mut u8,
            data_size: usize,
        ) -> std::io::Result<()> {
            match self.mode {
                XferMode::Save | XferMode::Crc => {
                    let bytes = std::slice::from_raw_parts(data, data_size);
                    self.buffer.extend_from_slice(bytes);
                    Ok(())
                }
                XferMode::Load => {
                    let end = self.position.saturating_add(data_size);
                    if end > self.buffer.len() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "xfer beyond buffer",
                        ));
                    }
                    std::ptr::copy_nonoverlapping(
                        self.buffer[self.position..end].as_ptr(),
                        data,
                        data_size,
                    );
                    self.position = end;
                    Ok(())
                }
                _ => Ok(()),
            }
        }
    }

    #[test]
    fn duration_fields_parse_ini_milliseconds_to_logic_frames() {
        let mut data = StealthUpdateModuleData::default();

        parse_field(&mut data, "StealthDelay", &["=", "1000"]);
        parse_field(&mut data, "PulseFrequency", &["=", "1000"]);
        parse_field(&mut data, "DisguiseTransitionTime", &["=", "1000"]);
        parse_field(&mut data, "DisguiseRevealTransitionTime", &["=", "1000"]);
        parse_field(&mut data, "BlackMarketCheckDelay", &["=", "500"]);

        assert_eq!(data.stealth_delay_frames, 30);
        assert_eq!(data.pulse_frames, 30);
        assert_eq!(data.disguise_transition_frames, 30);
        assert_eq!(data.disguise_reveal_transition_frames, 30);
        assert_eq!(data.black_market_check_frames, 15);
    }

    #[test]
    fn parses_status_tokens() {
        let mut data = StealthUpdateModuleData::default();
        data.set_hint_detectable_states_from_tokens(&["STEALTHED", "DETECTED"])
            .expect("hint detectable");
        data.set_required_status_from_tokens(&["+CAN_STEALTH"])
            .expect("required");
        data.set_forbidden_status_from_tokens(&["MASKED"])
            .expect("forbidden");

        assert!(data
            .hint_detectable_states()
            .contains(ObjectStatusMaskType::STEALTHED));
        assert!(data
            .hint_detectable_states()
            .contains(ObjectStatusMaskType::DETECTED));
        assert!(data
            .required_status()
            .contains(ObjectStatusMaskType::CAN_STEALTH));
        assert!(data
            .forbidden_status()
            .contains(ObjectStatusMaskType::MASKED));
    }

    #[test]
    fn begin_and_end_stealth_sets_status_flags() {
        let object_id: ObjectID = 4242;
        let object = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        OBJECT_REGISTRY.register_object(object_id, &object);

        {
            let mut guard = object.write().unwrap();
            guard.set_status(ObjectStatusMaskType::CAN_STEALTH, true);
        }

        let mut data = StealthUpdateModuleData::default();
        data.set_required_status_from_tokens(&["CAN_STEALTH"])
            .unwrap();
        let data = Arc::new(data);
        let controller = Arc::new(Mutex::new(StealthController::new(data.clone(), object_id)));

        {
            let mut guard = controller.lock().unwrap();
            guard.begin_stealth().expect("begin stealth");
        }

        {
            let guard = object.read().unwrap();
            assert!(guard
                .get_status_bits()
                .contains(ObjectStatusMaskType::STEALTHED));
        }

        {
            let mut guard = controller.lock().unwrap();
            guard.mark_as_detected();
            guard.end_stealth().expect("end stealth");
        }

        {
            let guard = object.read().unwrap();
            assert!(!guard
                .get_status_bits()
                .contains(ObjectStatusMaskType::STEALTHED));
        }

        OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn mark_as_detected_for_extends_detection_without_shortening() {
        let object_id: ObjectID = 4243;
        let object = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        OBJECT_REGISTRY.register_object(object_id, &object);

        let mut data = StealthUpdateModuleData::default();
        data.stealth_delay_frames = 3;
        let data = Arc::new(data);
        let mut controller = StealthController::new(data, object_id);
        let now = TheGameLogic::get_frame();

        controller.mark_as_detected_for(11);
        assert_eq!(controller.detection_expires_frame, now.saturating_add(11));

        controller.mark_as_detected_for(5);
        assert_eq!(controller.detection_expires_frame, now.saturating_add(11));

        controller.mark_as_detected_for(0);
        assert_eq!(controller.detection_expires_frame, now.saturating_add(3));

        OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn stealth_update_module_xfer_round_trips_disguise_state() {
        let data = Arc::new(StealthUpdateModuleData::default());
        let mut saved = StealthUpdateModule::new(7, data.clone(), 4244);
        saved.next_call_frame_and_phase = 0x5511;

        {
            let mut controller = saved.controller.lock().unwrap();
            controller.stealth_allowed_frame = 101;
            controller.detection_expires_frame = 202;
            controller.next_black_market_check_frame = 303;
            controller.enabled = false;
            controller.pulse_phase_rate = 0.37;
            controller.pulse_phase = 1.25;
            controller.disguise_as_player_index = 3;
            controller.disguise_as_template_name = Some("ChinaVehicleTroopCrawler".to_string());
            controller.disguise_transition_frames = 17;
            controller.disguise_halfpoint_reached = true;
            controller.transitioning_to_disguise = true;
            controller.disguised = true;
            controller.frames_granted = 44;
        }

        let mut save_xfer = MemoryXfer::save();
        saved.xfer(&mut save_xfer).expect("save xfer");

        let mut loaded = StealthUpdateModule::new(8, data, 5252);
        let mut load_xfer = MemoryXfer::load(save_xfer.buffer);
        loaded.xfer(&mut load_xfer).expect("load xfer");
        loaded.load_post_process().expect("load post process");

        assert_eq!(loaded.object_id, 5252);
        assert_eq!(loaded.next_call_frame_and_phase, 0x5511);

        let controller = loaded.controller.lock().unwrap();
        assert_eq!(controller.stealth_allowed_frame, 101);
        assert_eq!(controller.detection_expires_frame, 202);
        assert_eq!(controller.next_black_market_check_frame, 0);
        assert!(!controller.enabled);
        assert_eq!(controller.pulse_phase_rate, 0.37);
        assert_eq!(controller.pulse_phase, 1.25);
        assert_eq!(controller.disguise_as_player_index, 3);
        assert_eq!(
            controller.disguise_as_template_name.as_deref(),
            Some("ChinaVehicleTroopCrawler")
        );
        assert_eq!(controller.disguise_transition_frames, 17);
        assert!(controller.disguise_halfpoint_reached);
        assert!(controller.transitioning_to_disguise);
        assert!(controller.disguised);
        assert_eq!(controller.frames_granted, 44);
        assert!(controller.xfer_restore_disguise);
    }

    #[test]
    fn stealth_update_module_exposes_disguise_control_interface() {
        let mut data = StealthUpdateModuleData::default();
        data.disguise_transition_frames = 12;
        data.disguise_reveal_transition_frames = 7;
        let data = Arc::new(data);
        let mut module = StealthUpdateModule::new(9, data, 4245);

        module
            .get_stealth_disguise_control_interface()
            .expect("disguise interface")
            .disguise_as_template(Some("GLAVehicleBombTruck".to_string()), 100);

        {
            let controller = module.controller.lock().unwrap();
            assert_eq!(
                controller.disguise_as_template_name.as_deref(),
                Some("GLAVehicleBombTruck")
            );
            assert_eq!(controller.disguise_as_player_index, 0);
            assert!(controller.enabled);
            assert!(controller.transitioning_to_disguise);
            assert_eq!(controller.disguise_transition_frames, 12);
            assert!(!controller.disguise_halfpoint_reached);
        }

        {
            let mut controller = module.controller.lock().unwrap();
            controller.disguised = true;
        }

        module
            .get_stealth_disguise_control_interface()
            .expect("disguise interface")
            .disguise_as_template(None, 110);

        let controller = module.controller.lock().unwrap();
        assert_eq!(controller.disguise_as_template_name, None);
        assert_eq!(controller.disguise_as_player_index, 0);
        assert!(!controller.transitioning_to_disguise);
        assert_eq!(controller.disguise_transition_frames, 7);
        assert!(!controller.disguise_halfpoint_reached);
    }
}
