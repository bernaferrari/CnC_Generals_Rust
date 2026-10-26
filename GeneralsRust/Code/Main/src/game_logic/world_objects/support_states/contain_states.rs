//! C++ OpenContain sound admissions borrow this operation's real audio recipient.
use super::super::super::*;
use game_engine::common::audio::{
    AudioEventRts, AudioLocalityRelationship, AudioLocalityResolver, AudioSubmissionLocality,
};
use game_engine::common::thing::thing_template::ThingTemplateAudioType;

/// Only the two players queried by shouldPlayLocally; no borrowed World is
/// hidden behind the service's Send + Sync resolver trait.
struct ContainAudioPlayers {
    source: i32,
    source_exists: bool,
    source_active: bool,
    local: Option<i32>,
    local_active: bool,
    relationship: AudioLocalityRelationship,
}
impl AudioLocalityResolver for ContainAudioPlayers {
    fn get_local_player_index(&self) -> Option<i32> {
        self.local
    }
    fn is_player_active(&self, id: i32) -> bool {
        if Some(id) == self.local {
            self.local_active
        } else {
            id == self.source && self.source_active
        }
    }
    fn player_exists(&self, id: i32) -> bool {
        self.local == Some(id) || (id == self.source && self.source_exists)
    }
    fn has_default_team(&self, id: i32) -> bool {
        self.player_exists(id)
    }
    fn get_relationship_to_local_team(&self, source: i32, local: i32) -> AudioLocalityRelationship {
        if source == local {
            AudioLocalityRelationship::Allies
        } else {
            self.relationship
        }
    }
}

impl GameLogic {
    pub(in crate::game_logic) fn admit_containment_audio(
        &self,
        object_id: ObjectId,
        mut event: AudioEventRts,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(audio) = effects.audio.as_mut() else {
            return;
        };
        let Some(object) = self.objects.get(&object_id) else {
            return;
        };
        event.set_object_id(object_id.0);
        let source = object.owner_player_id.map(|id| id as i32).unwrap_or(-1);
        let locality = self.receiving_audio_locality_for_player(source);
        event.admit_script_policy(self.queued_audio_events.policy());
        audio.add_audio_event(&event, &locality);
    }

    pub(crate) fn receiving_audio_locality_for_player(
        &self,
        source: i32,
    ) -> AudioSubmissionLocality {
        let local = self.local_player_id();
        let players = ContainAudioPlayers {
            source,
            source_exists: source >= 0 && self.players.contains_key(&(source as u32)),
            source_active: self
                .players
                .get(&(source as u32))
                .is_some_and(|p| p.is_alive),
            local: local.map(|id| id as i32),
            local_active: local
                .and_then(|id| self.players.get(&id))
                .is_some_and(|p| p.is_alive),
            relationship: local
                .map(
                    |local| match self.player_relationship(source as u32, local) {
                        gamelogic::common::Relationship::Allies => {
                            AudioLocalityRelationship::Allies
                        }
                        gamelogic::common::Relationship::Enemies => {
                            AudioLocalityRelationship::Enemies
                        }
                        _ => AudioLocalityRelationship::Neutral,
                    },
                )
                .unwrap_or(AudioLocalityRelationship::Neutral),
        };
        AudioSubmissionLocality::from_resolver(source, &players)
    }

    fn contain_template_audio(
        &self,
        object_id: ObjectId,
        slot: ThingTemplateAudioType,
    ) -> Option<AudioEventRts> {
        self.objects
            .get(&object_id)?
            .thing()
            .template
            .receiving_client_template
            .as_ref()?
            .audio_event(slot)
            .cloned()
    }

    /// OpenContain::onContaining before doLoadSound. Initial payload producers
    /// that disable loading sounds require an explicit silent admission route;
    /// ordinary native entry preserves the existing enabled operation.
    pub(crate) fn play_container_enter_sound(
        &mut self,
        container_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.play_container_on_containing_sound(container_id, effects);
        self.play_container_load_sound(container_id, effects);
    }

    pub(crate) fn play_container_on_containing_sound(
        &self,
        container_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if let Some(event) =
            self.contain_template_audio(container_id, ThingTemplateAudioType::SoundEnter)
        {
            self.admit_containment_audio(container_id, event, effects);
        }
    }

    pub(crate) fn play_container_load_sound(
        &mut self,
        container_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let event = self
            .objects
            .get(&container_id)
            .and_then(|container| {
                container
                    .thing()
                    .template
                    .receiving_client_template
                    .as_ref()
            })
            .and_then(|template| template.containment_module_audio(true));
        let Some(container) = self.objects.get_mut(&container_id) else {
            return;
        };
        if container.last_load_sound_frame == self.frame {
            return;
        }
        // CPP stamps every eligible frame, including absent/NoSound module cues.
        container.last_load_sound_frame = self.frame;
        if let Some(event) = event {
            self.admit_containment_audio(container_id, event, effects);
        }
    }

    /// OpenContain::doUnloadSound before onRemoving; exactly once per frame.
    fn play_container_exit_sound(
        &mut self,
        container_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let event = self
            .objects
            .get(&container_id)
            .and_then(|container| {
                container
                    .thing()
                    .template
                    .receiving_client_template
                    .as_ref()
            })
            .and_then(|template| template.containment_module_audio(false));
        let Some(container) = self.objects.get_mut(&container_id) else {
            return;
        };
        if container.last_unload_sound_frame == self.frame {
            return;
        }
        container.last_unload_sound_frame = self.frame;
        if let Some(event) = event {
            self.admit_containment_audio(container_id, event, effects);
        }
    }

    /// CPP OpenContain::onRemoving always admits the container exit, followed
    /// by the rider falling cue when that object still exists. Roster removal
    /// does not remove either object's immutable receiving template.
    pub(crate) fn play_container_removing_template_sounds(
        &mut self,
        container_id: ObjectId,
        rider_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.play_container_exit_sound(container_id, effects);
        if let Some(event) =
            self.contain_template_audio(container_id, ThingTemplateAudioType::SoundExit)
        {
            self.admit_containment_audio(container_id, event, effects);
        }
        if let Some(event) =
            self.contain_template_audio(rider_id, ThingTemplateAudioType::SoundFalling)
        {
            self.admit_containment_audio(rider_id, event, effects);
        }
    }

    /// C++ TransportContain `ResetMoodCheckTimeOnExit` → `wakeUpAndAttemptToTarget`.
    pub(crate) fn reset_rider_mood_check_on_exit(
        &mut self,
        container_id: ObjectId,
        rider_id: ObjectId,
    ) {
        let reset = self.objects.get(&container_id).is_some_and(|container| {
            let data = &container.thing().template.contain_module;
            data.reset_mood_check_time_on_exit
                && matches!(
                    data.kind,
                    ContainModuleKind::Transport
                        | ContainModuleKind::RiderChange
                        | ContainModuleKind::RailedTransport
                        | ContainModuleKind::InternetHack
                )
        });
        if !reset {
            return;
        }
        let now = self.frame;
        if let Some(unit) = self.objects.get_mut(&rider_id) {
            // AIUpdate.cpp:4428–4438: active passengers keep both fields.
            if unit.ai_state() == AIState::Idle {
                unit.next_mood_check_time = now;
                unit.randomly_offset_mood_check = true;
            }
        }
    }
}
