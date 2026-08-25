//! Host MissileLauncherBuildingUpdate door state machine.
//!
//! C++ `MissileLauncherBuildingUpdate` keys DOOR_1 model conditions to the
//! special-power ready frame (Scud Storm, Nuclear Missile, Cruise Missile).
//! Leftover GameLogic already matches C++; live host objects never enter
//! leftover `OBJECT_REGISTRY`, so Wave 382 fail-closes that update.
//!
//! This residual walks leftover ThingFactory module data when loaded, else
//! retail door-time residuals, and applies the same switchToState / update
//! transitions onto host `model_condition_bits`.

use crate::command_system::SpecialPowerType;
use serde::{Deserialize, Serialize};

/// C++ `DoorStateType` (MissileLauncherBuildingUpdate.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum HostMissileLauncherDoorState {
    #[default]
    Closed,
    Opening,
    Open,
    WaitingToClose,
    Closing,
}

/// Retail MissileLauncherBuildingUpdate DoorOpenTime (msec).
pub const MISSILE_LAUNCHER_DOOR_OPEN_TIME_MS: u32 = 8000;
/// DoorOpenTime frames (8000 ms → 240).
pub const MISSILE_LAUNCHER_DOOR_OPEN_TIME_FRAMES: u32 = 240;
/// Retail DoorWaitOpenTime (msec).
pub const MISSILE_LAUNCHER_DOOR_WAIT_OPEN_TIME_MS: u32 = 2000;
/// DoorWaitOpenTime frames (2000 ms → 60).
pub const MISSILE_LAUNCHER_DOOR_WAIT_OPEN_TIME_FRAMES: u32 = 60;
/// Retail DoorCloseTime (msec) — same as DoorOpenTime on Scud / Nuke silos.
pub const MISSILE_LAUNCHER_DOOR_CLOSE_TIME_MS: u32 = 8000;
/// DoorCloseTime frames (8000 ms → 240).
pub const MISSILE_LAUNCHER_DOOR_CLOSE_TIME_FRAMES: u32 = 240;

/// Authored INI / leftover module snapshot used by the host door SM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostMissileLauncherIni {
    pub special_power_template_name: String,
    pub door_open_time: u32,
    pub door_wait_open_time: u32,
    pub door_closing_time: u32,
    pub opening_fx: Option<String>,
    pub open_fx: Option<String>,
    pub waiting_to_close_fx: Option<String>,
    pub closing_fx: Option<String>,
    pub closed_fx: Option<String>,
    pub open_idle_audio: Option<String>,
}

impl Default for HostMissileLauncherIni {
    fn default() -> Self {
        Self {
            special_power_template_name: String::new(),
            door_open_time: MISSILE_LAUNCHER_DOOR_OPEN_TIME_FRAMES,
            door_wait_open_time: MISSILE_LAUNCHER_DOOR_WAIT_OPEN_TIME_FRAMES,
            door_closing_time: MISSILE_LAUNCHER_DOOR_CLOSE_TIME_FRAMES,
            opening_fx: None,
            open_fx: None,
            waiting_to_close_fx: None,
            closing_fx: None,
            closed_fx: None,
            open_idle_audio: None,
        }
    }
}

/// Live door SM state (C++ xfer: door / timeout / timeoutFrame).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostMissileLauncherBuildingUpdateData {
    pub ini: HostMissileLauncherIni,
    pub door_state: HostMissileLauncherDoorState,
    pub timeout_state: HostMissileLauncherDoorState,
    pub timeout_frame: u32,
    /// C++ `initiateIntentToDoSpecialPower` pending until the next host tick.
    pub pending_initiate: bool,
    /// C++ `Drawable::setAnimationLoopDuration` residual.
    pub animation_loop_duration: u32,
    /// Last FX / idle-audio cue emitted by a state switch (drained by host).
    pub pending_fx: Option<String>,
    pub pending_idle_audio: Option<String>,
    pub stop_idle_audio: bool,
}

