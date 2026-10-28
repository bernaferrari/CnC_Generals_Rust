//! One borrowed promotion presentation context. Audio uses the same ObjectAudio
//! loan as movement; no second audio queue or object-owned receipt is introduced.
use super::host_unit_training::{
    hide_promote_fx_for_stealth, should_queue_promote_fx, veterancy_rank,
};
use super::{Object, ObjectAudio, VeterancyLevel};
use crate::presentation_frame::WorldAnimations;
use game_engine::common::ini::ini_locomotor::LocomotorStore;

pub struct VeterancyFeedback<'a> {
    frame: u32,
    local_player: Option<u32>,
    draw_icon_ui: bool,
    definitions: Option<&'a super::promotion_definitions::PromotionDefinitions>,
    animations: Option<&'a mut WorldAnimations>,
}
impl<'a> VeterancyFeedback<'a> {
    pub(crate) fn logic_frame(&self) -> u32 {
        self.frame
    }

    /// Detached component operations have no UI owner or MiscAudio definitions.
    /// Their authored Body sound still reaches the explicitly supplied audio sink.
    pub fn standalone() -> Self {
        Self {
            frame: 0,
            local_player: None,
            draw_icon_ui: true,
            definitions: None,
            animations: None,
        }
    }
    pub(crate) fn new(
        frame: u32,
        local_player: Option<u32>,
        draw_icon_ui: bool,
        definitions: &'a super::promotion_definitions::PromotionDefinitions,
        animations: &'a mut WorldAnimations,
    ) -> Self {
        Self {
            frame,
            local_player,
            draw_icon_ui,
            definitions: Some(definitions),
            animations: Some(animations),
        }
    }
    pub(crate) fn body_sound(
        &self,
        object: &mut Object,
        old: VeterancyLevel,
        new: VeterancyLevel,
        audio: &mut ObjectAudio<'_>,
    ) {
        if object.is_inactive_body() || veterancy_rank(new) <= veterancy_rank(old) {
            return;
        }
        let index = veterancy_rank(new) as usize - 1;
        let Some(name) = object
            .thing()
            .template
            .sound_promoted
            .get(index)
            .and_then(Clone::clone)
        else {
            return;
        };
        // ActiveBody attaches the cue to ObjectID, not the health-box position.
        let request = super::AudioEventRequest::new(&name)
            .with_object(object.id)
            .with_priority(160);
        audio.submit(object, request);
    }
    pub(crate) fn object_effects(
        &mut self,
        object: &mut Object,
        old: VeterancyLevel,
        new: VeterancyLevel,
        audio: &mut ObjectAudio<'_>,
    ) {
        let local = object.owner_player_id.is_some() && object.owner_player_id == self.local_player;
        let hidden = hide_promote_fx_for_stealth(
            local,
            object.status.stealthed,
            object.status.detected,
            object.status.disguised,
        );
        if !should_queue_promote_fx(
            old,
            new,
            object.is_kind_of(super::KindOf::IgnoredInGui),
            hidden,
            self.draw_icon_ui,
            true,
        ) {
            return;
        }
        let Some(definitions) = self.definitions else {
            return;
        };
        if let (Some(template), Some(animations)) = (
            definitions.level_gain_template(),
            self.animations.as_deref_mut(),
        ) {
            animations.admit(
                crate::presentation_frame::PresentationWorldAnim {
                    identity: 0,
                    initial_frame: 0,
                    template: definitions.rules.animation_name.clone(),
                    position: object.get_health_box_position(),
                    display_time_seconds: definitions.rules.display_seconds,
                    z_rise_per_second: definitions.rules.rise_per_second,
                    fades: true,
                    spawn_frame: self.frame,
                    crate_id: object.id,
                    picker_id: object.id,
                },
                template,
            );
        }
        // Object.cpp: missing animation does not suppress MiscAudio.
        if let Some(name) = definitions.unit_promoted_name() {
            let request = super::AudioEventRequest::new(name)
                .with_object(object.id)
                .with_priority(160);
            audio.submit(object, request);
        }
    }
}

