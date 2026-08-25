//! C++ OpenContain/TransportContain sound and containment state helpers.
use super::super::super::*;

impl GameLogic {
    fn contain_module_sound_event_name(container: &Object, enter: bool) -> Option<String> {
        let leftover = if enter {
            gamelogic::object::contain::leftover_contain_module_enter_sound(
                &container.template_name,
            )
        } else {
            gamelogic::object::contain::leftover_contain_module_exit_sound(&container.template_name)
        };
        if leftover
            .as_deref()
            .is_some_and(|name| !name.is_empty() && !name.eq_ignore_ascii_case("NONE"))
        {
            return leftover;
        }
        let authored = if enter {
            container.thing.template.contain_module.enter_sound.as_str()
        } else {
            container.thing.template.contain_module.exit_sound.as_str()
        };
        let authored = authored.trim();
        if !authored.is_empty() && !authored.eq_ignore_ascii_case("NONE") {
            return Some(authored.to_string());
        }
        // Module EnterSound/ExitSound only. Humvee comments those out so
        // doLoad/doUnload stay silent; C++ still plays Object SoundEnter /
        // SoundExit + rider SoundFallingFromPlane from onContaining/onRemoving.
        container.is_garrison_contain().then(|| {
            if enter {
                "GarrisonEnter".to_string()
            } else {
                "GarrisonExit".to_string()
            }
        })
    }

    /// C++ `OpenContain::doLoadSound` — leftover TheAudio, once per frame per container.
    pub(crate) fn play_container_enter_sound(&self, container_id: ObjectId) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        let name = Self::contain_module_sound_event_name(container, true);
        gamelogic::object::contain::leftover_play_container_enter_sound(
            name.as_deref(),
            container_id.0,
            self.frame,
        );
        // C++ OpenContain::onContaining template SoundEnter (load-sounds-enabled).
        self.play_container_containing_template_sounds(container_id);
    }

    /// C++ `OpenContain::doUnloadSound` — leftover TheAudio, once per frame per container.
    pub(crate) fn play_container_exit_sound(&self, container_id: ObjectId) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        let name = Self::contain_module_sound_event_name(container, false);
        gamelogic::object::contain::leftover_play_container_exit_sound(
            name.as_deref(),
            container_id.0,
            self.frame,
        );
    }

    /// C++ `OpenContain::onContaining` Object `SoundEnter` via leftover TheAudio.
    pub(crate) fn play_container_containing_template_sounds(&self, container_id: ObjectId) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        let _ =
            gamelogic::object::contain::open_contain::leftover_play_on_containing_template_sounds(
                &container.template_name,
                container_id.0,
                true,
                None,
            );
    }

    /// C++ `OpenContain::onRemoving` Object `SoundExit` + rider `SoundFallingFromPlane`.
    pub(crate) fn play_container_removing_template_sounds(
        &self,
        container_id: ObjectId,
        rider_id: ObjectId,
    ) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        let Some(rider) = self.objects.get(&rider_id) else {
            return;
        };
        self.play_container_removing_template_sounds_named(
            &container.template_name,
            container_id,
            &rider.template_name,
            rider_id,
        );
    }

    pub(crate) fn play_container_removing_template_sounds_named(
        &self,
        container_template: &str,
        container_id: ObjectId,
        rider_template: &str,
        rider_id: ObjectId,
    ) {
        let _ = gamelogic::object::contain::open_contain::leftover_play_on_removing_template_sounds(
            container_template,
            container_id.0,
            rider_template,
            rider_id.0,
            None,
            None,
        );
    }

    /// C++ TransportContain `ResetMoodCheckTimeOnExit` → `wakeUpAndAttemptToTarget`.
    pub(crate) fn reset_rider_mood_check_on_exit(&mut self, rider_id: ObjectId) {
        let now = self.frame;
        if let Some(unit) = self.objects.get_mut(&rider_id) {
            unit.next_mood_check_time = now;
        }
    }
}