impl Default for HostMissileLauncherBuildingUpdateData {
    fn default() -> Self {
        Self {
            ini: HostMissileLauncherIni::default(),
            door_state: HostMissileLauncherDoorState::Closed,
            timeout_state: HostMissileLauncherDoorState::Closed,
            timeout_frame: 0,
            pending_initiate: false,
            animation_loop_duration: 0,
            pending_fx: None,
            pending_idle_audio: None,
            stop_idle_audio: false,
        }
    }
}

impl HostMissileLauncherBuildingUpdateData {
    pub fn from_ini(ini: HostMissileLauncherIni) -> Self {
        Self {
            ini,
            ..Self::default()
        }
    }

    /// C++ `initiateIntentToDoSpecialPower` → `switchToState(DOOR_WAITING_TO_CLOSE)`.
    pub fn initiate_intent(&mut self, now: u32) {
        self.pending_initiate = false;
        self.switch_to_state(HostMissileLauncherDoorState::WaitingToClose, now, 0);
    }

    /// C++ `MissileLauncherBuildingUpdate::update` after under-construction gate.
    ///
    /// `ready_frame` is `SpecialPowerModule::getReadyFrame()`. `is_ready` is
    /// `SpecialPowerModule::isReady()`.
    pub fn update(&mut self, now: u32, ready_frame: u32, is_ready: bool) {
        if self.pending_initiate {
            self.initiate_intent(now);
        }
        let when_to_start_opening = ready_frame.saturating_sub(self.ini.door_open_time);
        if self.timeout_frame != 0 && now > self.timeout_frame {
            let next = self.timeout_state;
            self.switch_to_state(next, now, ready_frame);
        }
        if self.door_state != HostMissileLauncherDoorState::Open && is_ready {
            self.switch_to_state(HostMissileLauncherDoorState::Open, now, ready_frame);
        } else if self.door_state == HostMissileLauncherDoorState::Closed
            && now >= when_to_start_opening
        {
            self.switch_to_state(HostMissileLauncherDoorState::Opening, now, ready_frame);
        }
    }

    /// C++ `switchToState`.
    pub fn switch_to_state(
        &mut self,
        dst: HostMissileLauncherDoorState,
        now: u32,
        ready_frame: u32,
    ) {
        if self.door_state == dst {
            return;
        }
        self.pending_fx = None;
        self.pending_idle_audio = None;
        self.stop_idle_audio = false;
        self.animation_loop_duration = 0;
        match dst {
            HostMissileLauncherDoorState::Closed => {
                self.timeout_frame = 0;
                self.timeout_state = HostMissileLauncherDoorState::Closed;
                self.pending_fx = self.ini.closed_fx.clone();
                self.stop_idle_audio = true;
            }
            HostMissileLauncherDoorState::Opening => {
                // C++: end one frame before ready.
                self.timeout_frame = ready_frame.saturating_sub(1);
                if self.timeout_frame == 0 {
                    self.timeout_frame = now;
                }
                self.timeout_state = HostMissileLauncherDoorState::Open;
                self.pending_fx = self.ini.opening_fx.clone();
                self.stop_idle_audio = true;
            }
            HostMissileLauncherDoorState::Open => {
                self.timeout_frame = 0;
                self.timeout_state = HostMissileLauncherDoorState::Open;
                self.pending_fx = self.ini.open_fx.clone();
                if let Some(audio) = &self.ini.open_idle_audio {
                    if !audio.is_empty() {
                        self.pending_idle_audio = Some(audio.clone());
                    }
                }
            }
            HostMissileLauncherDoorState::WaitingToClose => {
                self.timeout_frame = now.saturating_add(self.ini.door_wait_open_time);
                self.timeout_state = HostMissileLauncherDoorState::Closing;
                self.pending_fx = self.ini.waiting_to_close_fx.clone();
                self.stop_idle_audio = true;
            }
            HostMissileLauncherDoorState::Closing => {
                self.timeout_frame = now.saturating_add(self.ini.door_closing_time);
                let delta = ready_frame.saturating_sub(now);
                let half = now.saturating_add(delta / 2);
                if self.timeout_frame > half {
                    self.timeout_frame = half;
                }
                self.timeout_state = HostMissileLauncherDoorState::Closed;
                self.pending_fx = self.ini.closing_fx.clone();
                self.stop_idle_audio = true;
            }
        }
        self.door_state = dst;
        if self.timeout_frame > now {
            self.animation_loop_duration = self.timeout_frame - now;
        }
    }
}

