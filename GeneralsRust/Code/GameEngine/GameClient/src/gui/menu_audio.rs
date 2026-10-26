//! Immediate menu sound operations on the receiving native service.
//! Handles retain their service lifetime; queued audio requests are a separate API.
use std::sync::{Arc, Weak};

#[derive(Debug, Clone)]
pub struct MenuAudioHandle {
    raw: u32,
    owner: Weak<()>,
}

/// Identity for one actual menu-audio service generation, without mutable sharing.
#[derive(Debug, Default)]
pub struct MenuAudioLifetime(Arc<()>);
impl MenuAudioLifetime {
    pub fn handle(&self, raw: u32) -> MenuAudioHandle {
        MenuAudioHandle {
            raw,
            owner: Arc::downgrade(&self.0),
        }
    }
    pub fn resolve(&self, handle: &MenuAudioHandle) -> Result<u32, String> {
        if !handle.owner.ptr_eq(&Arc::downgrade(&self.0)) {
            return Err("menu audio handle belongs to another service lifetime".into());
        }
        Ok(handle.raw)
    }
}

impl PartialEq for MenuAudioHandle {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw && self.owner.ptr_eq(&other.owner)
    }
}
impl Eq for MenuAudioHandle {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAudioStop {
    Stopped,
    AlreadyEnded,
    NotOwned,
}

pub trait MenuAudioEffects {
    /// Returns the actual device handle; muted/NoSound returns None.
    fn is_bound(&self) -> bool;
    fn play(&mut self, name: &str) -> Option<MenuAudioHandle>;
    fn play_fading(&mut self, name: &str) -> Option<MenuAudioHandle>;
    fn stop(&mut self, handle: MenuAudioHandle) -> MenuAudioStop;
    fn pump(&mut self);
    fn fade_out_music(&mut self);
}

/// Deliberate device-less service for actual headless clients and fixtures.
/// Missing service is a checked admission error, never implicitly Silent.
#[derive(Debug, Default)]
pub struct SilentMenuAudio;
impl SilentMenuAudio {
    pub fn new() -> Self {
        Self
    }
}
impl MenuAudioEffects for SilentMenuAudio {
    fn is_bound(&self) -> bool {
        true
    }
    fn play(&mut self, _: &str) -> Option<MenuAudioHandle> {
        None
    }
    fn play_fading(&mut self, _: &str) -> Option<MenuAudioHandle> {
        None
    }
    fn stop(&mut self, _: MenuAudioHandle) -> MenuAudioStop {
        MenuAudioStop::NotOwned
    }
    fn pump(&mut self) {}
    fn fade_out_music(&mut self) {}
}

use game_engine::common::audio::audio_event_rts::{AudioEventSelection, Coord3D};
use game_engine::common::audio::game_audio::AudioViewObservation;
use game_engine::common::audio::{AudioEventRts, AudioManager, AudioSubmissionLocality};
use game_engine::common::random_value::RandomState;
use std::sync::Mutex;

/// Completed receiving camera observation. Querying never advances simulation.
pub trait MenuAudioView {
    fn observation(&self) -> AudioViewObservation;
    fn is_on_screen(&self, position: &Coord3D) -> bool;
}

#[derive(Default)]
pub struct CommonMenuAudioService {
    device: Option<Arc<Mutex<AudioManager>>>,
    lifetime: MenuAudioLifetime,
    handles: Vec<u32>,
}
impl CommonMenuAudioService {
    pub fn bind(&mut self, device: Arc<Mutex<AudioManager>>) {
        assert!(
            self.device.is_none(),
            "menu audio binds only at explicit native init"
        );
        self.device = Some(device);
    }
    pub fn require_bound(&self) -> Result<(), String> {
        self.device
            .as_ref()
            .map(|_| ())
            .ok_or_else(|| "receiving menu audio service is not initialized".into())
    }
    pub fn lend<'a, V: MenuAudioView>(
        &'a mut self,
        random: &'a mut RandomState,
        policy: &'a game_engine::common::audio::script_policy::ScriptAudioPolicy,
        locality: AudioSubmissionLocality,
        view: V,
    ) -> CommonMenuAudio<'a, V> {
        CommonMenuAudio {
            service: self,
            random,
            policy,
            locality,
            view,
        }
    }
    pub fn visit_owned_receipts(&self, visit: &mut dyn FnMut(&Arc<Mutex<AudioManager>>, &[u32])) {
        if let Some(device) = &self.device {
            visit(device, &self.handles);
        }
    }
    pub fn reset(&mut self) {
        if let Some(device) = &self.device {
            let mut device = device.lock().expect("receiving menu audio device poisoned");
            device.retire_audio_handles(&self.handles);
            self.handles.clear();
        }
        self.lifetime = MenuAudioLifetime::default();
    }
}
impl Drop for CommonMenuAudioService {
    fn drop(&mut self) {
        self.reset();
    }
}

