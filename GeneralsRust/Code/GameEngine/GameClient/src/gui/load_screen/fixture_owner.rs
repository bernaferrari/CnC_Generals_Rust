//! Owned fixture data around the SAME production Common menu-audio borrower.
//! The observer delegates all admission/handle/retirement decisions; it never
//! fabricates handles, filters sound, or implements a second playback policy.
use super::*;
use crate::core::game_client::{ClientInputAuthority, GameClient};
use crate::gui::menu_audio::{
    CommonMenuAudioService, MenuAudioEffects, MenuAudioHandle, MenuAudioStop, MenuAudioView,
};
use game_engine::common::audio::audio_event_rts::{
    AudioEventInfo, AudioType, Coord3D as AudioCoord, ST_PLAYER, ST_UI,
};
use game_engine::common::audio::game_audio::{
    AudioLocalityRelationship, AudioLocalityResolver, AudioViewObservation,
};
use game_engine::common::audio::{AudioManager, AudioSubmissionLocality};
use game_engine::common::random_value::RandomState;
use std::sync::{Arc, Mutex};
pub(super) type AudioTrace = Rc<RefCell<Vec<(String, MenuAudioHandle)>>>;
struct UiLocality;
impl AudioLocalityResolver for UiLocality {
    fn get_local_player_index(&self) -> Option<i32> {
        None
    }
    fn is_player_active(&self, _: i32) -> bool {
        false
    }
    fn player_exists(&self, _: i32) -> bool {
        false
    }
    fn has_default_team(&self, _: i32) -> bool {
        false
    }
    fn get_relationship_to_local_team(&self, _: i32, _: i32) -> AudioLocalityRelationship {
        AudioLocalityRelationship::Neutral
    }
}
#[derive(Clone, Copy)]
struct UiView;
impl MenuAudioView for UiView {
    fn observation(&self) -> AudioViewObservation {
        AudioViewObservation {
            ground_position: AudioCoord::ZERO,
            heading: 0.,
            camera_position: AudioCoord::ZERO,
            ground_height: 0.,
        }
    }
    fn is_on_screen(&self, _: &AudioCoord) -> bool {
        true
    }
}
struct Observe<'a> {
    inner: &'a mut dyn MenuAudioEffects,
    trace: AudioTrace,
}
impl MenuAudioEffects for Observe<'_> {
    fn is_bound(&self) -> bool {
        self.inner.is_bound()
    }
    fn play(&mut self, name: &str) -> Option<MenuAudioHandle> {
        let output = self.inner.play(name);
        if let Some(handle) = &output {
            self.trace
                .borrow_mut()
                .push((name.to_owned(), handle.clone()));
        }
        output
    }
    fn play_fading(&mut self, name: &str) -> Option<MenuAudioHandle> {
        let output = self.inner.play_fading(name);
        if let Some(handle) = &output {
            self.trace
                .borrow_mut()
                .push((name.to_owned(), handle.clone()));
        }
        output
    }
    fn stop(&mut self, handle: MenuAudioHandle) -> MenuAudioStop {
        self.inner.stop(handle)
    }
    fn pump(&mut self) {
        self.inner.pump();
    }
    fn fade_out_music(&mut self) {
        self.inner.fade_out_music();
    }
}
/// Explicit unavailable physical playback; Common admission still allocates its
/// real AR_Play receipt. These fixtures certify UI/admission ordering only.
struct NoPhysicalDevice;
impl game_engine::common::audio::game_audio::SoundPlaybackHook for NoPhysicalDevice {
    fn play(&self, _: &game_engine::common::audio::AudioEventRts) -> Result<(), String> {
        Err("fixture has no physical audio output".into())
    }
    fn stop(&self, _: u32) {}
    fn pause(&self, _: u32) {}
    fn resume(&self, _: u32) {}
    fn is_playing(&self, _: u32) -> bool {
        false
    }
}
pub(super) struct LoadScreenFixture {
    client: GameClient,
    audio: CommonMenuAudioService,
    random: RandomState,
    trace: AudioTrace,
}
impl LoadScreenFixture {
    pub(super) fn new() -> Self {
        let mut client =
            GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
        client.init_input_subsystems().unwrap();
        client.set_in_game_ui_subsystem(crate::core::subsystems::InGameUISubsystem::default());
        // Authored fixture definitions, installed BEFORE requests. Unknown names
        // still fail the production service; no new event is invented at play.
        let mut device = AudioManager::with_playback_hook(Box::new(NoPhysicalDevice));
        for name in [
            "LoadScreenAmbient",
            "PlayerNameSound",
            "OpponentNameSound",
            "Taunts_GCAnnouncer12",
            "Taunts_GCAnnouncer01",
            "PlayerTaunt1",
            "PlayerTaunt2",
            "PlayerTaunt3",
            "OpponentTaunt1",
            "OpponentTaunt2",
            "OpponentTaunt3",
            "SparseOpponentTaunt2",
            "Load_USA",
            "BriefingVoice",
        ] {
            device.register_audio_event_info(AudioEventInfo {
                audio_name: name.into(),
                sounds: vec!["OwnedLoadScreenClip".into()],
                sound_type: if name == "Load_USA" {
                    AudioType::Music
                } else {
                    AudioType::SoundEffect
                },
                type_field: ST_PLAYER | ST_UI,
                volume: 1.,
                low_pass_freq: 0.,
                ..Default::default()
            });
        }
        let mut audio = CommonMenuAudioService::default();
        audio.bind(Arc::new(Mutex::new(device)));
        Self {
            client,
            audio,
            random: RandomState::default(),
            trace: Rc::new(RefCell::new(Vec::new())),
        }
    }
    pub(super) fn client_mut(&mut self) -> &mut GameClient {
        &mut self.client
    }
    pub(super) fn with_runtime<R>(
        &mut self,
        operation: impl FnOnce(
            &mut LoadScreenOperation<'_>,
            &mut LoadScreenPresentationPump<'_>,
            AudioTrace,
        ) -> R,
    ) -> R {
        let locality = AudioSubmissionLocality::from_resolver(-1, &UiLocality);
        let policy = game_engine::common::audio::script_policy::ScriptAudioPolicy::default();
        let mut audio = self.audio.lend(&mut self.random, &policy, locality, UiView);
        let mut observed = Observe {
            inner: &mut audio,
            trace: Rc::clone(&self.trace),
        };
        let mut runtime = self.client.load_screen_runtime_mut(&mut observed);
        let mut present = |_: LoadScreenDrawServices<'_>| {
            pump_load_screen_presentation_test_hook();
            Ok(())
        };
        let mut pump = LoadScreenPresentationPump::for_native(&mut present);
        operation(&mut runtime, &mut pump, Rc::clone(&self.trace))
    }
}
pub(super) fn accepted_handle(trace: &AudioTrace, name: &str) -> Option<MenuAudioHandle> {
    Some(
        trace
            .borrow()
            .iter()
            .rev()
            .find(|(accepted, _)| accepted == name)
            .unwrap_or_else(|| {
                panic!("actual production audio service did not accept authored {name}")
            })
            .1
            .clone(),
    )
}