/// Template names that author MissileLauncherBuildingUpdate without leftover factory.
pub fn is_missile_launcher_building_template(template_name: &str) -> bool {
    leftover_missile_launcher_module_data(template_name).is_some()
        || residual_missile_launcher_ini(template_name).is_some()
}

/// Map leftover SpecialPowerTemplate / host template name to the door power.
pub fn missile_launcher_special_power(template_name: &str) -> Option<SpecialPowerType> {
    if let Some(ini) = leftover_missile_launcher_module_data(template_name) {
        if let Some(p) = special_power_from_template_name(&ini.special_power_template_name) {
            return Some(p);
        }
    }
    special_power_from_object_template(template_name)
}

fn special_power_from_template_name(name: &str) -> Option<SpecialPowerType> {
    let n = name.to_ascii_lowercase();
    if n.contains("scud") {
        Some(SpecialPowerType::ScudStorm)
    } else if n.contains("cruise") {
        Some(SpecialPowerType::CruiseMissile)
    } else if n.contains("neutron") || n.contains("nuclear") || n.contains("nuke") {
        Some(SpecialPowerType::NuclearMissile)
    } else {
        None
    }
}

fn special_power_from_object_template(template_name: &str) -> Option<SpecialPowerType> {
    let n = template_name.to_ascii_lowercase();
    if n.contains("scudstorm") {
        Some(SpecialPowerType::ScudStorm)
    } else if n.contains("nuclearmissilelauncher") || n.contains("nuclearmissile") {
        Some(SpecialPowerType::NuclearMissile)
    } else if n.contains("cruisemissile") && !n.contains("weapon") {
        Some(SpecialPowerType::CruiseMissile)
    } else {
        None
    }
}

/// Leftover ThingFactory `MissileLauncherBuildingUpdate` module, if already loaded.
pub fn leftover_missile_launcher_module_data(
    template_name: &str,
) -> Option<HostMissileLauncherIni> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    for entry in tmpl.get_behavior_module_info().iter() {
        if !entry
            .name
            .as_str()
            .eq_ignore_ascii_case("MissileLauncherBuildingUpdate")
        {
            continue;
        }
        if let Some(data) = entry
            .data
            .downcast_ref::<gamelogic::object::behavior::MissileLauncherBuildingUpdateModuleData>(
        ) {
            return Some(HostMissileLauncherIni {
                special_power_template_name: data.special_power_template_name.clone(),
                door_open_time: data.door_open_time,
                door_wait_open_time: data.door_wait_open_time,
                door_closing_time: data.door_closing_time,
                opening_fx: data.opening_fx.clone(),
                open_fx: data.open_fx.clone(),
                waiting_to_close_fx: data.waiting_to_close_fx.clone(),
                closing_fx: data.closing_fx.clone(),
                closed_fx: data.closed_fx.clone(),
                open_idle_audio: data.open_idle_audio.clone(),
            });
        }
        let parse_dur = |key: &str| -> Option<u32> {
            entry
                .data
                .get_ini_field(key)
                .and_then(|s| s.parse::<u32>().ok())
                .map(|ms| ((ms as f32) * 30.0 / 1000.0).ceil() as u32)
        };
        return Some(HostMissileLauncherIni {
            special_power_template_name: entry
                .data
                .get_ini_field("SpecialPowerTemplate")
                .unwrap_or("")
                .to_string(),
            door_open_time: parse_dur("DoorOpenTime")
                .unwrap_or(MISSILE_LAUNCHER_DOOR_OPEN_TIME_FRAMES),
            door_wait_open_time: parse_dur("DoorWaitOpenTime")
                .unwrap_or(MISSILE_LAUNCHER_DOOR_WAIT_OPEN_TIME_FRAMES),
            door_closing_time: parse_dur("DoorCloseTime")
                .unwrap_or(MISSILE_LAUNCHER_DOOR_CLOSE_TIME_FRAMES),
            opening_fx: nonempty_ini(entry.data.get_ini_field("DoorOpeningFX")),
            open_fx: nonempty_ini(entry.data.get_ini_field("DoorOpenFX")),
            waiting_to_close_fx: nonempty_ini(entry.data.get_ini_field("DoorWaitingToCloseFX")),
            closing_fx: nonempty_ini(entry.data.get_ini_field("DoorClosingFX")),
            closed_fx: nonempty_ini(entry.data.get_ini_field("DoorClosedFX")),
            open_idle_audio: nonempty_ini(entry.data.get_ini_field("DoorOpenIdleAudio")),
        });
    }
    None
}

