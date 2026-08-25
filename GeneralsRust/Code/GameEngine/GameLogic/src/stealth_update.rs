use std::any::Any;
use std::sync::{Arc, Mutex};

use crate::common::{
    Bool, CommandSourceType, DisabledMaskType, FROM_CENTER_2D, Int, KindOf, ObjectID,
    ObjectStatusMaskType, ObjectStatusTypes, Real, Relationship, UnsignedInt,
};
use crate::helpers::{
    TheAudio, TheGameLogic, ThePartitionManager, game_client_random_value_real,
    game_logic_random_value,
};
use crate::modules::{
    StealthUpdate as StealthUpdateTrait, UPDATE_SLEEP_NONE, UpdateModuleInterface,
};
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::{Object, ObjectScriptStatusBit};
use crate::player::{PlayerArcExt, player_list};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::xfer::XferMode;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData, NameKeyType, StealthDisguiseControlInterface, Thing,
};
use log::{trace, warn};
use std::f32::consts::PI;

/// Wave 283: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

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

/// C++ StealthUpdate.cpp:389-392 / leftover `allowed_to_stealth_runtime`.
/// Destalth only when leftover physics velocity exceeds leftover MoveThresholdSpeed.
#[inline]
pub fn leftover_not_while_moving_destalths(
    leftover_velocity_magnitude: Real,
    leftover_move_threshold_speed: Real,
) -> bool {
    leftover_velocity_magnitude > leftover_move_threshold_speed
}

/// Leftover ThingFactory first `StealthUpdate` MoveThresholdSpeed (`stealth_speed`).
pub fn leftover_stealth_move_threshold_speed(template_name: &str) -> Option<Real> {
    if template_name.is_empty() {
        return None;
    }
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    for entry in tmpl.get_behavior_module_info().iter() {
        if !entry.name.as_str().eq_ignore_ascii_case("StealthUpdate") {
            continue;
        }
        if let Some(data) = entry
            .data
            .as_any()
            .downcast_ref::<StealthUpdateModuleData>()
        {
            return Some(data.stealth_speed());
        }
        if let Some(data) = entry
            .data
            .as_any()
            .downcast_ref::<crate::object::behavior::StealthUpdateModuleData>()
        {
            return Some(data.stealth_speed);
        }
        if let Some(data) = entry
            .data
            .as_any()
            .downcast_ref::<crate::object::update::stealth_update::StealthUpdateModuleData>(
        ) {
            return Some(data.stealth_speed());
        }
        if let Some(speed) = entry.data.get_ini_real("MoveThresholdSpeed") {
            return Some(speed);
        }
    }
    None
}

fn play_object_stealth_sound(object_id: ObjectID, stealth_on: bool) {
    let Some(mut event) = OBJECT_REGISTRY.with_object(object_id, |obj| {
        if stealth_on {
            obj.get_template().get_sound_stealth_on()
        } else {
            obj.get_template().get_sound_stealth_off()
        }
    }) else {
        return;
    };
    event.set_object_id(object_id);
    if let Some(audio) = TheAudio::get() {
        audio.add_audio_event(&event);
    }
}

fn play_per_unit_sound(object_id: ObjectID, sound_name: &str) {
    let Some(mut event) = OBJECT_REGISTRY
        .with_object(object_id, |obj| {
            obj.get_template().get_per_unit_sound(sound_name)
        })
        .flatten()
    else {
        return;
    };
    event.set_object_id(object_id);
    if let Some(audio) = TheAudio::get() {
        audio.add_audio_event(&event);
    }
}

fn play_fx_at_object(object_id: ObjectID, fx_name: &str) {
    let Some(pos) = OBJECT_REGISTRY.with_object(object_id, |obj| *obj.get_position()) else {
        return;
    };
    if let Some(fx) = crate::helpers::TheFXListStore::find_fx_list(fx_name) {
        let _ = fx.do_fx_at_position(&pos);
    }
}

