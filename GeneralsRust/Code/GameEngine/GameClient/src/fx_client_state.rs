//! Per-client FX hooks and scene admission (was process TLS in `fx_list`).
//!
//! C++ `TheDisplay` light pulses and `TheAudio` FX routing belong to the
//! receiving GameClient. Nugget dispatch borrows these through
//! [`crate::fx_list::FxViewServices`].

use std::sync::{Arc, Mutex};

use crate::display::cinematic_camera::CameraShakeSystem;
use crate::effects::decals::DecalManager;
use crate::fx_list::{DisplayLightPulse, DisplayLightScene};
use crate::message_stream::game_message::Coord3D as MessageCoord3D;

pub type AudioHook = Box<dyn FnMut(&str, Option<MessageCoord3D>) + Send + Sync>;
pub type LightPulseHook = Box<dyn FnMut(&DisplayLightPulse) + Send + Sync>;

/// Live FX presentation hooks for one GameClient.
#[derive(Default)]
pub struct FxClientState {
    pub scene: DisplayLightScene,
    audio: Option<AudioHook>,
    decal_manager: Option<Arc<Mutex<DecalManager>>>,
    /// Optional display callback when a light pulse is admitted (W3D scene).
    light_pulse_hook: Option<LightPulseHook>,
    /// Retained for API compatibility; ViewShake nuggets use FxViewServices.
    #[allow(dead_code)]
    camera_shake: Option<Arc<Mutex<CameraShakeSystem>>>,
}

impl FxClientState {
    pub fn register_audio(&mut self, hook: AudioHook) {
        self.audio = Some(hook);
    }

    pub fn register_decal_manager(&mut self, manager: Arc<Mutex<DecalManager>>) {
        self.decal_manager = Some(manager);
    }

    pub fn decal_manager(&self) -> Option<Arc<Mutex<DecalManager>>> {
        self.decal_manager.clone()
    }

    pub fn register_camera_shake_system(&mut self, system: Arc<Mutex<CameraShakeSystem>>) {
        self.camera_shake = Some(system);
    }

    pub fn register_light_pulse_hook(&mut self, hook: LightPulseHook) {
        self.light_pulse_hook = Some(hook);
    }

    /// C++ `TheDisplay->createLightPulse`.
    pub fn create_display_light_pulse(&mut self, pulse: DisplayLightPulse) -> bool {
        if !self.scene.create_pulse(pulse.clone()) {
            return false;
        }
        if let Some(hook) = self.light_pulse_hook.as_mut() {
            hook(&pulse);
        }
        true
    }

    pub fn scene_dynamic_lights(&self) -> Vec<crate::fx_list::DisplayDynamicLight> {
        self.scene.lights().to_vec()
    }

    pub fn tick_scene_dynamic_lights(&mut self) {
        self.scene.update();
    }

    pub fn clear_scene_dynamic_lights(&mut self) {
        self.scene.clear_lights();
    }

    pub fn drain_display_light_pulses(&mut self) -> Vec<DisplayLightPulse> {
        self.scene.drain_pulses()
    }

    /// Invoke the registered FX audio hook if present.
    pub fn with_audio<F: FnOnce(&mut AudioHook)>(&mut self, f: F) -> bool {
        if let Some(ref mut hook) = self.audio {
            f(hook);
            true
        } else {
            false
        }
    }
}