fn nonempty_ini(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("NONE"))
        .map(str::to_string)
}

/// Retail residual when leftover ThingFactory is empty.
pub fn residual_missile_launcher_ini(template_name: &str) -> Option<HostMissileLauncherIni> {
    let power = special_power_from_object_template(template_name)?;
    let mut ini = HostMissileLauncherIni::default();
    match power {
        SpecialPowerType::ScudStorm => {
            ini.special_power_template_name = "SuperweaponScudStorm".into();
            ini.open_idle_audio = Some("ScudStormIdleLoop".into());
        }
        SpecialPowerType::NuclearMissile => {
            ini.special_power_template_name = "SuperweaponNeutronMissile".into();
        }
        SpecialPowerType::CruiseMissile => {
            ini.special_power_template_name = "SupW_CruiseMissile".into();
            ini.door_open_time =
                crate::game_logic::special_power_strikes::CRUISE_MISSILE_DOOR_OPEN_TIME_FRAMES;
            ini.door_wait_open_time =
                crate::game_logic::special_power_strikes::CRUISE_MISSILE_DOOR_WAIT_OPEN_TIME_FRAMES;
        }
        _ => return None,
    }
    Some(ini)
}

pub fn missile_launcher_ini_for_template(template_name: &str) -> Option<HostMissileLauncherIni> {
    leftover_missile_launcher_module_data(template_name)
        .or_else(|| residual_missile_launcher_ini(template_name))
}