impl super::GameLogic {
    #[cfg(test)]
    pub(crate) fn with_veterancy_feedback<R>(
        &mut self,
        object: &mut Object,
        action: impl FnOnce(&mut Object, &mut ObjectAudio<'_>, &mut VeterancyFeedback<'_>) -> R,
    ) -> R {
        self.with_veterancy_feedback_and_locomotors(object, |object, audio, feedback, _| {
            action(object, audio, feedback)
        })
    }
    pub(crate) fn with_veterancy_feedback_and_locomotors<R>(
        &mut self,
        object: &mut Object,
        action: impl FnOnce(
            &mut Object,
            &mut ObjectAudio<'_>,
            &mut VeterancyFeedback<'_>,
            Option<&LocomotorStore>,
        ) -> R,
    ) -> R {
        let local = self.local_player_id();
        let draw_icon_ui = self.get_draw_icon_ui();
        let mut feedback = VeterancyFeedback::new(
            self.frame,
            local,
            draw_icon_ui,
            &self.promotion_definitions,
            &mut self.session_view.world_animations,
        );
        let mut audio = ObjectAudio::queue(&mut self.queued_audio_events);
        action(
            object,
            &mut audio,
            &mut feedback,
            self.locomotor_definitions.as_ref(),
        )
    }
    #[cfg(test)]
    pub(crate) fn with_object_veterancy_feedback<R>(
        &mut self,
        id: super::ObjectId,
        action: impl FnOnce(&mut Object, &mut ObjectAudio<'_>, &mut VeterancyFeedback<'_>) -> R,
    ) -> Option<R> {
        self.with_object_veterancy_feedback_and_partition(
            id,
            |object, audio, feedback, _partition| action(object, audio, feedback),
        )
    }
    #[cfg(test)]
    pub(crate) fn with_object_veterancy_feedback_and_partition<R>(
        &mut self,
        id: super::ObjectId,
        action: impl FnOnce(
            &mut Object,
            &mut ObjectAudio<'_>,
            &mut VeterancyFeedback<'_>,
            &mut super::partition_manager::PartitionManager,
        ) -> R,
    ) -> Option<R> {
        self.with_object_veterancy_inputs(id, |object, audio, feedback, partition, _| {
            action(object, audio, feedback, partition)
        })
    }
    pub(crate) fn with_object_veterancy_feedback_and_locomotors<R>(
        &mut self,
        id: super::ObjectId,
        action: impl FnOnce(
            &mut Object,
            &mut ObjectAudio<'_>,
            &mut VeterancyFeedback<'_>,
            Option<&LocomotorStore>,
        ) -> R,
    ) -> Option<R> {
        self.with_object_veterancy_inputs(id, |object, audio, feedback, _, locomotors| {
            action(object, audio, feedback, locomotors)
        })
    }
    pub(crate) fn with_object_veterancy_inputs<R>(
        &mut self,
        id: super::ObjectId,
        action: impl FnOnce(
            &mut Object,
            &mut ObjectAudio<'_>,
            &mut VeterancyFeedback<'_>,
            &mut super::partition_manager::PartitionManager,
            Option<&LocomotorStore>,
        ) -> R,
    ) -> Option<R> {
        let local = self.local_player_id();
        let draw_icon_ui = self.get_draw_icon_ui();
        let mut feedback = VeterancyFeedback::new(
            self.frame,
            local,
            draw_icon_ui,
            &self.promotion_definitions,
            &mut self.session_view.world_animations,
        );
        let mut audio = ObjectAudio::queue(&mut self.queued_audio_events);
        Some(action(
            self.objects.get_mut(&id)?,
            &mut audio,
            &mut feedback,
            &mut self.partition_manager,
            self.locomotor_definitions.as_ref(),
        ))
    }

    /// C++ ExperienceTracker::setVeterancyLevel through Object::onVeterancyLevelChanged.
    /// Commit the new XP/rank first, apply the rank upgrade through the receiving
    /// world's synchronous upgrade callbacks, then resume Body feedback only if
    /// the same Object birth remains installed.
    pub(crate) fn set_exact_veterancy_level_with_upgrade_callback(
        &mut self,
        id: super::ObjectId,
        level: super::VeterancyLevel,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.set_exact_veterancy_level_with_upgrade_callback_mode(id, level, true, effects)
    }

    /// Finish the synchronous `onVeterancyLevelChanged` phase for an XP gain.
    /// The caller has already committed its earned XP and rank, matching
    /// ExperienceTracker::addExperiencePoints before Object.cpp:3064.
    pub(crate) fn finish_experience_rank_change_with_upgrade_callback(
        &mut self,
        id: super::ObjectId,
        previous: super::VeterancyLevel,
        level: super::VeterancyLevel,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(birth) = self
            .objects
            .get(&id)
            .map(|object| std::sync::Arc::downgrade(&object.visual_draw_identity_token))
        else {
            return Ok(false);
        };

        if let Some(upgrade) = crate::game_logic::host_unit_training::veterancy_upgrade_name(level)
        {
            self.apply_upgrade_to_object_with_interpreter_checked(
                id,
                upgrade,
                interpreter,
                effects,
            )?;
        }

        let same_birth = self.objects.get(&id).is_some_and(|object| {
            birth.ptr_eq(&std::sync::Arc::downgrade(
                &object.visual_draw_identity_token,
            ))
        });
        if !same_birth {
            return Ok(false);
        }

        let health_bonus = self.veterancy_health_bonus;
        self.with_object_veterancy_feedback_and_locomotors(
            id,
            |object, audio, feedback, _locomotors| {
                object.apply_veterancy_bonuses_after_rank_upgrade_in(
                    previous,
                    level,
                    &health_bonus,
                    audio,
                    feedback,
                );
            },
        );
        Ok(true)
    }

