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
        // RadarUpgrade::onDelete belongs to the still-installed phase. This
        // defensive admission is idempotent when onDelete already consumed it.
        self.remove_host_radar_provider(id);
        let Some(object) = self.objects.remove(&id) else {
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
        self.cancel_retiring_dock_reservation(&object);
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