pub fn honesty_missile_launcher_building_update_residual_ok() -> bool {
    MISSILE_LAUNCHER_DOOR_OPEN_TIME_FRAMES == 240
        && MISSILE_LAUNCHER_DOOR_WAIT_OPEN_TIME_FRAMES == 60
        && MISSILE_LAUNCHER_DOOR_CLOSE_TIME_FRAMES == 240
        && is_missile_launcher_building_template("GLAScudStorm")
        && is_missile_launcher_building_template("ChinaNuclearMissileLauncher")
        && !is_missile_launcher_building_template("AmericaTankCrusader")
        && missile_launcher_special_power("GLAScudStorm") == Some(SpecialPowerType::ScudStorm)
        && {
            let mut d = HostMissileLauncherBuildingUpdateData::from_ini(
                residual_missile_launcher_ini("GLAScudStorm").unwrap(),
            );
            d.update(0, 240, false);
            d.door_state == HostMissileLauncherDoorState::Opening
                && {
                    d.update(239, 240, true);
                    d.door_state == HostMissileLauncherDoorState::Open
                }
                && {
                    d.initiate_intent(300);
                    d.door_state == HostMissileLauncherDoorState::WaitingToClose
                }
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ini_with_door_fx() -> HostMissileLauncherIni {
        let mut ini = residual_missile_launcher_ini("GLAScudStorm").unwrap();
        ini.opening_fx = Some("FX_TestDoorOpening".into());
        ini.open_fx = Some("FX_TestDoorOpen".into());
        ini.waiting_to_close_fx = Some("FX_TestDoorWaitClose".into());
        ini.closing_fx = Some("FX_TestDoorClosing".into());
        ini.closed_fx = Some("FX_TestDoorClosed".into());
        ini
    }

    #[test]
    fn residual_ini_does_not_invent_door_fx_names() {
        let ini = residual_missile_launcher_ini("GLAScudStorm").unwrap();
        assert!(ini.opening_fx.is_none());
        assert!(ini.open_fx.is_none());
        assert!(ini.waiting_to_close_fx.is_none());
        assert!(ini.closing_fx.is_none());
        assert!(ini.closed_fx.is_none());
    }

    #[test]
    fn door_state_enter_stashes_pending_fx_like_cpp_do_fx_pos() {
        let mut d = HostMissileLauncherBuildingUpdateData::from_ini(ini_with_door_fx());
        d.update(0, 240, false);
        assert_eq!(d.door_state, HostMissileLauncherDoorState::Opening);
        assert_eq!(d.pending_fx.as_deref(), Some("FX_TestDoorOpening"));
        d.update(239, 240, true);
        assert_eq!(d.door_state, HostMissileLauncherDoorState::Open);
        assert_eq!(d.pending_fx.as_deref(), Some("FX_TestDoorOpen"));
        d.initiate_intent(300);
        assert_eq!(d.door_state, HostMissileLauncherDoorState::WaitingToClose);
        assert_eq!(d.pending_fx.as_deref(), Some("FX_TestDoorWaitClose"));
        d.update(361, 1000, false);
        assert_eq!(d.door_state, HostMissileLauncherDoorState::Closing);
        assert_eq!(d.pending_fx.as_deref(), Some("FX_TestDoorClosing"));
        let close_at = d.timeout_frame;
        d.update(close_at + 1, 1000, false);
        assert_eq!(d.door_state, HostMissileLauncherDoorState::Closed);
        assert_eq!(d.pending_fx.as_deref(), Some("FX_TestDoorClosed"));
    }

    #[test]
    fn door_open_stashes_pending_idle_audio_like_cpp() {
        let mut d = HostMissileLauncherBuildingUpdateData::from_ini(
            residual_missile_launcher_ini("GLAScudStorm").unwrap(),
        );
        d.update(0, 240, false);
        assert!(d.pending_idle_audio.is_none());
        assert!(d.stop_idle_audio);
        d.update(239, 240, true);
        assert_eq!(d.door_state, HostMissileLauncherDoorState::Open);
        assert_eq!(d.pending_idle_audio.as_deref(), Some("ScudStormIdleLoop"));
        assert!(!d.stop_idle_audio);
        d.initiate_intent(300);
        assert!(d.stop_idle_audio);
        assert!(d.pending_idle_audio.is_none());
    }

    #[test]
    fn live_tick_dispatches_pending_door_fx_at_building_pos() {
        let tick = include_str!("object/update.rs");
        let start = tick
            .find("pub fn tick_missile_launcher_building")
            .expect("tick_missile_launcher_building");
        let win = &tick[start..start + 2800];
        assert!(
            win.contains("let pending_fx = data.pending_fx.take()"),
            "door pending_fx must be consumed: {win}"
        );
        assert!(
            win.contains("let pending_idle = data.pending_idle_audio.take()"),
            "DoorOpenIdleAudio pending_idle_audio must be consumed: {win}"
        );
        assert!(
            win.contains("let stop_idle = std::mem::take(&mut data.stop_idle_audio)"),
            "DoorOpenIdleAudio stop_idle_audio must be consumed: {win}"
        );
        assert!(
            win.contains("dispatch_fx_list_at_pos(&fx, self.get_position())"),
            "C++ doFXPos at building missing: {win}"
        );
    }
}