    /// C++ ExperienceTracker setter used by hijack collision (`provideFeedback = FALSE`).
    pub(crate) fn set_exact_veterancy_level_with_upgrade_callback_quiet(
        &mut self,
        id: super::ObjectId,
        level: super::VeterancyLevel,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.set_exact_veterancy_level_with_upgrade_callback_mode(id, level, false, effects)
    }

    fn set_exact_veterancy_level_with_upgrade_callback_mode(
        &mut self,
        id: super::ObjectId,
        level: super::VeterancyLevel,
        provide_feedback: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(false);
        };
        let birth = std::sync::Arc::downgrade(&object.visual_draw_identity_token);
        let Some(previous) = self
            .objects
            .get_mut(&id)
            .and_then(|object| object.begin_exact_veterancy_level_for_world_callback(level))
        else {
            return Ok(false);
        };

        if let Some(upgrade) = crate::game_logic::host_unit_training::veterancy_upgrade_name(level)
        {
            self.apply_upgrade_to_object_with_interpreter_checked(id, upgrade, None, effects)?;
        }

        let same_birth = self.objects.get(&id).is_some_and(|object| {
            birth.ptr_eq(&std::sync::Arc::downgrade(
                &object.visual_draw_identity_token,
            ))
        });
        if !same_birth {
            return Ok(true);
        }
        let health_bonus = self.veterancy_health_bonus;
        if provide_feedback {
            self.with_object_veterancy_feedback_and_locomotors(
                id,
                |object, audio, feedback, _locomotors| {
                    object.apply_veterancy_bonuses_after_rank_upgrade_in(
                        previous,
                        level,
                        &health_bonus,
                        audio,
                        feedback,
                    );
                },
            );
        } else if let Some(object) = self.objects.get_mut(&id) {
            let frame = self.frame;
            object.apply_veterancy_bonuses_after_rank_upgrade_quiet_in(
                previous,
                level,
                &health_bonus,
                frame,
            );
        }
        Ok(true)
    }
}

impl super::GameLogic {
    /// C++ InGameUI preDraw phase. Snapshot collection itself stays read-only.
    pub(crate) fn advance_world_animations_for_presentation(&mut self) {
        self.session_view
            .world_animations
            .advance(self.frame, self.is_paused());
    }
    pub(crate) fn admit_money_pickup_animation(
        &mut self,
        animation: super::host_money_crate::HostMoneyPickUpAnim,
    ) {
        if let Some(template) = self
            .promotion_definitions
            .animation_templates()
            .find(&animation.template)
        {
            self.session_view.world_animations.admit(
                crate::presentation_frame::PresentationWorldAnim::from_money_pickup(&animation),
                template,
            );
        }
        self.host_money_crates.record_money_pickup_anim(animation);
    }
}

impl super::GameLogic {
    pub(crate) fn with_object_recrew_inputs<R>(
        &mut self,
        id: super::ObjectId,
        action: impl FnOnce(
            &mut Object,
            &mut super::host_owner_log::OwnershipEvents,
            &mut ObjectAudio<'_>,
            &mut VeterancyFeedback<'_>,
            &mut super::partition_manager::PartitionManager,
            Option<&LocomotorStore>,
        ) -> R,
    ) -> Option<R> {
        let local = self.local_player_id();
        let draw_icon_ui = self.get_draw_icon_ui();
        let mut feedback = VeterancyFeedback::new(
            self.frame,
            local,
            draw_icon_ui,
            &self.promotion_definitions,
            &mut self.session_view.world_animations,
        );
        let mut audio = ObjectAudio::queue(&mut self.queued_audio_events);
        Some(action(
            self.objects.get_mut(&id)?,
            &mut self.ownership_events,
            &mut audio,
            &mut feedback,
            &mut self.partition_manager,
            self.locomotor_definitions.as_ref(),
        ))
    }
}
