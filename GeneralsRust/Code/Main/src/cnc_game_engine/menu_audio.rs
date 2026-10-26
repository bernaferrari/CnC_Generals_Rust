//! Immediate menu audio borrows one receiving service and its existing stream.
use game_client::gui::menu_audio::{
    MenuAudioEffects, MenuAudioHandle, MenuAudioLifetime, MenuAudioStop,
};
use game_engine::common::audio::audio_event_rts::AudioEventSelection;
use game_engine::common::audio::{AudioEventRts, AudioManager, AudioSubmissionLocality};
use game_engine::common::random_value::RandomState;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct NativeMenuAudioService {
    device: Option<Arc<Mutex<AudioManager>>>,
    lifetime: MenuAudioLifetime,
    handles: Vec<u32>,
}
impl NativeMenuAudioService {
    pub(super) fn bind(&mut self, device: Arc<Mutex<AudioManager>>) {
        assert!(
            self.device.is_none(),
            "menu audio binds only at explicit native init"
        );
        self.device = Some(device);
    }
    pub(super) fn require_bound(&self) -> Result<(), String> {
        self.device
            .as_ref()
            .map(|_| ())
            .ok_or_else(|| "native menu audio service is not initialized".into())
    }
    pub(super) fn lend<'a>(
        &'a mut self,
        random: &'a mut RandomState,
        locality: AudioSubmissionLocality,
        view: super::audio_view::CompletedAudioView,
    ) -> NativeMenuAudio<'a> {
        NativeMenuAudio {
            service: self,
            random,
            locality,
            view,
        }
    }
    pub(super) fn reset(&mut self) {
        if let Some(device) = &self.device {
            let mut device = device.lock().expect("native menu audio device poisoned");
            device.retire_audio_handles(&self.handles);
            self.handles.clear();
        }
        self.lifetime = MenuAudioLifetime::default();
    }
}
impl Drop for NativeMenuAudioService {
    fn drop(&mut self) {
        self.reset();
    }
}

pub(super) struct NativeMenuAudio<'a> {
    service: &'a mut NativeMenuAudioService,
    pub(super) random: &'a mut RandomState,
    locality: AudioSubmissionLocality,
    view: super::audio_view::CompletedAudioView,
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
impl NativeMenuAudio<'_> {
    fn play_event(&mut self, name: &str, fade: bool) -> Option<MenuAudioHandle> {
        self.service
            .require_bound()
            .expect("native menu admission must check service binding");
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
            .expect("native menu audio device poisoned")
            .add_audio_event_with_locality_and_selection(
                &event,
                &self.locality,
                &mut MenuSelection(self.random),
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
impl MenuAudioEffects for NativeMenuAudio<'_> {
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
            .expect("native menu audio device poisoned")
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
            .expect("native menu audio device poisoned")
            .update_with_view(view.listener, &mut |position| view.is_on_screen(position));
    }
    fn fade_out_music(&mut self) {
        let handles: Vec<_> = self.service.handles.iter().copied().collect();
        self.service
            .device
            .as_ref()
            .expect("bound menu audio service")
            .lock()
            .expect("native menu audio device poisoned")
            .fade_music_for_handles(&handles);
    }
}
