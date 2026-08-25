//! Object::onCapture / defect / initial-capture-bonus C++ parity hooks.
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// C++ `newOwner->getScoreKeeper()->addObjectCaptured(this)`.
    pub(super) fn on_capture_award_score(&self, new_owner: &Option<Arc<RwLock<Player>>>) {
        if let Some(new_owner_arc) = new_owner {
            if let Ok(mut owner_guard) = new_owner_arc.write() {
                owner_guard
                    .get_score_keeper_mut()
                    .add_object_captured_obj(self);
            }
        }
    }

    /// C++ skirmish AI that captures a faction structure sells it immediately.
    pub(super) fn on_capture_sell_ai_faction_building(
        &self,
        owners_differ: bool,
        new_owner: &Option<Arc<RwLock<Player>>>,
    ) {
        if !owners_differ {
            return;
        }
        let Some(new_owner_arc) = new_owner else {
            return;
        };
        let is_skirmish = new_owner_arc
            .read()
            .ok()
            .map(|g| g.is_skirmish_ai_player())
            .unwrap_or(false);
        if !is_skirmish || !self.is_faction_structure() {
            return;
        }
        if let Some(mut assistant) =
            game_engine::common::system::build_assistant::get_build_assistant()
        {
            let object = game_engine::common::system::build_assistant::Object {
                id: self.get_id(),
                position: game_engine::common::system::build_assistant::Coord3D {
                    x: self.get_position().x,
                    y: self.get_position().y,
                    z: self.get_position().z,
                },
                orientation: self.get_orientation(),
                command_set: None,
            };
            assistant.sell_object(&object, crate::helpers::TheGameLogic::get_frame());
        }
    }

    /// C++ `Player::becomingTeamMember` AutoDeposit `awardInitialCaptureBonus`.
    pub(super) fn award_initial_capture_bonus_if_needed(
        &self,
        new_owner: Option<Arc<RwLock<Player>>>,
    ) {
        let Some(player_arc) = new_owner else {
            return;
        };
        let is_neutral = player_arc
            .read()
            .ok()
            .map(|g| g.get_player_type() == PlayerType::Neutral)
            .unwrap_or(true);
        if is_neutral {
            return;
        }
        if let Some(handle) = self.find_update_module("AutoDepositUpdate") {
            handle.with_module_downcast::<
                crate::object::behavior::auto_deposit_update::AutoDepositUpdateModule,
                _,
                _,
            >(|module| {
                module
                    .behavior_mut()
                    .award_initial_capture_bonus(Some(player_arc.clone()));
            });
        }
    }

    /// C++ `getVoiceDefect()` + `m_defectorTimerTickSound` after the team switch.
    pub(super) fn defect_play_voice_and_timer(&self) {
        let mut voice = self.get_template().get_voice_defect();
        voice.set_object_id(self.id);
        if let Some(audio) = crate::helpers::TheAudio::get() {
            audio.add_audio_event(&voice);
        }
        if self.drawable.is_none() {
            return;
        }
        if let Some(audio) = crate::helpers::TheAudio::get() {
            if let Some(misc_audio) = game_engine::common::ini::ini_misc_audio::get_misc_audio() {
                let misc_audio = misc_audio.read();
                let sound_name = misc_audio
                    .defector_timer_tick_sound
                    .playable_event_name()
                    .to_string();
                let mut event =
                    crate::object::special_power_template::AudioEventRts::new(sound_name);
                event.set_object_id(self.id);
                audio.add_audio_event(&event);
            }
        }
    }

    /// C++ world walk: every `KINDOF_MINE` whose producer is this object `setTeam`s.
    pub(super) fn defect_owned_mines(&mut self, new_team: &Arc<RwLock<Team>>) {
        let mut ids: Vec<ObjectID> = OBJECT_REGISTRY.get_all_object_ids();
        if let Ok(logic) = crate::system::game_logic::get_game_logic().lock() {
            for id in logic.get_all_object_ids() {
                if !ids.contains(id) {
                    ids.push(*id);
                }
            }
        }
        for obj_id in ids {
            if obj_id == self.id {
                continue;
            }
            let Some(mine) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                .or_else(|| OBJECT_REGISTRY.get_object(obj_id))
            else {
                continue;
            };
            let Ok(mut mine_guard) = mine.write() else {
                continue;
            };
            if !mine_guard.is_kind_of(KindOf::Mine) {
                continue;
            }
            if mine_guard.get_producer_id() != self.id {
                continue;
            }
            let _ = mine_guard.set_team(Some(new_team.clone()));
        }
    }
}
