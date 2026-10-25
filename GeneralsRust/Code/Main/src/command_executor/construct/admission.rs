//! The tail of DozerAIUpdate::construct, shared by fresh player and AI builds.
use super::*;

impl GameLogic {
    /// BuildAssistant.cpp:351, after clear/scoot and before construct checks.
    /// This individual source-AI command is distinct from player/group Stop.
    pub(crate) fn dozer_prepare_construct_owned_with_effects(
        &mut self,
        constructor: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<std::sync::Arc<()>>> {
        let Some(birth) = self
            .host_object(constructor)
            .map(|object| std::sync::Arc::clone(&object.visual_draw_identity_token))
        else {
            return Ok(None);
        };
        // C++ ignores the void command's refusal and calls construct anyway.
        let _ = self.unit_command_ai_idle_owned_with_effects(constructor, effects)?;
        if !self.host_object(constructor).is_some_and(|object| {
            std::sync::Arc::ptr_eq(&birth, &object.visual_draw_identity_token)
        }) {
            return Err(gamelogic::GameLogicError::Configuration(
                "Dozer construct lost its constructor birth during Idle".into(),
            ));
        }
        // AIUpdate.h342's base construct returns NULL; only the installed
        // Dozer/Worker virtual interface supplies this task owner. The source
        // Idle above still runs before that virtual construct refusal.
        if !self.constructor_has_owned_dozer_source(constructor) {
            return Ok(None);
        }
        if !self
            .host_object(constructor)
            .unwrap()
            .dozer_tasks
            .primary
            .control
            .initialized
        {
            return Err(gamelogic::GameLogicError::Configuration(
                "Admitted Dozer constructor has no initialized primary owner".into(),
            ));
        }
        // Actual virtual construct entry, before factory/onCreate/refusal.
        self.dozer_record_construct_kind_owned(constructor, &birth, false)?;
        Ok(Some(birth))
    }

    /// Source construct entry latch, before factory/callback effects. Resume,
    /// newTask and cancellation deliberately do not call this operation.
    pub(crate) fn dozer_record_construct_kind_owned(
        &mut self,
        id: ObjectId,
        birth: &std::sync::Arc<()>,
        is_rebuild: bool,
    ) -> gamelogic::GameLogicResult<()> {
        let actor = self
            .objects
            .get_mut(&id)
            .filter(|actor| std::sync::Arc::ptr_eq(birth, &actor.visual_draw_identity_token))
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "Dozer construct kind received foreign/retired birth".into(),
                )
            })?;
        actor.dozer_tasks.construct_is_rebuild = is_rebuild;
        Ok(())
    }

    /// After the real factory, payment, placement and structure-created callback,
    /// admit the supplied scaffold through newTask. This is not aiResumeConstruction.
    /// The caller captures the constructor birth before the BuildAssistant Idle.
    pub(crate) fn dozer_admit_constructed_scaffold_owned(
        &mut self,
        constructor: ObjectId,
        constructor_birth: &std::sync::Arc<()>,
        scaffold: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        let same_constructor = self.host_object(constructor).is_some_and(|object| {
            std::sync::Arc::ptr_eq(constructor_birth, &object.visual_draw_identity_token)
        });
        if !same_constructor
            || !self.constructor_has_owned_dozer_source(constructor)
            || !self
                .host_object(constructor)
                .is_some_and(|object| object.dozer_tasks.primary.control.initialized)
            || self.host_object(scaffold).is_none()
        {
            // An actual callback failed the synchronous source continuation.
            // Earlier factory/payment effects remain committed, as on other
            // checked receiving boundaries; do not fabricate a refund or target.
            return Err(gamelogic::GameLogicError::Configuration(
                "Dozer construct lost its constructor birth or supplied scaffold".into(),
            ));
        }
        // DozerAIUpdate.cpp:1717 / WorkerAIUpdate.cpp:592-665.
        let _ = self.dozer_new_task_build(constructor, scaffold)?;
        if !self.host_object(constructor).is_some_and(|object| {
            std::sync::Arc::ptr_eq(constructor_birth, &object.visual_draw_identity_token)
                && object.dozer_tasks.requests[0].target == Some(scaffold)
                && object.dozer_tasks.requests[0].order_frame == self.frame
        }) {
            return Err(gamelogic::GameLogicError::Configuration(
                "Dozer construct did not admit the supplied fresh scaffold request".into(),
            ));
        }
        self.worker_exit_supply_for_dozer_task(constructor);
        Ok(())
    }
    fn constructor_has_owned_dozer_source(&self, constructor: ObjectId) -> bool {
        self.host_object(constructor)
            .and_then(|object| object.thing().template.receiving_client_template.as_ref())
            .and_then(|source| source.dozer_ai_identity())
            .is_some()
    }
}
