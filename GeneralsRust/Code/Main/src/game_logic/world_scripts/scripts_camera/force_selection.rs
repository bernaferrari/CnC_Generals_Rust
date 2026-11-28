use super::*;
use crate::game_logic::mission_scripts::CameraMoveToRequest;
use game_engine::common::audio::AudioEventRts;
use generals_sim::GameLogicResult;
use generals_sim::scripting::object_control_actions::ForceSelection;

impl HostScriptExecutionDriver<'_> {
    pub(super) fn receive_force_selection(
        &mut self,
        request: &ForceSelection<'_>,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
    ) -> GameLogicResult<()> {
        let Some((_, members)) =
            self.world
                .resolve_owned_script_team(request.team, calling_team, condition_team)?
        else {
            return Ok(());
        };
        // ScriptActions.cpp chooses by exact ThingTemplate name and ObjectId.
        // Drawable presence is checked on the chosen object, after ranking.
        let Some(target) = members
            .into_iter()
            .map(ObjectId)
            .filter(|id| {
                self.world
                    .host_object(*id)
                    .is_some_and(|object| object.get_template().name == request.template)
            })
            .min_by_key(|id| id.0)
        else {
            return Ok(());
        };
        let Some((player, position)) = self.world.force_select_local_drawable(target) else {
            return Ok(());
        };
        if let Some(ui) = self.ui.ui.as_deref_mut() {
            ui.mark_control_bar_dirty();
        }
        if !request.audio.is_empty() && request.audio != "NoSound" {
            if self.ui.audio.is_some() {
                let mut event = AudioEventRts::with_event_name(request.audio);
                event.set_player_index(player as i32);
                if let Err(error) = self.admit_owned_script_audio(&event, player as i32) {
                    log::warn!("Could not play scripted selection audio: {error}");
                }
            } else {
                self.world.queue_audio_event(
                    AudioEventRequest::new(request.audio).with_player_index(player as i32),
                );
            }
        }
        if request.center_in_view {
            // The source executes one 1ms waypoint step and preserves the current angle.
            self.world.start_camera_move_to_oriented(
                CameraMoveToRequest {
                    position,
                    seconds: 0.0,
                    camera_stutter_seconds: 0.0,
                    ease_in_seconds: 0.0,
                    ease_out_seconds: 0.0,
                },
                false,
            );
        }
        Ok(())
    }
}
