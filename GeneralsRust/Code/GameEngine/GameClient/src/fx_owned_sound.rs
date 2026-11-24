//! SoundFXNugget admission with actual receiving audio definitions and locality.
use super::Coord3D;
use game_engine::common::audio::AudioInstance;
use game_engine::common::audio::audio_event_rts::{
    AudioEventRts, AudioEventSelection, Coord3D as AudioCoord3D,
};
use game_engine::common::audio::game_audio::{
    AHSV_NO_SOUND, AudioManager, AudioSubmissionLocality,
};

pub(super) fn sound_fx_event(
    name: &str,
    position: Option<&Coord3D>,
    player: Option<i32>,
) -> AudioEventRts {
    let mut event = AudioEventRts::with_event_name(name);
    if let Some(position) = position {
        event.set_position(&AudioCoord3D {
            x: position.x,
            y: position.y,
            z: position.z,
        });
    }
    if let Some(player) = player {
        event.set_player_index(player);
    }
    event
}

/// Valid definitions follow SoundFXNugget's original position/player/event path.
/// Missing content is reported before any audio handle/request is admitted.
pub fn submit_sound_fx_owned(
    name: &str,
    position: Option<&Coord3D>,
    player: Option<i32>,
    manager: &mut AudioManager,
    instance: &AudioInstance,
    locality: &AudioSubmissionLocality,
    selection: &mut dyn AudioEventSelection,
) -> Result<u32, String> {
    if name.is_empty() || name == "NoSound" {
        return Ok(AHSV_NO_SOUND);
    }
    let info = manager.find_audio_event_info(name).ok_or_else(|| {
        format!("SoundFXNugget definition '{name}' is unavailable in receiving audio manager")
    })?;
    let mut event = sound_fx_event(name, position, player);
    event.set_volume(info.volume);
    event.set_audio_event_info(info);
    event.admit_instance(instance);
    Ok(manager.add_audio_event_with_locality_and_selection(&event, locality, selection))
}
pub fn submit_sound_fx_with_policy(
    name: &str,
    position: Option<&Coord3D>,
    player: Option<i32>,
    manager: &mut AudioManager,
    instance: Option<&AudioInstance>,
    locality: &AudioSubmissionLocality,
    selection: &mut dyn AudioEventSelection,
    policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
) -> Result<u32, String> {
    if name.is_empty() || name == "NoSound" {
        return Ok(AHSV_NO_SOUND);
    }
    let info = manager.find_audio_event_info(name).ok_or_else(|| {
        format!("SoundFXNugget definition '{name}' is unavailable in receiving audio manager")
    })?;
    let mut event = sound_fx_event(name, position, player);
    event.set_volume(info.volume);
    event.set_audio_event_info(info);
    if let Some(instance) = instance {
        event.admit_instance(instance);
    }
    Ok(manager.add_audio_event_with_policy(&event, locality, selection, policy, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::audio::audio_event_rts::{
        AudioEventInfo, AudioSelectionStreams, ST_PLAYER,
    };
    use game_engine::common::audio::game_audio::{
        AudioLocalityRelationship, AudioLocalityResolver,
    };
    use game_engine::common::random_value::RandomState;

    struct Players(i32);
    impl AudioLocalityResolver for Players {
        fn get_local_player_index(&self) -> Option<i32> {
            Some(self.0)
        }
        fn is_player_active(&self, _: i32) -> bool {
            true
        }
        fn player_exists(&self, _: i32) -> bool {
            true
        }
        fn has_default_team(&self, _: i32) -> bool {
            true
        }
        fn get_relationship_to_local_team(
            &self,
            source: i32,
            local: i32,
        ) -> AudioLocalityRelationship {
            if source == local {
                AudioLocalityRelationship::Allies
            } else {
                AudioLocalityRelationship::Enemies
            }
        }
    }
    fn manager() -> AudioManager {
        let mut manager = AudioManager::new();
        manager.init();
        manager.register_audio_event_info(AudioEventInfo {
            audio_name: "OwnedSound".into(),
            sounds: vec!["owned_sound".into()],
            type_field: ST_PLAYER,
            limit: 1,
            ..Default::default()
        });
        manager
    }

    #[test]
    fn sound_fx_queues_only_in_the_receiving_players_audio_manager() {
        crate::test_support::ensure_ambient_host();
        let mut first = manager();
        let mut peer = manager();
        let mut logic = RandomState::default();
        let mut audio = RandomState::default();
        let mut selection = AudioSelectionStreams {
            logic: &mut logic,
            audio: &mut audio,
        };
        let pos = Coord3D::new(1.0, 2.0, 3.0);
        let local = AudioSubmissionLocality::from_resolver(3, &Players(3));
        let remote = AudioSubmissionLocality::from_resolver(3, &Players(4));
        let first_instance = AudioInstance::default();
        let peer_instance = AudioInstance::default();
        submit_sound_fx_owned(
            "OwnedSound",
            Some(&pos),
            Some(3),
            &mut first,
            &first_instance,
            &local,
            &mut selection,
        )
        .unwrap();
        submit_sound_fx_owned(
            "OwnedSound",
            Some(&pos),
            Some(3),
            &mut peer,
            &peer_instance,
            &remote,
            &mut selection,
        )
        .unwrap();
        assert_eq!(first.pending_play_request_count_for("OwnedSound"), 1);
        assert_eq!(peer.pending_play_request_count_for("OwnedSound"), 0);
        peer.remove_all_audio_requests();
        assert_eq!(first.pending_play_request_count_for("OwnedSound"), 1);
    }

    #[test]
    fn unavailable_sound_is_an_error_before_request_admission() {
        crate::test_support::ensure_ambient_host();
        let mut manager = manager();
        let mut logic = RandomState::default();
        let mut audio = RandomState::default();
        let mut selection = AudioSelectionStreams {
            logic: &mut logic,
            audio: &mut audio,
        };
        let local = AudioSubmissionLocality::from_resolver(3, &Players(3));
        let instance = AudioInstance::default();
        assert!(
            submit_sound_fx_owned(
                "MissingSound",
                None,
                Some(3),
                &mut manager,
                &instance,
                &local,
                &mut selection
            )
            .unwrap_err()
            .contains("MissingSound")
        );
        assert_eq!(manager.pending_play_request_count(), 0);
    }

    #[test]
    fn policy_fx_limits_the_cue_per_receiving_instance_on_one_mixer() {
        crate::test_support::ensure_ambient_host();
        let mut mixer = manager();
        let first = AudioInstance::default();
        let peer = AudioInstance::default();
        let mut logic = RandomState::default();
        let mut audio = RandomState::default();
        let mut selection = AudioSelectionStreams {
            logic: &mut logic,
            audio: &mut audio,
        };
        let locality = AudioSubmissionLocality::from_resolver(3, &Players(3));
        let policy = game_engine::common::audio::script_policy::ScriptAudioPolicy::default();
        let mut submit = |instance: &AudioInstance| {
            submit_sound_fx_with_policy(
                "OwnedSound",
                None,
                Some(3),
                &mut mixer,
                Some(instance),
                &locality,
                &mut selection,
                &policy,
            )
            .unwrap()
        };
        assert_ne!(submit(&first), AHSV_NO_SOUND);
        assert_eq!(submit(&first), AHSV_NO_SOUND);
        assert_ne!(submit(&peer), AHSV_NO_SOUND);
        assert_eq!(mixer.pending_play_request_count_for("OwnedSound"), 2);
    }
}