pub struct CommonMenuAudio<'a, V: MenuAudioView> {
    service: &'a mut CommonMenuAudioService,
    random: &'a mut RandomState,
    policy: &'a game_engine::common::audio::script_policy::ScriptAudioPolicy,
    locality: AudioSubmissionLocality,
    view: V,
}
struct MenuSelection<'a>(&'a mut RandomState);
impl AudioEventSelection for MenuSelection<'_> {
    fn index(&mut self, logical: bool, count: usize) -> usize {
        assert!(
            !logical,
            "menu sound cannot consume authoritative random stream"
        );
        if count <= 1 {
            0
        } else {
            self.0.next_u32() as usize % count
        }
    }
    fn real(&mut self, low: f32, high: f32) -> f32 {
        self.0.next_real(low, high)
    }
}
impl<V: MenuAudioView> CommonMenuAudio<'_, V> {
    pub fn random_mut(&mut self) -> &mut RandomState {
        self.random
    }

    fn play_event(&mut self, name: &str, fade: bool) -> Option<MenuAudioHandle> {
        self.service
            .require_bound()
            .expect("receiving menu admission must check service binding");
        let mut event = AudioEventRts::new();
        event.set_event_name(name.to_owned());
        event.set_should_fade(fade);
        // No logical flag: C++ menu audio is presentation, never a gameplay draw.
        let handle = self
            .service
            .device
            .as_ref()
            .unwrap()
            .lock()
            .expect("receiving menu audio device poisoned")
            .add_audio_event_with_policy(
                &event,
                &self.locality,
                &mut MenuSelection(self.random),
                self.policy,
                false,
            );
        use game_engine::common::audio::game_audio::{
            AHSV_ERROR, AHSV_MUTED, AHSV_NO_SOUND, AHSV_NOT_FOR_LOCAL,
        };
        if matches!(handle, AHSV_NO_SOUND | AHSV_MUTED | AHSV_NOT_FOR_LOCAL) {
            return None;
        }
        assert_ne!(
            handle, AHSV_ERROR,
            "authored menu audio event {name} is not configured"
        );
        assert!(
            handle >= 1000 && handle < 0xFFFF_FFF0,
            "unexpected menu audio handle"
        );
        self.service.handles.push(handle);
        Some(self.service.lifetime.handle(handle))
    }
}
impl<V: MenuAudioView> MenuAudioEffects for CommonMenuAudio<'_, V> {
    fn is_bound(&self) -> bool {
        self.service.device.is_some()
    }
    fn play(&mut self, name: &str) -> Option<MenuAudioHandle> {
        self.play_event(name, false)
    }
    fn play_fading(&mut self, name: &str) -> Option<MenuAudioHandle> {
        self.play_event(name, true)
    }
    fn stop(&mut self, receipt: MenuAudioHandle) -> MenuAudioStop {
        let Ok(handle) = self.service.lifetime.resolve(&receipt) else {
            return MenuAudioStop::NotOwned;
        };
        let Some(index) = self
            .service
            .handles
            .iter()
            .position(|owned| *owned == handle)
        else {
            return MenuAudioStop::AlreadyEnded;
        };
        self.service.handles.remove(index);
        self.service
            .device
            .as_ref()
            .expect("bound menu audio service")
            .lock()
            .expect("receiving menu audio device poisoned")
            .remove_audio_event(handle);
        MenuAudioStop::Stopped
    }
    fn pump(&mut self) {
        let view = &self.view;
        self.service
            .device
            .as_ref()
            .expect("bound menu audio service")
            .lock()
            .expect("receiving menu audio device poisoned")
            .update_with_view(view.observation(), &mut |position| {
                view.is_on_screen(position)
            });
    }
    fn fade_out_music(&mut self) {
        let handles: Vec<_> = self.service.handles.iter().copied().collect();
        self.service
            .device
            .as_ref()
            .expect("bound menu audio service")
            .lock()
            .expect("receiving menu audio device poisoned")
            .fade_music_for_handles(&handles);
    }
}
