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
