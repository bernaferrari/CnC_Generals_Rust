//! GameLogic::processDestroyList unlinks lookup/list before Object::~Object.
//! Earlier onDie/onDelete callbacks still operate on the installed Object.
use super::super::*;

#[cfg(all(test, feature = "game_client"))]
#[path = "physical_retirement_tests.rs"]
mod tests;

impl GameLogic {
    /// A failed native destructor is terminal for this session. Keeping the
    /// owned Object outside lookup permits diagnostics without replaying any
    /// already completed destructor callbacks or pretending it is live again.
    pub(crate) fn object_retirement_error(&self) -> Option<gamelogic::GameLogicError> {
        self.objects
            .failed_retirement_reason()
            .map(|reason| gamelogic::GameLogicError::Configuration(reason.to_owned()))
    }

    pub(in super::super) fn take_object_for_physical_retirement(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<Object>> {
        if let Some(error) = self.object_retirement_error() {
            return Err(error);
        }
        if self.owned_script_engine.is_none() {
            return Err(gamelogic::GameLogicError::Configuration(
                "physical retirement requires the receiving interpreter after its script loan"
                    .into(),
            ));
        }
        // RadarUpgrade::onDelete belongs to the still-installed phase. This
        // defensive admission is idempotent when onDelete already consumed it.
        self.remove_host_radar_provider(id);
        let Some(mut object) = self.objects.remove(&id) else {
            return Ok(None);
        };
        // GameLogic.cpp2501–2506 removes BOTH lookup and original list before
        // friend_deleteInstance. The owned value keeps its original team until
        // Object.cpp604 sendObjectDestroyed completes, then setTeam(NULL)607.
        self.host_radar_remove_object(id);
        if let Err(reason) = self.retire_owned_object_drawable(&object, effects) {
            self.objects
                .retain_failed_retirement(object, reason.clone());
            return Err(gamelogic::GameLogicError::Configuration(reason));
        }
        self.unlink_retiring_team_host_member(id, object.team_instance_id);
        // Original SightingInfo destruction does not undo remaining values.
        // Live direct-deletion residual stays in this owner grid until reset.
        object.forget_retired_partition_scalar_receipt();
        self.cancel_retiring_dock_reservation(&object);
        // Object::~Object623 follows Drawable/sendDestroyed/team teardown and
        // precedes module destruction. Pending dirty identity retires with it.
        self.partition_manager.unregister_object(id.0);
        object.partition_namespace = Default::default();
        object.host_trigger_world = Default::default();
        object.team_factory.clear();
        // Object.cpp635-640 module destruction, after actor Drawable/team/
        // partition teardown and before script destruction notification.
        if let Err(reason) = self.retire_jet_lockon_for_object(&mut object, effects) {
            self.objects
                .retain_failed_retirement(object, reason.clone());
            return Err(gamelogic::GameLogicError::Configuration(reason));
        }

        // Object.cpp665 follows current destructor obligations. Ordinary
        // death and direct deletion converge here on the same Object birth.
        if !object.name.is_empty() {
            self.named_script_cache
                .remove(id, &object.visual_draw_identity_token);
        }
        self.owned_script_engine
            .as_ref()
            .expect("retirement interpreter preflight")
            .notify_receiving_object_destruction(id.0);
        Ok(Some(object))
    }

    fn retire_owned_object_drawable(
        &self,
        object: &Object,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        #[cfg(feature = "game_client")]
        {
            let Some(view) = effects.view.as_deref_mut() else {
                return if self.requires_native_drawable_owner() {
                    Err(format!(
                        "physical Object {} removal requires its native Drawable owner",
                        object.id
                    ))
                } else {
                    Ok(())
                };
            };
            view.retire_physical_object_drawable(&crate::game_logic::NativeObjectRetirement {
                identity: self.live_draw_playback_identity(object),
                object,
                live_objects: &self.objects,
            })
        }
        #[cfg(not(feature = "game_client"))]
        if self.requires_native_drawable_owner() {
            Err(format!(
                "physical Object {} removal requires the native GameClient feature",
                object.id
            ))
        } else {
            Ok(())
        }
    }
}

impl GameLogic {
    /// C++ destroyAllObjectsImmediate: ordinary installed onDelete callbacks,
    /// then the existing ordered physical-destruction phase. Native resources
    /// are retired by this operation's explicit receiving presentation loan.
    pub(crate) fn destroy_all_objects_immediate_with_effects_checked(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if let Some(error) = self.object_retirement_error() {
            return Err(error);
        }
        if self.object_lifecycle_callback_depth != 0 {
            return Err(gamelogic::GameLogicError::Configuration(
                "bulk Object retirement cannot replace an active lifecycle callback".into(),
            ));
        }
        if self.requires_native_drawable_owner() && !self.objects.registered_is_empty() {
            #[cfg(feature = "game_client")]
            let has_owner = effects.view.is_some();
            #[cfg(not(feature = "game_client"))]
            let has_owner = false;
            if !has_owner {
                return Err(gamelogic::GameLogicError::Configuration(
                    "bulk Object retirement requires its native Drawable owner".into(),
                ));
            }
        }
        // Snapshot original linked-list order. Containment may append child
        // destruction, but never sort/requeue the existing pending events.
        let ids = self.objects.ordered_ids().to_vec();
        for id in ids {
            self.destroy_object_with_effects(id, effects);
        }
        self.process_destroy_list_with_effects_checked(effects)?;
        if !self.objects.registered_is_empty() || self.has_pending_object_removals() {
            let remaining: Vec<_> = self
                .objects
                .ordered_ids()
                .iter()
                .filter_map(|id| self.objects.get(id))
                .take(8)
                .map(|object| {
                    (
                        object.id,
                        object.template_name.as_str(),
                        object.status.destroyed,
                        object.status.on_die_started,
                    )
                })
                .collect();
            return Err(gamelogic::GameLogicError::Configuration(format!(
                "bulk Object retirement left live or pending objects: live={}, pending={}, registered={}, sample={remaining:?}",
                self.objects.registered_len(),
                self.objects_to_destroy.len(),
                self.objects.ordered_ids().len(),
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../object/runtime_attachment_owner_tests.rs"]
mod runtime_attachment_owner_tests;
