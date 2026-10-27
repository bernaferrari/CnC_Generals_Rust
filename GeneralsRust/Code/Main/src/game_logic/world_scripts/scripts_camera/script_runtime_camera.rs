//! Host script runtime loop and script-camera behavior.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    pub(in crate::game_logic) fn evaluate_and_execute_scripts(&mut self, dt: f32) {
        self.evaluate_and_execute_scripts_with_effects(dt, Default::default());
    }

    pub(in crate::game_logic) fn evaluate_and_execute_scripts_with_effects(
        &mut self,
        _dt: f32,
        mut ui: crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if !self.scripts_loaded {
            return;
        }

        // The installed ScriptActionHandler has no ScriptContext parameter.
        // Refresh its per-session local-player view at the synchronous action-walk
        // boundary so audio actions observe the same current player as this world.
        self.mission_scripts
            .set_script_local_player_id(self.local_player_id());

        // Host script path: named-unit/team/area queries hit HOST objects.
        // Crate evaluator sees the name→id map + query snapshot (no crate Objects).
        self.inject_host_script_query_snapshot();
        self.inject_host_supply_source_queries();

        // Increment script frame counter
        self.mission_script_counter += 1;

        // Leftover ScriptingEngine event queue / process_events is leftover-only
        // (hq-8ta4n). Live host conditions/actions walk ScriptEngine::update.
        // C++ GameLogic.cpp:3600 — one TheScriptEngine->UPDATE() per logic frame.
        // The interpreter remains owned and callback-visible throughout the walk.
        self.with_owned_script_engine(|engine, world| {
            let context = gamelogic::scripting::executor::ScriptContext {
                game_logic_id: 0,
                object_manager_id: 0,
                player_manager_id: 0,
                event_system_id: 0,
                camera_system_id: 0,
                audio_system_id: 0,
                partition_manager_id: 0,
                special_powers_id: 0,
                current_frame: world.frame,
                suppress_new_windows: false,
                host_trigger_world: Arc::clone(&world.host_trigger_world),
            };
            let mut driver = script_execution_driver::HostScriptExecutionDriver::with_effects(
                world,
                engine,
                ui.reborrow(),
            );
            if let Err(err) = engine.update_with_driver(context, &mut driver) {
                log::error!("ScriptEngine::update failed: {err}");
            }
        });
        // Flush requests queued outside the action walk as well.
        if let Err(error) = self.apply_script_action_requests_with_effects(None, ui) {
            log::error!("script presentation effects: {error}");
        }

        self.script_broadcasts
            .retain(|msg| self.sim_time_seconds <= msg.expires_at);

        if self
            .cinematic_text
            .as_ref()
            .is_some_and(|(_, expires_at)| self.sim_time_seconds > *expires_at)
        {
            self.cinematic_text = None;
            self.cinematic_font = None;
        }

        if self
            .military_caption
            .as_ref()
            .is_some_and(|(_, expires_at)| self.sim_time_seconds > *expires_at)
        {
            self.military_caption = None;
        }
    }

    pub(in crate::game_logic::game_logic) fn military_caption_duration_seconds(
        duration_ms: i32,
    ) -> f32 {
        (duration_ms as f32 / 1000.0).max(0.0)
    }
}
