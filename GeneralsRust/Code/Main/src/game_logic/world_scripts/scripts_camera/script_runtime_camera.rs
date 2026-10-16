//! Host script runtime loop and script-camera behavior.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    pub(in crate::game_logic) fn evaluate_and_execute_scripts(&mut self, _dt: f32) {
        if !self.scripts_loaded {
            return;
        }

        // Host script path: named-unit/team/area queries hit HOST objects.
        // Crate evaluator sees the name→id map + query snapshot (no crate Objects).
        self.inject_host_named_unit_map_into_crate_tracker();
        self.inject_host_supply_source_queries();

        // Increment script frame counter
        self.mission_script_counter += 1;

        for event in script_events::drain_events() {
            match event {
                ScriptEvent::PlayerDefeated { player_id } => {
                    log::debug!(
                        "📜 Script event: player {} defeated (frame {})",
                        player_id,
                        self.frame
                    );
                    self.partition_manager.reveal_map_for_player_permanently(
                        &mut self
                            .world_services
                            .shroud()
                            .lock()
                            .unwrap_or_else(|e| e.into_inner()),
                        player_id,
                    );
                }
                ScriptEvent::RevealMapForPlayer { player_id } => {
                    log::debug!("📜 Script event: reveal map for player {}", player_id);
                    self.partition_manager.reveal_map_for_player(
                        &mut self
                            .world_services
                            .shroud()
                            .lock()
                            .unwrap_or_else(|e| e.into_inner()),
                        player_id,
                    );
                }
                ScriptEvent::CompletedSpecialPower {
                    player_id,
                    ref special_power_name,
                    creator_id,
                } => {
                    log::debug!(
                        "📜 Script event: completed special power {} player {} creator {}",
                        special_power_name,
                        player_id,
                        creator_id
                    );
                    if let Some(engine) = &self.owned_script_engine {
                        engine.notify_of_completed_special_power(
                            player_id as usize,
                            special_power_name,
                            creator_id,
                        );
                    }
                }

                ScriptEvent::AllianceStateChanged { player_id, state } => {
                    log::debug!(
                        "📜 Script event: alliance state {:?} for player {}",
                        state,
                        player_id
                    );
                }
            }
        }

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
            let mut driver =
                script_execution_driver::HostScriptExecutionDriver::with_interpreter(world, engine);
            if let Err(err) = engine.update_with_driver(context, &mut driver) {
                log::error!("ScriptEngine::update failed: {err}");
            }
        });
        // Flush requests queued outside the action walk as well.
        self.apply_script_action_requests();

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