fn refresh_radar_for_object(object_id: ObjectID) {
    use game_engine::common::system::radar::{
        Coord3D as RadarCoord3D, RadarObject, RadarPriorityType, get_radar_system,
    };
    let Some(pos) = OBJECT_REGISTRY.with_object(object_id, |obj| *obj.get_position()) else {
        return;
    };
    if let Ok(mut radar) = get_radar_system().write() {
        radar.remove_object(object_id as u32);
        let mut radar_obj = RadarObject::new(object_id as u32);
        radar_obj.world_pos = RadarCoord3D::new(pos.x, pos.y, pos.z);
        radar_obj.priority = RadarPriorityType::Unit;
        radar.add_object(radar_obj);
    }
}

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

    pub fn stealth_speed(&self) -> Real {
        self.stealth_speed
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
        OBJECT_REGISTRY
            .with_object_mut(self.object_id, |guard| f(guard))
            .ok_or_else(|| {
                StealthUpdateError::new(format!(
                    "object {} unavailable for StealthUpdate",
                    self.object_id
                ))
            })?
    }

    fn current_status(&self) -> Result<ObjectStatusMaskType, StealthUpdateError> {
        // Wave 283: empty dual-world → unavailable.
        if dual_world_registry_unavailable() {
            return Err(StealthUpdateError::new("dual-world registry unavailable"));
        }

        OBJECT_REGISTRY
            .with_object(self.object_id, |guard| guard.get_status_bits())
            .ok_or_else(|| {
                StealthUpdateError::new(format!(
                    "object {} unavailable for status query",
                    self.object_id
                ))
            })
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
        // Wave 283: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        // C++ StealthUpdate.cpp:572-589 — restore disguise visual after xfer
        // before the enabled check (cannot swap drawables during load).
        if self.xfer_restore_disguise {
            let was_hidden = OBJECT_REGISTRY
                .with_object(self.object_id, |obj| {
                    obj.get_drawable()
                        .and_then(|d| d.read().ok().map(|g| g.is_drawable_effectively_hidden()))
                })
                .flatten()
                .unwrap_or(false);
            self.change_visual_disguise();
            if was_hidden {
                if let Some(drawable) = OBJECT_REGISTRY
                    .with_object(self.object_id, |obj| obj.get_drawable())
                    .flatten()
                {
                    if let Ok(mut draw) = drawable.write() {
                        let _ = draw.set_drawable_hidden(true);
                    }
                }
            }
        }

        if !self.enabled {
            return Ok(());
        }

        let now = TheGameLogic::get_frame();
        if OBJECT_REGISTRY
            .with_object(self.object_id, |_| ())
            .is_none()
        {
            return Ok(());
        }

        // C++ StealthUpdate.cpp:622-671 — mines stay fully hidden; disguise
        // transition swaps the drawable at halfway; otherwise pulse opacity.
        if self.update_drawable_visuals()? {
            return Ok(());
        }

        if self.frames_granted > 0 {
            self.frames_granted = self.frames_granted.saturating_sub(1);
            let from_player = OBJECT_REGISTRY
                .with_object(self.object_id, |obj_guard| {
                    obj_guard.get_ai().and_then(|ai| {
                        ai.try_lock().ok().map(|guard| {
                            guard.get_last_command_source() == CommandSourceType::FromPlayer
                        })
                    })
                })
                .flatten()
                .unwrap_or(false);
            if from_player {
                let _ = self.receive_grant(false, 0, now);
                return Ok(());
            }
            if self.frames_granted == 0 {
                let _ = self.receive_grant(false, 0, now);
                return Ok(());
            }
        }

        let stealth_owner_id = self.calc_stealth_owner();
        let stealth_delay = self.stealth_rules_for(stealth_owner_id).0;

        let (allowed, reveal_too_close, was_stealthed, was_detected, locally_controlled) =
            OBJECT_REGISTRY
                .with_object(self.object_id, |obj_guard| {
                    let status = obj_guard.get_status_bits();
                    (
                        self.allowed_to_stealth_runtime(obj_guard, now, stealth_owner_id),
                        self.is_too_close_to_current_target(obj_guard),
                        status.contains(ObjectStatusMaskType::STEALTHED),
                        status.contains(ObjectStatusMaskType::DETECTED),
                        obj_guard.is_locally_controlled(),
                    )
                })
                .unwrap_or((false, false, false, false, false));

        if reveal_too_close {
            self.mark_as_detected();
            return Ok(());
        }

        if allowed {
            if now >= self.stealth_allowed_frame {
                // C++ StealthUpdate.cpp:727-731 — SoundStealthOn when gaining STEALTHED.
                if !was_stealthed {
                    play_object_stealth_sound(self.object_id, true);
                }
                self.set_status_flag(ObjectStatusMaskType::STEALTHED, true)?;
                self.is_stealthed = true;
            }
        } else {
            self.stealth_allowed_frame = now.saturating_add(stealth_delay);
            // C++ StealthUpdate.cpp:742-746 — destalth still plays SoundStealthOn.
            if was_stealthed {
                play_object_stealth_sound(self.object_id, true);
            }
            self.set_status_flag(ObjectStatusMaskType::STEALTHED, false)?;
            self.is_stealthed = false;
            self.hint_detectable_while_unstealthed();
        }

        let detected = self.detection_expires_frame > now;
        if detected {
            // C++ StealthUpdate.cpp:758-763 — first DETECTED plays SoundStealthOff.
            if !was_detected {
                play_object_stealth_sound(self.object_id, false);
            }
        } else if was_detected && locally_controlled {
            // C++ StealthUpdate.cpp:771-779 — detection clear plays SoundStealthOn if local.
            play_object_stealth_sound(self.object_id, true);
        }
        self.set_status_flag(ObjectStatusMaskType::DETECTED, detected)?;

        Ok(())
    }

    /// C++ StealthUpdate.cpp:407-421 hintDetectableWhileUnstealthed.
    fn hint_detectable_while_unstealthed(&self) {
        let _ = OBJECT_REGISTRY.with_object(self.object_id, |obj| {
            if !self
                .data
                .hint_detectable_states
                .intersects(obj.get_status_bits())
            {
                return;
            }
            if !obj.is_locally_controlled() {
                return;
            }
            if let Some(drawable) = obj.get_drawable() {
                if let Ok(mut draw) = drawable.write() {
                    draw.set_second_material_pass_opacity(1.0);
                }
            }
        });
    }

    /// Returns true if disguise-reveal finished this frame (caller should sleep).
    fn update_drawable_visuals(&mut self) -> Result<bool, StealthUpdateError> {
        let is_mine = OBJECT_REGISTRY
            .with_object(self.object_id, |obj| obj.is_kind_of(KindOf::Mine))
            .unwrap_or(false);

        if is_mine {
            self.set_drawable_opacity(0.0, Some(0.0));
            return Ok(false);
        }

        if self.disguise_transition_frames > 0 {
            self.disguise_transition_frames -= 1;
            let total_frames = if self.transitioning_to_disguise {
                self.data.disguise_transition_frames
            } else {
                self.data.disguise_reveal_transition_frames
            };
            let factor = if total_frames == 0 {
                1.0
            } else {
                1.0 - (self.disguise_transition_frames as f32 / total_frames as f32)
            };
            if factor >= 0.5 && !self.disguise_halfpoint_reached {
                self.change_visual_disguise();
                self.disguise_halfpoint_reached = true;
            }
            let opacity = (1.0 - (factor * 2.0)).abs();
            let override_opacity = if opacity < 1.0 { 0.0 } else { 1.0 };
            self.set_drawable_opacity(opacity, Some(override_opacity));
            if self.disguise_transition_frames == 0 && !self.transitioning_to_disguise {
                self.enabled = false;
                self.set_status_flag(ObjectStatusMaskType::STEALTHED, false)?;
                self.set_status_flag(ObjectStatusMaskType::DETECTED, false)?;
                self.is_stealthed = false;
                return Ok(true);
            }
            return Ok(false);
        }

        let opacity = 0.5 + (self.pulse_phase.sin() * 0.5);
        self.set_drawable_opacity(opacity, None);
        self.pulse_phase += self.pulse_phase_rate;
        Ok(false)
    }

    fn set_drawable_opacity(&self, pulse: Real, explicit: Option<Real>) {
        if let Some(drawable) = OBJECT_REGISTRY
            .with_object(self.object_id, |obj| obj.get_drawable())
            .flatten()
        {
            if let Ok(mut draw) = drawable.write() {
                draw.set_effective_opacity(pulse, explicit);
            }
        }
    }

    /// C++ StealthUpdate.cpp:960-1097 changeVisualDisguise.
    fn change_visual_disguise(&mut self) {
        if dual_world_registry_unavailable() {
            return;
        }

        let applying = self.disguise_as_template_name.is_some();
        let revealing = !applying && self.disguise_as_player_index != -1;
        if !applying && !revealing {
            self.xfer_restore_disguise = false;
            return;
        }

        let snapshot = OBJECT_REGISTRY.with_object(self.object_id, |obj| {
            let selected = obj
                .get_drawable()
                .and_then(|d| d.read().ok().map(|g| g.is_selected()))
                .unwrap_or(false);
            let flags = obj
                .get_drawable()
                .and_then(|d| d.read().ok().map(|g| g.get_model_conditions()))
                .unwrap_or_default();
            let old_id = obj
                .get_drawable()
                .and_then(|d| d.read().ok().map(|g| g.get_drawable_id()));
            let pos = *obj.get_position();
            let orientation = obj.get_orientation();
            let true_template = obj.get_template().get_name().to_string();
            let own_color = obj.get_indicator_color();
            let own_night = obj.get_night_indicator_color();
            (
                selected,
                flags,
                old_id,
                pos,
                orientation,
                true_template,
                own_color,
                own_night,
            )
        });
        let Some((selected, flags, old_id, pos, orientation, true_template, own_color, own_night)) =
            snapshot
        else {
            self.xfer_restore_disguise = false;
            return;
        };

        let template_name = if applying {
            self.disguise_as_template_name.clone()
        } else {
            Some(true_template)
        };

        if let (Some(name), Some(client)) = (template_name, crate::helpers::TheGameClient::get()) {
            if let Some(template) = crate::helpers::TheThingFactory::find_template(&name) {
                if let Some(old) = old_id {
                    client.destroy_drawable(old);
                }
                let new_id = client.create_drawable(template.as_ref());
                crate::system::game_logic::bind_object_and_drawable(self.object_id, new_id);
                if let Some(drawable) = OBJECT_REGISTRY
                    .with_object(self.object_id, |obj| obj.get_drawable())
                    .flatten()
                {
                    if let Ok(mut draw) = drawable.write() {
                        draw.set_orientation(orientation);
                        draw.set_model_conditions(flags);
                    }
                }
                let _ = OBJECT_REGISTRY.with_object_mut(self.object_id, |obj| {
                    if let Some(physics) = obj.get_physics() {
                        crate::modules::PhysicsBehaviorExt::reset_dynamic_physics(&physics);
                    }
                    let _ = obj.set_position(&pos);
                });

                let night = crate::helpers::TheGlobalData::get()
                    .map(|g| g.get_time_of_day() == crate::common::audio::TimeOfDay::Night)
                    .unwrap_or(false);
                let color = if applying {
                    let disguise_player = player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_player(self.disguise_as_player_index).cloned());
                    let local = player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_local_player().cloned());
                    let allied_or_inactive = match local.as_ref() {
                        Some(local) => OBJECT_REGISTRY
                            .with_object(self.object_id, |obj| {
                                let Some(owner) = obj.get_controlling_player() else {
                                    return false;
                                };
                                match (owner.read(), local.read()) {
                                    (Ok(mine), Ok(client_p)) => {
                                        if !client_p.is_player_active() {
                                            return true;
                                        }
                                        let Some(team) = client_p.get_default_team() else {
                                            return false;
                                        };
                                        match team.read() {
                                            Ok(team_guard) => {
                                                mine.get_relationship_with_team(&team_guard)
                                                    == Relationship::Allies
                                            }
                                            Err(_) => false,
                                        }
                                    }
                                    _ => false,
                                }
                            })
                            .unwrap_or(false),
                        None => true,
                    };
                    if allied_or_inactive {
                        if night { own_night } else { own_color }
                    } else {
                        disguise_player
                            .and_then(|p| {
                                p.read().ok().map(|g| {
                                    if night {
                                        g.get_player_night_color()
                                    } else {
                                        g.get_player_color()
                                    }
                                })
                            })
                            .unwrap_or(own_color)
                    }
                } else if night {
                    own_night
                } else {
                    own_color
                };
                if let Some(drawable) = OBJECT_REGISTRY
                    .with_object(self.object_id, |obj| obj.get_drawable())
                    .flatten()
                {
                    if let Ok(mut draw) = drawable.write() {
                        draw.set_indicator_color(color);
                    }
                }
                let _ = selected;
            }
        }

        if applying {
            play_per_unit_sound(self.object_id, "DisguiseStarted");
            if let Some(fx_name) = self.data.disguise_fx.as_ref() {
                play_fx_at_object(self.object_id, fx_name);
            }
            self.disguised = true;
            let _ = OBJECT_REGISTRY.with_object_mut(self.object_id, |obj| {
                obj.set_status(ObjectStatusMaskType::DISGUISED, true);
                obj.set_model_condition_state(crate::common::ModelConditionFlags::DISGUISED);
                if let Some(player) = obj.get_controlling_player() {
                    if let Ok(mut player_guard) = player.write() {
                        player_guard
                            .get_academy_stats_mut()
                            .record_vehicle_disguised();
                    }
                }
            });
        } else {
            self.disguise_as_player_index = -1;
            let successful_reveal = OBJECT_REGISTRY
                .with_object(self.object_id, |obj| obj.get_current_victim_id().is_some())
                .unwrap_or(false);
            play_per_unit_sound(
                self.object_id,
                if successful_reveal {
                    "DisguiseRevealedSuccess"
                } else {
                    "DisguiseRevealedFailure"
                },
            );
            if let Some(fx_name) = self.data.disguise_reveal_fx.as_ref() {
                play_fx_at_object(self.object_id, fx_name);
            }
            self.disguised = false;
            let _ = OBJECT_REGISTRY.with_object_mut(self.object_id, |obj| {
                obj.set_status(ObjectStatusMaskType::DISGUISED, false);
                obj.clear_model_condition_state(crate::common::ModelConditionFlags::DISGUISED);
                obj.force_refresh_sub_object_upgrade_status();
            });
        }

        refresh_radar_for_object(self.object_id);
        self.xfer_restore_disguise = false;
    }

    pub fn is_stealthed(&self) -> bool {
        self.is_stealthed
    }

    pub fn is_disguised(&self) -> bool {
        self.disguise_as_template_name.is_some()
    }

    pub fn get_friendly_opacity(&self) -> Real {
        self.data.friendly_opacity_min
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
                .with_object(self.object_id, |guard| guard.get_controlling_player_id())
                .flatten()
                .map(|player_id| player_id as Int)
                .unwrap_or(0);
            self.enabled = true;
            self.transitioning_to_disguise = true;
            self.disguise_transition_frames = self.data.disguise_transition_frames;
            self.disguise_halfpoint_reached = false;
            TheGameLogic::set_wake_frame(self.object_id, UPDATE_SLEEP_NONE);
        } else if self.disguised || self.disguise_as_template_name.is_some() {
            self.disguise_as_template_name = None;
            self.disguise_as_player_index = 0;
            self.disguise_transition_frames = self.data.disguise_reveal_transition_frames;
            self.transitioning_to_disguise = false;
            self.disguise_halfpoint_reached = false;
            self.disguised = false;
        }
    }

    /// Get the stealth level mask (StealthForbiddenConditions bitmask)
    pub fn get_stealth_level(&self) -> UnsignedInt {
        self.data.stealth_level_mask
    }

    pub fn get_stealth_delay(&self) -> UnsignedInt {
        self.data.stealth_delay_frames
    }

    pub fn get_order_idle_enemies_to_attack_me_upon_reveal(&self) -> bool {
        self.data.order_idle_enemies_to_attack
    }

    pub fn enemy_detection_eva_event(&self) -> Option<&str> {
        self.data.enemy_detection_eva_event.as_deref()
    }

    pub fn own_detection_eva_event(&self) -> Option<&str> {
        self.data.own_detection_eva_event.as_deref()
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
        self.apply_detection(0);
    }

    pub fn mark_as_detected_for(&mut self, num_frames: UnsignedInt) {
        self.apply_detection(num_frames);
    }

    fn apply_detection(&mut self, num_frames: UnsignedInt) {
        let owner_id = self.calc_stealth_owner();
        let (stealth_delay, _, order_idles) = self.stealth_rules_for(owner_id);

        // C++ StealthUpdate.cpp:875-878 — permanently strip an active disguise.
        if self.disguise_as_template_name.is_some() || self.disguised {
            let now = TheGameLogic::get_frame();
            self.disguise_as_template(None, now);
        }

        let now = TheGameLogic::get_frame();
        if num_frames == 0 {
            self.detection_expires_frame = now.saturating_add(stealth_delay);
        } else if self.detection_expires_frame < now.saturating_add(num_frames) {
            self.detection_expires_frame = now.saturating_add(num_frames);
        }

        self.set_detected_status();

        if order_idles {
            self.order_idle_enemies_to_attack();
        }
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

    fn calc_stealth_owner(&self) -> ObjectID {
        // C++ StealthUpdate.cpp:531-556 — first contained rider when UseRiderStealth.
        if dual_world_registry_unavailable() || !self.data.use_rider_stealth {
            return self.object_id;
        }
        OBJECT_REGISTRY
            .with_object(self.object_id, |guard| {
                let Some(contain) = guard.get_contain() else {
                    return self.object_id;
                };
                contain
                    .lock()
                    .ok()
                    .and_then(|contain_guard| {
                        contain_guard.get_contained_objects().first().copied()
                    })
                    .unwrap_or(self.object_id)
            })
            .unwrap_or(self.object_id)
    }

    fn stealth_rules_for(&self, owner_id: ObjectID) -> (UnsignedInt, u32, bool) {
        if owner_id == self.object_id {
            return (
                self.data.stealth_delay_frames,
                self.data.stealth_level_mask,
                self.data.order_idle_enemies_to_attack,
            );
        }
        OBJECT_REGISTRY
            .with_object(owner_id, |owner| {
                owner.get_stealth().and_then(|handle| {
                    handle.lock().ok().map(|guard| {
                        (
                            guard.data.stealth_delay_frames,
                            guard.data.stealth_level_mask,
                            guard.data.order_idle_enemies_to_attack,
                        )
                    })
                })
            })
            .flatten()
            .unwrap_or((
                self.data.stealth_delay_frames,
                self.data.stealth_level_mask,
                self.data.order_idle_enemies_to_attack,
            ))
    }

    fn order_idle_enemies_to_attack(&self) {
        // C++ StealthUpdate.cpp:892-910 + setWakeupIfInRange 817-834.
        if dual_world_registry_unavailable() {
            return;
        }
        let Some((self_pos, self_player)) = OBJECT_REGISTRY.with_object(self.object_id, |obj| {
            (*obj.get_position(), obj.get_controlling_player())
        }) else {
            return;
        };
        let Some(self_player) = self_player else {
            return;
        };
        let Ok(list) = player_list().read() else {
            return;
        };
        for player in list.iter() {
            let is_enemy = match (player.read(), self_player.read()) {
                (Ok(other), Ok(mine)) => other.get_relationship(&mine) == Relationship::Enemies,
                _ => false,
            };
            if !is_enemy {
                continue;
            }
            let _ = player.iterate_objects(|enemy| {
                if let Ok(enemy_guard) = enemy.read() {
                    if enemy_guard.get_ai().is_some() {
                        let vision = enemy_guard.get_vision_range();
                        let delta = *enemy_guard.get_position() - self_pos;
                        if delta.length() <= vision {
                            TheGameLogic::set_wake_frame(enemy_guard.get_id(), UPDATE_SLEEP_NONE);
                        }
                    }
                }
                Ok(())
            });
        }
    }

    /// Apply a temporary stealth grant (simplified parity with C++).
    pub fn receive_grant(
        &mut self,
        grant: Bool,
        frames: UnsignedInt,
        current_frame: UnsignedInt,
    ) -> Result<(), StealthUpdateError> {
        // Wave 283: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

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
            if let Some(drawable) = OBJECT_REGISTRY
                .with_object(self.object_id, |obj_guard| obj_guard.get_drawable())
                .flatten()
            {
                if let Ok(mut draw_guard) = drawable.write() {
                    draw_guard.set_effective_opacity(1.0, None);
                }
            }
        }

        if let Some(rider_id) = OBJECT_REGISTRY
            .with_object(self.object_id, |obj_guard| {
                obj_guard
                    .get_contain()
                    .and_then(|contain| contain.lock().ok().and_then(|cg| cg.friend_get_rider()))
            })
            .flatten()
        {
            let _ = OBJECT_REGISTRY.with_object_mut(rider_id, |rider_guard| {
                if let Some(stealth) = rider_guard.get_stealth() {
                    if let Ok(mut stealth_guard) = stealth.lock() {
                        let _ = stealth_guard.receive_grant(grant, frames, current_frame);
                    }
                }
            });
        }

        Ok(())
    }

    pub fn allowed_to_stealth(&self, object: &Object) -> bool {
        let status = object.get_status_bits();
        status.contains(self.data.required_status())
            && !status.intersects(self.data.forbidden_status())
    }

    fn allowed_to_stealth_runtime(
        &mut self,
        object: &Object,
        current_frame: UnsignedInt,
        stealth_owner_id: ObjectID,
    ) -> bool {
        // Wave 283: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let flags = self.stealth_rules_for(stealth_owner_id).1;
        let status = object.get_status_bits();

        // C++ StealthUpdate.cpp:253-266 — pack stealth: if any slave is visible,
        // reveal everyone and refuse cloak.
        if object.is_kind_of(KindOf::SpawnsAreTheWeapons) {
            let mut all_stealthed = true;
            let _ = object.with_spawn_behavior_full_interface(|spawn| {
                if !spawn.are_all_slaves_stealthed() {
                    let _ = spawn.reveal_slaves();
                    all_stealthed = false;
                }
            });
            if !all_stealthed {
                return false;
            }
        }

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

        // C++ StealthUpdate.cpp:294 — CAN_STEALTH is tested on the stealth owner (rider).
        let can_stealth = if stealth_owner_id == self.object_id {
            status.contains(ObjectStatusMaskType::CAN_STEALTH)
        } else {
            OBJECT_REGISTRY
                .with_object(stealth_owner_id, |owner| {
                    owner
                        .get_status_bits()
                        .contains(ObjectStatusMaskType::CAN_STEALTH)
                })
                .unwrap_or(false)
        };
        if !can_stealth {
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

        if let Some(container_id) = object.get_container_id() {
            let not_garrisonable = crate::object::registry::OBJECT_REGISTRY
                .with_object(container_id, |container_guard| {
                    let Some(contain) = container_guard.get_contain() else {
                        return false;
                    };
                    contain
                        .lock()
                        .ok()
                        .map(|contain_guard| !contain_guard.is_garrisonable())
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if not_garrisonable {
                return false;
            }
        }

        if (flags & STEALTH_NOT_WHILE_RIDERS_ATTACKING) != 0 {
            if let Some(contain) = object.get_contain() {
                if let Ok(contain_guard) = contain.lock() {
                    if contain_guard.is_passenger_allowed_to_fire(None) {
                        for rider_id in contain_guard.get_contained_objects() {
                            let Some(attacking) =
                                OBJECT_REGISTRY.with_object(*rider_id, |rider_guard| {
                                    let rider_status = rider_guard.get_status_bits();
                                    rider_status.contains(ObjectStatusMaskType::IS_ATTACKING)
                                        || rider_status
                                            .contains(ObjectStatusMaskType::IS_FIRING_WEAPON)
                                })
                            else {
                                continue;
                            };
                            if attacking {
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
                    if leftover_not_while_moving_destalths(
                        physics_guard.get_velocity().length(),
                        self.data.stealth_speed,
                    ) {
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
        // Wave 283: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.data.reveal_distance_from_target <= 0.0 {
            return false;
        }
        let Some(victim_id) = object.get_current_victim_id() else {
            return false;
        };
        let reveal_dist = self.data.reveal_distance_from_target;
        let Some(dist_sq) =
            crate::object::registry::OBJECT_REGISTRY.with_object(victim_id, |victim_guard| {
                ThePartitionManager::get_distance_squared(object, victim_guard, FROM_CENTER_2D)
            })
        else {
            return false;
        };
        dist_sq <= reveal_dist * reveal_dist
    }

    fn check_black_market_available(&self, owner: &Object) -> bool {
        // Wave 283: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let Some(player) = owner.get_controlling_player() else {
            return false;
        };

        let mut has_black_market = false;
        if let Ok(player_guard) = player.read() {
            let _ = player_guard.iterate_object_ids(|object_id| {
                if has_black_market {
                    return Ok(());
                }

                let object_arc = match crate::helpers::TheGameLogic::find_object_by_id(object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(object_id))
                {
                    Some(a) => a,
                    None => return Ok(()),
                };
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

                // C++ StealthUpdate.cpp:157-175 isBlackMarket — KindOf, not name.
                if object_guard.is_kind_of(KindOf::FsBlackMarket) {
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
        // Wave 283: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = OBJECT_REGISTRY.with_object_mut(self.object_id, |guard| {
            guard.set_stealth_module(self.controller.clone());
        });
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

impl UpdateModuleInterface for StealthUpdateModule {
    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        // C++ StealthUpdate.h:103 — still ticks while DISABLED_HELD.
        DisabledMaskType::HELD
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
        // C++ StealthUpdate::loadPostProcess: if (isDisguised()) where
        // isDisguised() is m_disguiseAsTemplate != NULL, not m_disguised.
        if controller.disguise_as_template_name.is_some() {
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
    use game_engine::common::system::snapshot::Snapshotable;
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

        fn xfer_snapshot(&mut self, _snapshot: &mut dyn Snapshotable) -> Result<(), XferStatus> {
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

        assert!(
            data.hint_detectable_states()
                .contains(ObjectStatusMaskType::STEALTHED)
        );
        assert!(
            data.hint_detectable_states()
                .contains(ObjectStatusMaskType::DETECTED)
        );
        assert!(
            data.required_status()
                .contains(ObjectStatusMaskType::CAN_STEALTH)
        );
        assert!(
            data.forbidden_status()
                .contains(ObjectStatusMaskType::MASKED)
        );
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
            assert!(
                guard
                    .get_status_bits()
                    .contains(ObjectStatusMaskType::STEALTHED)
            );
        }

        {
            let mut guard = controller.lock().unwrap();
            guard.mark_as_detected();
            guard.end_stealth().expect("end stealth");
        }

        {
            let guard = object.read().unwrap();
            assert!(
                !guard
                    .get_status_bits()
                    .contains(ObjectStatusMaskType::STEALTHED)
            );
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

    #[test]
    fn mark_as_detected_strips_active_disguise() {
        let object_id: ObjectID = 4246;
        let object = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        OBJECT_REGISTRY.register_object(object_id, &object);

        let mut data = StealthUpdateModuleData::default();
        data.order_idle_enemies_to_attack = true;
        data.use_rider_stealth = true;
        let mut controller = StealthController::new(Arc::new(data), object_id);
        controller.disguise_as_template(Some("GLAVehicleBombTruck".to_string()), 0);
        assert!(controller.is_disguised());

        controller.mark_as_detected();
        assert!(!controller.is_disguised());
        assert!(controller.detection_expires_frame > 0);

        OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn stealth_defaults_match_cpp() {
        let data = StealthUpdateModuleData::default();
        assert_eq!(data.stealth_delay_frames, u32::MAX);
        assert_eq!(data.pulse_frames, 30);
        assert!(data.innate_stealth);
        assert!(!data.use_rider_stealth);
    }

    #[test]
    fn leftover_not_while_moving_destalths_uses_move_threshold_speed() {
        assert!(!leftover_not_while_moving_destalths(0.0, 3.0));
        assert!(!leftover_not_while_moving_destalths(3.0, 3.0));
        assert!(leftover_not_while_moving_destalths(3.1, 3.0));
        assert!(leftover_not_while_moving_destalths(0.1, 0.0));
        assert!(!leftover_not_while_moving_destalths(0.0, 0.0));
        assert_eq!(leftover_stealth_move_threshold_speed(""), None);
    }

    #[test]
    fn stealth_module_ticks_while_held() {
        let data = Arc::new(StealthUpdateModuleData::default());
        let module = StealthUpdateModule::new(1, data, 99);
        assert_eq!(
            UpdateModuleInterface::get_disabled_types_to_process(&module),
            DisabledMaskType::HELD
        );
    }
}
