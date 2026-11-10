//! Control Bar Commands - C++ ControlBarCommandProcessing.cpp unit sound.
//! Command processing lives on the owning `ControlBar` (impl_execute.rs).

use super::CommandButton;

use gamelogic::player::{PLAYER_INDEX_INVALID, PlayerIndex, player_list as logic_player_list};

fn local_player_index() -> Option<PlayerIndex> {
    let local = logic_player_list()
        .read()
        .ok()
        .map(|list| list.get_local_player_index())?;
    if local == PLAYER_INDEX_INVALID {
        None
    } else {
        Some(local)
    }
}

/// C++ ControlBarCommandProcessing.cpp:183-189.
/// `AudioEventRTS sound = *commandButton->getUnitSpecificSound();`
/// `sound.setPlayerIndex(localPlayer); TheAudio->addAudioEvent(&sound);`
pub(crate) fn play_command_button_unit_specific_sound(button: &CommandButton) {
    let Some(player_index) = local_player_index() else {
        return;
    };
    let Some(audio) = gamelogic::helpers::TheAudio::get() else {
        return;
    };
    let name = button.unit_specific_sound.playable_event_name();
    if name.is_empty() {
        return;
    }
    let mut sound = gamelogic::common::audio::AudioEventRts::new(name);
    sound.set_player_index(player_index as u32);
    let _ = audio.add_audio_event(&sound);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_specific_sound_plays_named_event_for_local_player() {
        crate::test_support::ensure_ambient_host();
        // C++ processCommandUI plays getUnitSpecificSound with local player index.
        if let Ok(mut list) = logic_player_list().write() {
            list.set_local_player_index(0);
        }
        let mut button = CommandButton::default();
        button.command_name = "Command_BlackLotusHackBuilding".into();
        button.unit_specific_sound =
            game_engine::ini::AudioEventRTS::from_event_name("BlackLotusVoiceModeBuilding".into());
        play_command_button_unit_specific_sound(&button);
        assert_eq!(
            button.unit_specific_sound.playable_event_name(),
            "BlackLotusVoiceModeBuilding"
        );
        if let Ok(mut list) = logic_player_list().write() {
            list.set_local_player_index(PLAYER_INDEX_INVALID);
        }
    }

    #[test]
    fn empty_unit_specific_sound_is_a_no_op() {
        crate::test_support::ensure_ambient_host();
        if let Ok(mut list) = logic_player_list().write() {
            list.set_local_player_index(0);
        }
        let button = CommandButton::default();
        play_command_button_unit_specific_sound(&button);
        assert!(button.unit_specific_sound.playable_event_name().is_empty());
        if let Ok(mut list) = logic_player_list().write() {
            list.set_local_player_index(PLAYER_INDEX_INVALID);
        }
    }
}
