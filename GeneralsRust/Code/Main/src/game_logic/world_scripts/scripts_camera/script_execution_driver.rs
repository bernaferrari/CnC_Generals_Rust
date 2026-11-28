//! Live synchronous Main owner for one ScriptEngine action walk.
use super::*;
use game_engine::common::random_value::SessionRandom;
use game_engine::common::rts::science::ScienceType;
use game_engine::common::game_common::Relationship;
use gamelogic::scripting::ScriptSurfacesAllowed;
use gamelogic::scripting::engine::ScriptEngine;
use generals_sim::scripting::context::ScriptContext;
use generals_sim::scripting::driver::ScriptExecutionDriver;
use generals_sim::scripting::object_actions::ObjectPanelFlag;
use generals_sim::scripting::world::{
    ScriptAiPlayerRequest, ScriptAttacker, ScriptBridgeStatus, ScriptCameraRequest,
    ScriptCommandButton, ScriptConditionTeam, ScriptDamageSource, ScriptDisplayRequest, ScriptNamedCommand,
    ScriptObjectStatus, ScriptOwnerQuery, ScriptPlayerAreaQuery,
    ScriptPlayerConstructionRequest, ScriptPlayerRankRequest,
    ScriptSightingCandidate, ScriptSightingFilter, ScriptSightingObserver, ScriptSkirmishCensus,
    ScriptSpecialPowerReadiness, ScriptSupplyQuery,
    ScriptTeamPresence, ScriptTeamStatus, ScriptWaterRequest,
};
use std::sync::{Arc, RwLock};

#[path = "script_player_resolution.rs"]
mod script_player_resolution;
#[path = "named_map_reveals.rs"]
mod named_map_reveals;
#[path = "script_radar_events.rs"]
mod script_radar_events;
#[path = "script_command_button_readiness.rs"]
mod script_command_button_readiness;
#[path = "force_selection.rs"]
mod force_selection;
#[path = "presentation_driver.rs"]
mod presentation_driver;
#[path = "mission_driver.rs"]
mod mission_driver;

pub(super) struct HostScriptExecutionDriver<'a> {
    world: &'a mut GameLogic,
    interpreter: Option<&'a gamelogic::scripting::engine::ScriptEngine>,
    ui: crate::game_logic::ScriptPresentationEffects<'a>,
    #[cfg(test)]
    after_action_observer: Option<&'a mut dyn FnMut(&GameLogic)>,
}

impl<'a> HostScriptExecutionDriver<'a> {
    pub(super) fn new(world: &'a mut GameLogic) -> Self {
        Self {
            world,
            interpreter: None,
            ui: Default::default(),
            #[cfg(test)]
            after_action_observer: None,
        }
    }

    pub(super) fn with_interpreter(
        world: &'a mut GameLogic,
        interpreter: &'a gamelogic::scripting::engine::ScriptEngine,
    ) -> Self {
        Self {
            world,
            interpreter: Some(interpreter),
            ui: crate::game_logic::ScriptPresentationEffects {
                script_events: Some(crate::game_logic::ScriptEventRecipient::new(interpreter)),
                ..Default::default()
},
            #[cfg(test)]
            after_action_observer: None,
        }
    }

    pub(super) fn with_effects(
        world: &'a mut GameLogic,
        interpreter: &'a gamelogic::scripting::engine::ScriptEngine,
        mut ui: crate::game_logic::ScriptPresentationEffects<'a>,
    ) -> Self {
        ui.script_events = Some(crate::game_logic::ScriptEventRecipient::new(interpreter));
        world.deliver_pending_script_ui(&mut ui);
        Self {
            world,
            interpreter: Some(interpreter),
            ui,
            #[cfg(test)]
            after_action_observer: None,
        }
    }

    pub(super) fn advance_frame(&mut self) -> generals_sim::GameLogicResult<()> {
        let engine = self.interpreter.ok_or_else(|| {
            generals_sim::GameLogicError::Configuration(
                "Script frame requires the session's interpreter".into(),
            )
        })?;
        let context = self.world.script_context();
        engine.with_script_program(context, self, |runtime, frame, program| {
            runtime.advance_frame(frame, program)
        })
    }

    // Explicit UI-only adapter for standalone callers; native roots lend both domains.
    pub(super) fn with_ui(
        world: &'a mut GameLogic,
        interpreter: &'a gamelogic::scripting::engine::ScriptEngine,
        mut ui: Option<&'a mut dyn crate::game_logic::ScriptUiEffects>,
    ) -> Self {
        let effects = crate::game_logic::ScriptPresentationEffects {
            pristine_bones: None,
            script_events: None,
            partition: None,
            trigger_world: None,
            audio: None,
            terrain: None,
            ui: ui,
            #[cfg(feature = "game_client")]
            headless_particles: None,
            view: None,
        };
        Self::with_effects(world, interpreter, effects)
    }

    /// Callback delivery while the interpreter is already borrowed by a frame
    /// or AI script walk uses that exact interpreter, never a missing field.
    pub(crate) fn complete_structure(&mut self, object: ObjectId) {
        self.world
            .notify_structure_construction_complete_with_effects(
                object,
                self.interpreter,
                &mut self.ui,
            );
    }

    /// Observe production state at the immediate between-action callback in
    /// unit tests; this field/method do not exist in production builds.
    #[cfg(test)]
    pub(super) fn with_after_action_observer(
        mut self,
        observer: &'a mut dyn FnMut(&GameLogic),
    ) -> Self {
        self.after_action_observer = Some(observer);
        self
    }

    fn choose_victim_uses_normal(&self) -> bool {
        self.interpreter
            .or(self.world.owned_script_engine.as_ref())
            .expect("script target selection must borrow its receiving interpreter")
            .get_choose_victim_always_uses_normal()
    }

    fn status(object: &crate::game_logic::object::Object) -> ScriptObjectStatus {
        ScriptObjectStatus {
            has_ai: object.has_ai_update_interface(),
            idle: matches!(object.ai_state(), AIState::Idle)
                && !object.status.moving
                && !object.status.attacking,
            effectively_dead: object.status.effectively_dead
                || object.status.destroyed
                || !object.is_alive(),
        }
    }

    fn owned_team(&self, id: u32) -> Option<Arc<RwLock<gamelogic::team::Team>>> {
        self.world
            .team_factory
            .lock()
            .expect("owned script team factory")
            .find_team_by_id(id)
    }
}

impl HostScriptExecutionDriver<'_> {
    fn owned_audio_extra_receipts(&self) -> Vec<u32> {
        let mut extra_handles = Vec::new();
        if let (Some(view), Some(audio)) = (self.ui.view.as_deref(), self.ui.audio.as_deref()) {
            view.visit_owned_ambient_audio_handles(&mut |device, handle| {
                if audio.accepts_audio_device(device) {
                    extra_handles.push(handle);
                }
            });
        }
        if let (Some(ui), Some(audio)) = (self.ui.ui.as_deref(), self.ui.audio.as_deref()) {
            ui.visit_owned_menu_audio_receipts(&mut |device, handles| {
                if audio.accepts_audio_device(device) {
                    extra_handles.extend_from_slice(handles);
                }
            });
        }
        extra_handles
    }

    fn admit_owned_script_audio(
        &mut self,
        event: &game_engine::common::audio::AudioEventRts,
        source_player: i32,
    ) -> generals_sim::GameLogicResult<()> {
        let locality = self
            .world
            .receiving_audio_locality_for_player(source_player);
        let Some(audio) = self.ui.audio.as_deref_mut() else {
            return Ok(());
        };
        let mut selection = crate::game_logic::SessionAudioSelection {
            logic: &self.world.logic_random,
            audio: &mut self.world.session_view.audio_random,
        };
        audio
            .submit_selected_audio(
                event,
                &locality,
                &mut selection,
                self.world.queued_audio_events.policy(),
            )
            .map_err(generals_sim::GameLogicError::Configuration)
    }

    /// CPP OpenContain::addOrRemoveObjFromWorld removes enclosed riders and
    /// their descendants from the partition. Non-enclosing riders stay visible.
    fn is_in_script_partition(&self, candidate: &Object) -> bool {
        let mut rider = candidate;
        // A valid containment chain cannot visit more objects than this world
        // owns. Broken references and cycles cannot supply a visible candidate.
        for _ in 0..self.world.host_objects().registered_len() {
            let Some(container_id) = rider.contained_by else {
                return true;
            };
            let Some(container) = self.world.host_object(container_id) else {
                return false;
            };
            if container.is_enclosing_container_for(rider) {
                return false;
            }
            rider = container;
        }
        false
    }
}

impl ScriptExecutionDriver for HostScriptExecutionDriver<'_> {
    fn presentation_world(
        &mut self,
    ) -> Option<&mut dyn generals_sim::scripting::presentation_actions::ScriptPresentationWorld> {
        Some(self)
    }

    fn logic_random(&self) -> Option<&SessionRandom> {
        Some(&self.world.logic_random)
    }

    fn script_audio_duration_ms(&self, name: &str) -> Option<f32> {
        let event = gamelogic::common::audio::AudioEventRts::new(name);
        Some(gamelogic::helpers::TheAudio::of(&self.world.world_services).get_audio_length_ms(&event))
    }

    fn script_player_name_token(
        &mut self,
        raw: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<Option<String>> {
        match self.query_script_player_id(raw, current_player) {
            Some(id) => ScriptOwnerQuery::Present(
                self.world
                    .players
                    .get(&id)
                    .map(|player| player.map_side.map_player_name.clone()),
            ),
            None => {
                use generals_sim::scripting::program::{
                    LOCAL_PLAYER, THE_PLAYER, THIS_PLAYER, THIS_PLAYER_ENEMY,
                };
                let authored_selector = matches!(
                    raw,
                    LOCAL_PLAYER | THE_PLAYER | THIS_PLAYER | THIS_PLAYER_ENEMY
                );
                if authored_selector {
                    ScriptOwnerQuery::Present(None)
                } else {
                    ScriptOwnerQuery::Present(Some(raw.to_string()))
                }
            }
        }
    }

    fn side_script_current_player(&self, side_index: usize) -> ScriptOwnerQuery<Option<String>> {
        // ScriptEngine.cpp:5555-5557 reads the PlayerList slot at the script
        // list's ordinal, not the side dictionary's player-name key.
        let player = u32::try_from(side_index)
            .ok()
            .and_then(|id| self.world.players.get(&id));
        ScriptOwnerQuery::Present(player.map(|player| player.map_side.map_player_name.clone()))
    }

    fn current_player_ai_difficulty(
        &self,
        player: &str,
    ) -> ScriptOwnerQuery<generals_sim::scripting::program_execution::ScriptDifficulty> {
        use generals_sim::scripting::program_execution::ScriptDifficulty;

        let difficulty = self.world.authored_script_player_id(player)
            .and_then(|id| self.world.ai_manager.ai_difficulty(id));
        difficulty.map_or(ScriptOwnerQuery::Missing, |difficulty| {
            ScriptOwnerQuery::Present(ScriptDifficulty::from(difficulty))
        })
    }

    fn music_track_completed(
        &mut self,
        track: &str,
        number_of_times: i32,
    ) -> Option<generals_sim::GameLogicResult<bool>> {
        let extra_handles = self.owned_audio_extra_receipts();
        Some(match self.ui.audio.as_deref_mut() {
            Some(audio) => audio
                .music_track_completed(track, number_of_times, &extra_handles)
                .map_err(generals_sim::GameLogicError::Configuration),
            None => Ok(false),
        })
    }

    fn speech_play(
        &mut self,
        name: &str,
        allow_overlap: bool,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.play_script_speech(name, allow_overlap))
    }

    fn audio(
        &mut self,
        request: generals_sim::scripting::world::ScriptAudioRequest<'_>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.apply_script_audio(request))
    }

    fn set_campaign_victorious(
        &mut self,
        victorious: bool,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        use generals_sim::scripting::mission_actions::ScriptMissionWorld;
        Some(ScriptMissionWorld::set_campaign_victorious(self, victorious))
    }

    fn create_win_lose_window(
        &mut self,
        layout_filename: &str,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        use generals_sim::scripting::mission_actions::ScriptMissionWorld;
        Some(ScriptMissionWorld::create_win_lose_window(self, layout_filename))
    }

    fn destroy_win_lose_window(&mut self) -> Option<generals_sim::GameLogicResult<()>> {
        use generals_sim::scripting::mission_actions::ScriptMissionWorld;
        Some(ScriptMissionWorld::destroy_win_lose_window(self))
    }
    fn close_game_windows(&mut self) -> Option<generals_sim::GameLogicResult<()>> {
        use generals_sim::scripting::mission_actions::ScriptMissionWorld;
        Some(ScriptMissionWorld::close_game_windows(self))
    }

    fn enqueue_clear_game_data(&mut self) -> Option<()> {
        use game_engine::common::message_stream::GameMessageType;
        let application = &self.world.world_services.common().application;
        if let Ok(mut stream) = application.message_stream.write() {
            stream.append_message(GameMessageType::ClearGameData);
        }
        Some(())
    }

    fn initialize_named_cache(&mut self) -> Option<()> {
        self.world.initialize_receiving_named_script_cache();
        Some(())
    }

    fn set_buildable_status_override(
        &mut self,
        template: &str,
        status: i32,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.world.script_set_buildable_override(template, status);
        Some(Ok(()))
    }
    fn command_bar_override(
        &mut self,
        button: &str,
        template: &str,
        slot: Option<i32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.world
            .script_command_bar_override(button, template, slot);
        Some(Ok(()))
    }

    fn local_victory_state(
        &self,
    ) -> generals_sim::scripting::world::ScriptOwnerQuery<(bool, bool, bool)> {
        let victory = &self.world.victory_conditions;
        let players = &self.world.players;
        generals_sim::scripting::world::ScriptOwnerQuery::Present((
            victory.is_local_allied_victory_owned(
                players,
                self.world.player_template_definitions.as_ref(),
                self.world.local_player_id(),
            ),
            victory.is_local_allied_defeat_owned(
                players,
                self.world.player_template_definitions.as_ref(),
                self.world.local_player_id(),
            ),
            victory.local_player_defeated(players, self.world.local_player_id()),
        ))
    }

    fn set_draw_icon_ui(&mut self, enabled: bool) -> Option<generals_sim::GameLogicResult<()>> {
        self.world.set_draw_icon_ui(enabled);
        Some(Ok(()))
    }

    fn set_show_behind_building_markers(
        &mut self,
        enabled: bool,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.world.set_show_behind_building_markers(enabled);
        Some(Ok(()))
    }

    fn override_hulk_lifetime(&mut self, frames: i32) -> Option<generals_sim::GameLogicResult<()>> {
        self.world.native_hulk_max_lifetime_override = frames;
        Some(Ok(()))
    }
    fn object_lifecycle(
        &mut self,
        request: &generals_sim::scripting::lifecycle_actions::LifecycleAction,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_script_lifecycle_with_effects(
            request,
            this_object.map(ObjectId),
            calling,
            condition,
            self.interpreter,
            &mut self.ui,
        ))
    }

    fn player_relates(
        &mut self,
        source: &str,
        destination: &str,
        relationship: Relationship,
        current: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        // ScriptActions.cpp resolves the destination first. Resolving the
        // enemy alias can advance the active AI's enemy-acquisition clock.
        let destination = self.query_script_player_id(destination, current);
        let source = self.query_script_player_id(source, current);
        if let Some((source, destination)) = source.zip(destination)
            && let Some(player) = self.world.players.get_mut(&source)
        {
            player.set_map_relationship(destination, relationship);
            // The next path query in this script walk must see the new relation.
            self.world.refresh_pathfind_ally_masks();
        }
        Some(Ok(()))
    }

    fn prioritize_skirmish_building(
        &mut self,
        template: &str,
        current_player: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        if let Some(id) = current_player.and_then(|name| self.world.authored_script_player_id(name)) {
            self.world.ai_manager.build_specific_ai_building(id, template);
        }
        Some(Ok(()))
    }

    fn player_hunt(
        &mut self,
        player: &str,
        current: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        if let Some(id) = self.query_script_player_id(player, current) {
            self.world.apply_script_player_hunt(id);
        }
        Some(Ok(()))
    }

    fn exclude_player_from_score(
        &mut self,
        player: &str,
        current: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        if let Some(id) = self.query_script_player_id(player, current)
            && let Some(player) = self.world.players.get_mut(&id)
        {
            player.list_in_score_screen = false;
        }
        Some(Ok(()))
    }

    fn kill_player(
        &mut self,
        player: &str,
        current: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        if let Some(id) = self.script_player_id(player, current) {
            return Some(self.world.kill_player_with_interpreter_with_effects(
                id,
                self.interpreter,
                &mut self.ui,
            ));
        }
        Some(Ok(()))
    }

    fn named_frame_command(
        &mut self,
        request: ScriptNamedCommand<'_>,
        this_object: Option<u32>,
    ) -> ScriptOwnerQuery<()> {
        let unit = match request {
            ScriptNamedCommand::Stop { unit } | ScriptNamedCommand::Guard { unit } => unit,
            _ => return ScriptOwnerQuery::Missing,
        };
        let Some(id) = self
            .world
            .owned_named_script_object(unit, this_object.map(ObjectId))
        else {
            return ScriptOwnerQuery::Missing;
        };
        if !self
            .world
            .host_object(id)
            .is_some_and(|object| object.has_ai_update_interface())
        {
            return ScriptOwnerQuery::Missing;
        }
        match request {
            ScriptNamedCommand::Stop { .. } => self.world.apply_owned_script_idle(id),
            ScriptNamedCommand::Guard { .. } => {
                // CPP4793 differs from NamedGuard: no leaveGroup.
                let position = self.world.host_object(id).unwrap().get_position();
                self.world.apply_unit_locomotor_set(id, "normal");
                self.world.apply_owned_script_guard_position(id, position);
            }
            _ => unreachable!(),
        }
        ScriptOwnerQuery::Present(())
    }

    fn money(
        &mut self,
        request: &generals_sim::scripting::world::HostScriptMoneyRequest,
        current_player: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        use generals_sim::scripting::world::HostScriptMoneyRequest;
        let (player, amount) = match request {
            HostScriptMoneyRequest::Set { player, amount }
            | HostScriptMoneyRequest::Give { player, amount } => (player, *amount),
        };
        if let Some(id) = self.script_player_id(player, current_player) {
            match request {
                HostScriptMoneyRequest::Set { .. } => {
                    self.world.host_script_set_player_money(id, amount);
                }
                HostScriptMoneyRequest::Give { .. } => {
                    self.world.host_script_give_player_money(id, amount);
                }
            }
        }
        Some(Ok(()))
    }

    fn object_status_action(
        &mut self,
        action: &generals_sim::scripting::object_status_actions::ObjectStatusAction,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_script_object_status(
            action,
            this_object.map(ObjectId),
            calling,
            condition,
            self.interpreter,
            &mut self.ui,
        ))
    }

    fn object_sound(
        &mut self,
        request: &generals_sim::scripting::world::HostScriptObjectSoundRequest,
        this_object: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.apply_script_object_sound(request, this_object))
    }

    fn radar_event(
        &mut self,
        target: generals_sim::scripting::radar::RadarEventTarget<'_>,
        kind: game_engine::common::system::radar::RadarEventType,
        this_object: Option<u32>,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.apply_radar_event(target, kind, this_object, calling_team, condition_team))
    }

    fn named_containment(
        &self,
        unit: &str,
        this_object: Option<u32>,
    ) -> ScriptOwnerQuery<generals_sim::scripting::world::ScriptContainmentStatus> {
        let Some(id) = self
            .world
            .owned_named_script_object(unit, this_object.map(ObjectId))
        else {
            return ScriptOwnerQuery::Missing;
        };
        self.world
            .owned_script_containment(id)
            .map(ScriptOwnerQuery::Present)
            .unwrap_or(ScriptOwnerQuery::Missing)
    }

    fn named_owned_by_player(
        &self,
        unit: &str,
        player: &str,
        current_player: Option<&str>,
        this_object: Option<u32>,
    ) -> ScriptOwnerQuery<bool> {
        let Some(player) = self.script_player_id(player, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(object) = self
            .world
            .owned_named_script_object(unit, this_object.map(ObjectId))
            .and_then(|id| self.world.host_object(id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present(object.owner_player_id == Some(player))
    }

    fn named_lifecycle(
        &self,
        name: &str,
        this_object: Option<u32>,
    ) -> ScriptOwnerQuery<generals_sim::scripting::world::ScriptNamedLifecycle> {
        let object = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
            .and_then(|id| self.world.host_object(id));
        ScriptOwnerQuery::Present(generals_sim::scripting::world::ScriptNamedLifecycle {
            exists: object.is_some(),
            effectively_dead: object.is_some_and(|object| object.status.effectively_dead),
            ever_existed: self.world.named_script_cache.did_exist(name),
        })
    }

    fn set_object_difficulty_bonus(&mut self, enable: bool) -> Option<()> {
        self.world
            .set_owned_object_difficulty_bonus_with_effects(enable, &mut self.ui);
        Some(())
    }

    fn update_team_states(
        &mut self,
        runtime: &generals_sim::scripting::interpreter::ScriptInterpreter,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let Some(engine) = self
            .interpreter
            .filter(|engine| std::ptr::eq(engine.interpreter(), runtime))
        else {
            return Some(Err(generals_sim::GameLogicError::Configuration(
                "Team-state phase requires the running interpreter's script executor".into(),
            )));
        };
        Some(
            self.world
                .update_owned_script_team_states_with_effects(engine, &mut self.ui),
        )
    }

    fn local_player_observer(&self) -> ScriptOwnerQuery<bool> {
        use generals_sim::scripting::mission_actions::ScriptMissionWorld;
        ScriptOwnerQuery::Present(ScriptMissionWorld::local_player_is_observer(self))
    }

    fn create_named_object(
        &mut self,
        name: &str,
        request: generals_sim::scripting::world::ScriptObjectCreation<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.receive_create_named_object(name, request, calling, condition)
    }

    fn create_named_object_at_waypoint(
        &mut self,
        name: Option<&str>,
        template: &str,
        team: &str,
        waypoint: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.receive_create_named_object_at_waypoint(
            name, template, team, waypoint, calling, condition,
        )
    }

    fn create_object(
        &mut self,
        request: generals_sim::scripting::world::ScriptObjectCreation<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_script_create_object_with_effects(
            request,
            calling,
            condition,
            self.interpreter,
            &mut self.ui,
        ))
    }

    fn object_appearance(
        &mut self,
        action: &generals_sim::scripting::object_appearance_actions::ObjectAppearanceAction,
        this_object: Option<u32>,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_script_object_appearance(
            action,
            this_object.map(ObjectId),
            calling_team,
            condition_team,
        ))
    }

    fn team_attack_named(
        &mut self,
        team: &str,
        target: &str,
        this_object: Option<gamelogic::common::ObjectID>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_team_attack_named(
            team,
            target,
            this_object.map(ObjectId),
            calling,
            condition,
            &mut self.ui,
        ))
    }

    fn named_object_id(&self, name: &str, this_object: Option<u32>) -> ScriptOwnerQuery<u32> {
        self.world
            .owned_named_script_object(name, this_object.map(ObjectId))
            .map_or(ScriptOwnerQuery::Missing, |id| {
                ScriptOwnerQuery::Present(id.0)
            })
    }
    fn named_superweapon_display(
        &mut self,
        name: &str,
        show: bool,
        this_object: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let Some(id) = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
        else {
            return Some(Ok(()));
        };
        let mutation = if show {
            crate::game_logic::mission_scripts::SuperweaponObjectDisplayMutation::Show {
                object_id: id.0,
            }
        } else {
            crate::game_logic::mission_scripts::SuperweaponObjectDisplayMutation::Hide {
                object_id: id.0,
            }
        };
        self.world
            .mission_scripts
            .push_superweapon_object_display_mutation(mutation);
        Some(Ok(()))
    }
    fn team_guard_position(
        &mut self,
        team: &str,
        waypoint: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_guard_position(team, waypoint, calling, condition),
        )
    }
    fn team_guard_area(
        &mut self,
        team: &str,
        area: &str,
        current_player: Option<&str>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let Some(area) = self.world.qualified_script_area(area, current_player) else {
            return Some(Ok(()));
        };
        Some(
            self.world
                .apply_owned_team_guard_area(team, &area, calling, condition),
        )
    }
    fn team_guard_object(
        &mut self,
        team: &str,
        target: &str,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_team_guard_object(
            team,
            target,
            this_object.map(ObjectId),
            calling,
            condition,
        ))
    }
    fn team_attack_team(
        &mut self,
        source: &str,
        target: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let force_normal = self.choose_victim_uses_normal();
        Some(self.world.apply_owned_team_attack_team(
            source,
            target,
            calling,
            condition,
            force_normal,
            &mut self.ui,
        ))
    }

    fn team_attack_area(
        &mut self,
        team: &str,
        area: &str,
        current_player: Option<&str>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_team_attack_area(
            team,
            area,
            current_player,
            calling,
            condition,
            &mut self.ui,
        ))
    }

    fn named_waypoint(
        &mut self,
        unit: &str,
        waypoint: &str,
        follow: Option<bool>,
        this_object: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.world
            .apply_owned_named_waypoint(unit, waypoint, follow, this_object.map(ObjectId));
        Some(Ok(()))
    }
    fn named_fire_weapon_following_waypoint_path(
        &mut self,
        unit: &str,
        waypoint_path: &str,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world
            .host_script_named_fire_weapon_following_path_with_effects(
                unit,
                waypoint_path,
                &mut self.ui,
            ))
    }
    fn team_hunt_button(
        &mut self,
        team: &str,
        button: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_hunt_button(team, button, calling, condition),
        )
    }
    fn team_hunt(
        &mut self,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_team_hunt(team, calling, condition))
    }
    fn team_load_transports(
        &mut self,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_load_transports(team, calling, condition),
        )
    }
    fn team_relationship_override(
        &mut self,
        action: generals_sim::scripting::world::ScriptTeamRelationship<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_relationship_override(action, calling, condition),
        )
    }
    fn player_team_relationship_override(
        &mut self,
        action: generals_sim::scripting::world::ScriptPlayerTeamRelationship<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_player_team_relationship_override(action, calling, condition),
        )
    }
    fn team_capture_nearest_unowned_faction_unit(
        &mut self,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        if team == generals_sim::scripting::program::TEAM_THE_PLAYER && self.world.is_challenge_session()
        {
            let result = (|| {
                let Some(player_id) = self.world.local_player_id() else {
                    return Ok(());
                };
                let Some((team_id, roster)) =
                    self.world.resolve_owned_player_default_team(player_id)?
                else {
                    return Ok(());
                };
                self.world
                    .apply_owned_team_capture_nearest_roster(team_id, roster)
            })();
            return Some(result);
        }
        Some(
            self.world
                .apply_owned_team_capture_nearest_unowned(team, calling, condition),
        )
    }
    fn team_transfer(
        &mut self,
        team: &str,
        player: &str,
        current_player: Option<&str>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let Some(player) = self.script_player_id(player, current_player) else {
            return Some(Ok(()));
        };
        Some(self.world.apply_owned_team_transfer(
            team,
            player,
            calling,
            condition,
            self.interpreter,
            &mut self.ui,
        ))
    }
    fn transfer_player_assets(
        &mut self,
        source: &str,
        destination: &str,
        current_player: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let (Some(source), Some(destination)) = (
            self.script_player_id(source, current_player),
            self.script_player_id(destination, current_player),
        ) else {
            return Some(Ok(()));
        };
        Some(self.world.host_script_transfer_assets_from_with_effects(
            destination,
            source,
            &mut self.ui,
        ))
    }
    fn sell_everything(
        &mut self,
        player: &str,
        current_player: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        if let Some(player) = self.script_player_id(player, current_player) {
            return Some(self.world
                .apply_owned_sell_everything_with_effects(player, &mut self.ui));
        }
        Some(Ok(()))
    }

    fn team_waypoint_path(
        &mut self,
        team: &str,
        path: &str,
        as_team: bool,
        exact: bool,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_waypoint_path(team, path, as_team, exact, calling, condition),
        )
    }

    fn team_move_to_waypoint(
        &mut self,
        team: &str,
        waypoint: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        if team == generals_sim::scripting::program::TEAM_THE_PLAYER && self.world.is_challenge_session()
        {
            let Some(player_id) = self.world.local_player_id() else {
                return Some(Err(generals_sim::GameLogicError::Configuration(
                    "challenge TeamMove has no receiving local Player".into(),
                )));
            };
            let members = match self.world.resolve_owned_player_default_team(player_id) {
                Ok(Some((_, members))) => members,
                Ok(None) => {
                    return Some(Err(generals_sim::GameLogicError::Configuration(
                        "challenge TeamMove has no admitted local default Team".into(),
                    )));
                }
                Err(error) => return Some(Err(error)),
            };
            return Some(
                self.world
                    .apply_owned_team_move_members_to_waypoint(&members, waypoint),
            );
        }
        Some(
            self.world
                .apply_owned_team_move_to_waypoint(team, waypoint, calling, condition),
        )
    }

    fn named_attack_area(
        &mut self,
        unit: &str,
        area: &str,
        current_player: Option<&str>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let Some(source) = self.world.owned_named_script_object(unit, this_object.map(ObjectId))
        else {
            return Some(Ok(()));
        };
        let Some(area) = self.world.qualified_script_area(area, current_player) else {
            return Some(Ok(()));
        };
        Some(self.world.apply_owned_named_attack_area_with_effects(
            source,
            &area,
            &mut self.ui,
        ))
    }

    fn named_attack_team(
        &mut self,
        unit: &str,
        team: &str,
        this_object: Option<gamelogic::common::ObjectID>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let force_normal = self.choose_victim_uses_normal();
        Some(self.world.apply_owned_named_attack_team(
            unit,
            team,
            this_object.map(ObjectId),
            calling,
            condition,
            force_normal,
            &mut self.ui,
        ))
    }

    fn team_nearest_button(
        &mut self,
        request: &generals_sim::scripting::command_button_actions::CommandButtonAction<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_nearest_button(request, calling, condition),
        )
    }

    fn team_partial_button(
        &mut self,
        team: &str,
        button: &str,
        percentage: f32,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_partial_button(team, button, percentage, calling, condition),
        )
    }
    fn skirmish_attack_nearest_group(
        &mut self,
        team: &str,
        comparison: Option<generals_sim::scripting::program::ComparisonType>,
        value: i32,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_skirmish_attack_nearest_group(
            team,
            comparison,
            value,
            calling,
            condition,
            &mut self.ui,
        ))
    }

    fn skirmish_button_on_most_valuable(
        &mut self,
        team: &str,
        button: &str,
        range: f32,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let Some(interpreter) = self.interpreter else {
            return Some(Err(generals_sim::GameLogicError::Configuration(
                "Skirmish command-button action requires its receiving interpreter".into(),
            )));
        };
        Some(self.world.apply_owned_skirmish_button_on_most_valuable(
            team,
            button,
            range,
            calling,
            condition,
            interpreter,
            &mut self.ui,
        ))
    }

    fn team_panic(
        &mut self,
        team: &str,
        path: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_panic(team, path, calling, condition),
        )
    }
    fn team_wander(
        &mut self,
        team: &str,
        path: Option<&str>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_wander(team, path, calling, condition),
        )
    }
    fn skirmish_approach_path(
        &mut self,
        team: &str,
        path: &str,
        as_team: bool,
        follow: bool,
        current_player: Option<&str>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let player = current_player.and_then(|name| self.world.authored_script_player_id(name));
        Some(self.world.apply_owned_skirmish_approach_path(
            team, path, as_team, follow, player, calling, condition,
        ))
    }
    // CPP ScriptConditions1301-1325: resolve actual named Object, require AI,
    // then compare the completed waypoint's labels exactly. No active goal or
    // motion destination can stand in for a completed waypoint.
    fn named_waypoint_complete(
        &self,
        unit: &str,
        path: &str,
        this_object: Option<u32>,
    ) -> ScriptOwnerQuery<bool> {
        let Some(object) = self
            .world
            .owned_named_script_object(unit, this_object.map(ObjectId))
            .and_then(|id| self.world.host_object(id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present(
            object.has_ai_update_interface()
                && object
                    .completed_waypoint_labels
                    .iter()
                    .any(|label| label == path),
        )
    }
    fn team_waypoint_complete(
        &self,
        team: &str,
        path: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> ScriptOwnerQuery<bool> {
        let Some((_, members)) = self
            .world
            .resolve_owned_script_team(team, calling, condition)
            .expect("owned completed waypoint team")
        else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present(
            members
                .into_iter()
                .filter_map(|id| self.world.host_object(ObjectId(id)))
                .any(|object| {
                    object.has_ai_update_interface()
                        && object
                            .completed_waypoint_labels
                            .iter()
                            .any(|label| label == path)
                }),
        )
    }

    fn team_area_membership(
        &mut self,
        team: &str,
        area: &str,
        current_player: Option<&str>,
        calling: Option<u32>,
        condition: Option<u32>,
        which_to_consider: u32,
    ) -> ScriptOwnerQuery<generals_sim::scripting::world::ScriptTeamAreaMembership> {
        use generals_sim::scripting::world::ScriptTeamAreaMembership;

        let team = self
            .world
            .resolve_owned_script_team(team, calling, condition)
            .expect("owned team area query");
        let Some(area_name) = self.world.qualified_script_area(area, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let Some((_, members)) = team else {
            return ScriptOwnerQuery::Missing;
        };
        let trigger_world = self
            .world
            .host_trigger_world
            .lock()
            .expect("owned team area polygon");
        let Some(trigger) = trigger_world.trigger_area_by_canonical_name(&area_name) else {
            return ScriptOwnerQuery::Missing;
        };

        let mut membership = ScriptTeamAreaMembership::default();
        let considered = ScriptSurfacesAllowed::from_bits_truncate(which_to_consider).locomotor_mask();
        for member_id in members {
            let Some(object) = self.world.host_object(ObjectId(member_id)) else {
                continue;
            };
            let surfaces = if object.has_ai_update_interface() {
                object.locomotor_surfaces
            } else {
                generals_sim::pathfinding::SURFACE_GROUND
            };
            if (surfaces & considered) == 0
                || Self::status(object).effectively_dead
                || object.is_kind_of(KindOf::Inert)
            {
                continue;
            }
            membership.any_considered = true;
            // C++ Object::isInside reads the tracked slot. The receiving host
            // trigger owner is projected at the early script boundary, with
            // movement updates retaining the same slot/frame semantics.
            if trigger_world.is_inside(member_id, &trigger) {
                membership.any_inside = true;
            } else {
                membership.any_outside = true;
            }
        }
        ScriptOwnerQuery::Present(membership)
    }

    fn skirmish_special_power_exists(&self, power_name: &str) -> ScriptOwnerQuery<bool> {
        let template_exists = match self.world.receiving_module_content.as_ref() {
            Some(content) => content
                .action_definitions()
                .powers
                .find_template(power_name)
                .is_some(),
            None => self.world.templates.values().any(|template| {
                template.special_power_modules.iter().any(|module| {
                    module
                        .special_power_template
                        .eq_ignore_ascii_case(power_name)
                })
            }),
        };
        ScriptOwnerQuery::Present(template_exists)
    }

    fn skirmish_special_power_readiness(
        &self,
        owner: u32,
        power_name: &str,
    ) -> ScriptOwnerQuery<ScriptSpecialPowerReadiness> {
        let Some(player) = self.world.get_player(owner) else {
            return ScriptOwnerQuery::Missing;
        };
        let mut earliest_ready_frame = None;
        for object in self
            .world
            .host_objects()
            .registered_values()
            .filter(|object| object.owner_player_id == Some(owner))
        {
            if object.status.under_construction || object.is_disabled() {
                continue;
            }
            if let Some(module) =
                object
                    .thing()
                    .template
                    .special_power_modules
                    .iter()
                    .find(|module| {
                        module
                            .special_power_template
                            .eq_ignore_ascii_case(power_name)
                    })
            {
                if module.required_science.as_deref().is_some_and(|science| {
                    !player.has_unlocked_science(science)
                }) {
                    continue;
                }
                let state_for_module = object.authored_special_power_timers.get(&module.source_index).copied().unwrap_or_default();
                let seconds = if module.shared_n_sync {
                    player.authored_special_power_timers
                        .get(&module.special_power_template.to_ascii_lowercase()).copied().unwrap_or(0.0)
                } else {
                    state_for_module.0
                };
                if seconds <= 0.0 && state_for_module.1 == 0 && !object.is_disabled() {
                    return ScriptOwnerQuery::Present(ScriptSpecialPowerReadiness::Ready);
                }
                let ready_frame = self
                    .world
                    .frame
                    .wrapping_add((seconds.max(0.0) * 30.0) as u32);
                earliest_ready_frame = Some(earliest_ready_frame.map_or(ready_frame, |earliest: u32| earliest.min(ready_frame)));
            }
        }
        ScriptOwnerQuery::Present(ScriptSpecialPowerReadiness::NotReady { earliest_ready_frame })
    }

    fn skirmish_census(&self, query: ScriptSkirmishCensus) -> ScriptOwnerQuery<i32> {
        let owner = match query {
            ScriptSkirmishCensus::NeutralUnmanned => self.world.get_players().values()
                .find(|player| player.team == Team::Neutral)
                .map(|player| player.id),
            ScriptSkirmishCensus::Garrisoned(owner) | ScriptSkirmishCensus::Captured(owner) => Some(owner),
        };
        let Some(owner) = owner else {
            return ScriptOwnerQuery::Missing;
        };
        let count = self
            .world
            .host_objects()
            .registered_values()
            .filter(|object| {
                object.owner_player_id == Some(owner)
                    || (matches!(query, ScriptSkirmishCensus::NeutralUnmanned)
                        && object.owner_player_id.is_none()
                        && object.team == Team::Neutral)
            })
            .filter(|object| match query {
                ScriptSkirmishCensus::NeutralUnmanned => object.status.disabled_unmanned,
                ScriptSkirmishCensus::Garrisoned(_) => {
                    object.is_garrison_contain() && !object.contained_units().is_empty()
                }
                ScriptSkirmishCensus::Captured(_) => object.status.private_captured,
            })
            .count() as i32;
        ScriptOwnerQuery::Present(count)
    }
    fn player_discovered_by(&self, owner: u32, viewer: u32) -> ScriptOwnerQuery<bool> {
        let shroud = self
            .world
            .world_services
            .shroud()
            .lock()
            .expect("owned player discovery visibility");
        let found = self
            .world
            .host_objects()
            .registered_values()
            .filter(|object| object.owner_player_id == Some(owner))
            .any(|object| {
                matches!(
                    shroud.get_host_object_shroud_status(viewer, object.id.0),
                    Some(
                        generals_sim::object::ObjectShroudStatus::Clear
                            | generals_sim::object::ObjectShroudStatus::PartialClear
                    )
                )
            });
        ScriptOwnerQuery::Present(found)
    }

    fn player_was_attacked(&self, player: u32, attacker: u32) -> bool {
        self.world
            .get_player(player)
            .is_some_and(|player| player.get_attacked_by(attacker as i32))
    }

    fn player_outside_area_preflight(
        &mut self,
        player: &str,
        player_mask: u32,
        area: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<(u32, bool)> {
        let Some(owner) = self
            .query_condition_player_id(player, player_mask, current_player)
            .filter(|id| self.world.players.contains_key(id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        let area_exists = self.world.qualified_script_area(area, current_player)
            .is_some_and(|name| self.world.has_qualified_script_area(&name));
        ScriptOwnerQuery::Present((1u32.checked_shl(owner).unwrap_or(0), area_exists))
    }

    fn player_area_value(
        &mut self,
        player: &str,
        player_mask: u32,
        area: &str,
        current_player: Option<&str>,
        query: ScriptPlayerAreaQuery<'_>,
    ) -> ScriptOwnerQuery<generals_sim::scripting::world::ScriptPlayerAreaCensus> {
        use generals_sim::scripting::world::ScriptPlayerAreaCensus;

        let resolve_player = |driver: &mut Self| {
            driver
                .query_condition_player_id(player, player_mask, current_player)
                .filter(|id| driver.world.players.contains_key(id))
        };
        let mask_for = |id| 1u32.checked_shl(id).unwrap_or(0);
        // CPP2088 resolves SIDE first for value. CPP420/513/2472 need
        // a live polygon before they can resolve SIDE.
        let value_owner = if matches!(query, ScriptPlayerAreaQuery::Value) {
            match resolve_player(self) {
                Some(owner) => Some(owner),
                None => return ScriptOwnerQuery::Present(ScriptPlayerAreaCensus::MissingPlayer),
            }
        } else {
            None
        };
        let Some(area) = self.world.qualified_script_area(area, current_player) else {
            return match value_owner {
                Some(owner) => ScriptOwnerQuery::Present(ScriptPlayerAreaCensus::MissingArea {
                    player_mask: mask_for(owner),
                }),
                None => ScriptOwnerQuery::Missing,
            };
        };
        if value_owner.is_none() && !self.world.has_qualified_script_area(&area) {
            return ScriptOwnerQuery::Missing;
        }
        let Some(owner) = value_owner.or_else(|| resolve_player(self)) else {
            return ScriptOwnerQuery::Present(ScriptPlayerAreaCensus::MissingPlayer);
        };
        let player_mask = mask_for(owner);
        let geometry = self
            .world
            .host_trigger_world
            .lock()
            .expect("owned player polygon census");
        let Some(trigger) = geometry.trigger_area_by_canonical_name(&area) else {
            return ScriptOwnerQuery::Present(ScriptPlayerAreaCensus::MissingArea { player_mask });
        };
        let mut value = 0i32;
        let mut changed = false;
        for object in self
            .world
            .host_objects()
            .registered_values()
            .filter(|object| object.owner_player_id == Some(owner))
        {
            changed |= geometry.did_enter_or_exit(object.id.0, self.world.frame);
            let position = object.get_position();
            let point = glam::IVec3::new(
                position.x as i32,
                position.z as i32,
                position.y as i32,
            );
            if !trigger.point_in_trigger_int(&point) {
                continue;
            }
            let dead = Self::status(object).effectively_dead;
            let inert = object.is_kind_of(KindOf::Inert);
            let contribution = match &query {
                ScriptPlayerAreaQuery::Templates(names) => {
                    if (!dead && !inert || object.is_kind_of(KindOf::Crate))
                        && names.contains(&object.template_name)
                    {
                        1
                    } else {
                        0
                    }
                }
                ScriptPlayerAreaQuery::Kind(kind) => {
                    let mask = crate::game_logic::host_radar_stealth_vision_residual::live_object_kind_of_cpp_mask(object.thing().template.kind_of.iter());
                    let matches = generals_sim::kind_of::classic::kind_of_indices(*kind)
                        .iter()
                        .any(|index| mask & (1u128 << index) != 0);
                    if !dead && !inert && matches { 1 } else { 0 }
                }
                ScriptPlayerAreaQuery::Units => {
                    if !dead && !inert && !object.is_kind_of(KindOf::Projectile) {
                        1
                    } else {
                        0
                    }
                }
                ScriptPlayerAreaQuery::Value => {
                    if !dead && !inert {
                        object.thing().template.build_cost.supplies as i32
                    } else {
                        0
                    }
                }
            };
            value = value.wrapping_add(contribution);
        }
        ScriptOwnerQuery::Present(ScriptPlayerAreaCensus::Measured {
            player_mask,
            value,
            membership_changed: changed,
        })
    }

    fn named_query_fact(
        &self,
        name: &str,
        this_object: Option<u32>,
    ) -> ScriptOwnerQuery<(u32, generals_sim::scripting::world::HostScriptQueryObject)> {
        self.world
            .owned_named_script_object(name, this_object.map(ObjectId))
            .and_then(|id| {
                self.world.host_object(id).map(|object| {
                    (
                        self.world.frame,
                        self.world.receiving_script_query_object(id, object),
                    )
                })
            })
            .map_or(ScriptOwnerQuery::Missing, ScriptOwnerQuery::Present)
    }
    fn receiving_player_query_name(
        &self,
        name: &str,
        current: Option<&str>,
    ) -> ScriptOwnerQuery<String> {
        self.script_player_id(name, current)
            .and_then(|id| self.world.player_name(id))
            .map_or(ScriptOwnerQuery::Missing, ScriptOwnerQuery::Present)
    }

    fn named_selection(
        &self,
        literal_name: &str,
        _this_object: Option<u32>,
    ) -> ScriptOwnerQuery<generals_sim::scripting::world::ScriptSelectionStatus> {
        ScriptOwnerQuery::Present(generals_sim::scripting::world::ScriptSelectionStatus {
            multiplayer: self.world.isInMultiplayerGame(),
            changed_frame: self.world.selection_changed_frame,
            selected: self.world.selected_objects.iter().any(|id| {
                self.world.host_object(*id).is_some_and(|object| object.name == literal_name)
            }),
        })
    }

    fn named_inside_area(
        &mut self,
        unit: &str,
        area: &str,
        current_player: Option<&str>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> ScriptOwnerQuery<bool> {
        let Some(pos) = self
            .world
            .owned_named_script_object(unit, this_object.map(ObjectId))
            .and_then(|id| self.world.host_object(id))
            .map(|object| object.get_position())
        else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(area) = self.world.qualified_script_area(area, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let world = self
            .world
            .host_trigger_world
            .lock()
            .expect("owned named polygon query");
        let Some(trigger) = world.trigger_area_by_canonical_name(&area) else {
            return ScriptOwnerQuery::Missing;
        };
        // CPP current float position truncates to integer; Main is Y-up/XZ.
        // Inside deliberately includes dead/inert objects, unlike Entered.
        let point = glam::IVec3::new(pos.x as i32, pos.z as i32, pos.y as i32);
        ScriptOwnerQuery::Present(trigger.point_in_trigger_int(&point))
    }

    fn named_entered_area(
        &mut self,
        unit: &str,
        area: &str,
        current_player: Option<&str>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> ScriptOwnerQuery<bool> {
        let Some(object) = self
            .world
            .owned_named_script_object(unit, this_object.map(ObjectId))
            .and_then(|id| self.world.host_object(id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        if object.is_kind_of(KindOf::Inert) {
            return ScriptOwnerQuery::Present(false);
        }
        let id = object.id;
        let Some(area) = self.world.qualified_script_area(area, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let world = self
            .world
            .host_trigger_world
            .lock()
            .expect("owned named polygon entry");
        let Some(trigger) = world.trigger_area_by_canonical_name(&area) else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present(gamelogic::scripting::host_object_did_enter(
            &world,
            id.0,
            &trigger,
            self.world.frame,
        ))
    }

    fn named_exited_area(
        &mut self,
        unit: &str,
        area: &str,
        current_player: Option<&str>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> ScriptOwnerQuery<bool> {
        let Some(id) = self
            .world
            .owned_named_script_object(unit, this_object.map(ObjectId))
            .filter(|id| self.world.host_object(*id).is_some())
        else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(area) = self.world.qualified_script_area(area, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let world = self
            .world
            .host_trigger_world
            .lock()
            .expect("owned named polygon exit");
        let Some(trigger) = world.trigger_area_by_canonical_name(&area) else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present(gamelogic::scripting::host_object_did_exit(
            &world,
            id.0,
            &trigger,
            self.world.frame,
        ))
    }

    fn object_count_changed_frame(&self) -> ScriptOwnerQuery<u32> {
        ScriptOwnerQuery::Present(self.world.script_object_count_changed_frame)
    }
    fn scoring_enabled(&mut self, enabled: bool) -> ScriptOwnerQuery<()> {
        self.world.set_scoring_enabled(enabled);
        ScriptOwnerQuery::Present(())
    }

    fn eva_enabled(&mut self, enabled: bool) -> ScriptOwnerQuery<()> {
        self.world.set_eva_enabled(enabled);
        ScriptOwnerQuery::Present(())
    }

    fn building_entered_by_player(
        &self,
        player: &str,
        building: &str,
        current_player: Option<&str>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> ScriptOwnerQuery<bool> {
        let Some(object) = self
            .world
            .owned_named_script_object(building, this_object.map(ObjectId))
            .and_then(|id| self.world.host_object(id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        if object.get_template().contain_module.kind == crate::game_logic::ContainModuleKind::None {
            return ScriptOwnerQuery::Present(false);
        }
        let Some(player_id) = self.script_player_id(player, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present(object.player_who_entered == Some(player_id))
    }

    fn sighting_observer(
        &self,
        unit: &str,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> ScriptOwnerQuery<u32> {
        self.world
            .owned_named_script_object(unit, this_object.map(ObjectId))
            .filter(|id| self.world.host_object(*id).is_some())
            .map_or(ScriptOwnerQuery::Missing, |id| ScriptOwnerQuery::Present(id.0))
    }

    fn any_sighted_object(
        &self,
        observer: u32,
        player: u32,
        filter: ScriptSightingFilter<'_>,
    ) -> bool {
        let Some(looker) = self.world.host_object(ObjectId(observer))
        else {
            return false;
        };
        let observer = ScriptSightingObserver {
            position: looker.get_position(),
            vision_range: looker.vision_range,
            off_map: looker.status.off_map,
        };
        // CPP filters candidate life, not the looking object's life.
        self.world.host_objects().registered_values().any(|candidate| {
            candidate.id != looker.id
                && !candidate.status.effectively_dead
                && self.is_in_script_partition(candidate)
                && filter.accepts(
                    observer,
                    ScriptSightingCandidate {
                        position: candidate.get_position(),
                        off_map: candidate.status.off_map,
                        effectively_stealthed: candidate.is_effectively_stealthed(),
                        owner: candidate.owner_player_id,
                        template: &candidate.template_name,
                    },
                    player,
                    || self.world.script_sighting_relationship(looker, candidate),
                )
        })
    }

    fn skirmish_player_can_build_any(
        &mut self,
        player: &str,
        mask: u32,
        object_types: &[String],
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<(u32, bool)> {
        if self.uses_legacy_network_player_bindings() {
            return ScriptOwnerQuery::Unavailable;
        }
        let Some(player) = self
            .query_condition_player_id(player, mask, current_player)
            .and_then(|id| self.world.players.get(&id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present((
            1u32.checked_shl(player.id).unwrap_or(0),
            object_types
                .iter()
                .any(|name| self.world.script_player_can_build_template(player, name)),
        ))
    }

    fn player_construction(
        &mut self,
        request: &ScriptPlayerConstructionRequest,
        current_player: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let player_token = match request {
            ScriptPlayerConstructionRequest::Units { player, .. }
            | ScriptPlayerConstructionRequest::Base { player, .. }
            | ScriptPlayerConstructionRequest::Factories { player, .. } => player.as_str(),
        };
        let Some(player_id) = self.script_player_id(player_token, current_player) else {
            return Some(Ok(()));
        };
        match request {
            ScriptPlayerConstructionRequest::Units { enabled, .. } => {
                if let Some(player) = self.world.players.get_mut(&player_id) {
                    player.set_can_build_units(*enabled);
                }
            }
            ScriptPlayerConstructionRequest::Base { enabled, .. } => {
                if let Some(player) = self.world.players.get_mut(&player_id) {
                    player.set_can_build_base(*enabled);
                }
            }
            ScriptPlayerConstructionRequest::Factories {
                template, enabled, ..
            } => {
                self.world
                    .host_set_objects_enabled(player_id, template, *enabled, &mut self.ui);
            }
        }
        Some(Ok(()))
    }

    fn player_acquired_science_source(
        &mut self,
        player: &str,
        player_mask: u32,
        science: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<(usize, Option<ScienceType>)> {
        let Some(id) = self.query_condition_player_id(player, player_mask, current_player)
            .filter(|id| self.world.players.contains_key(id)) else {
            return ScriptOwnerQuery::Missing;
        };
        let science = self.world.receiving_module_content.as_ref()
            .and_then(|content| content.science_facts_ref().science(science));
        ScriptOwnerQuery::Present((id as usize, science))
    }

    fn player_can_purchase_science(&self, player: u32, science: &str) -> ScriptOwnerQuery<bool> {
        let Some((player, content)) = self.world.get_player(player)
            .zip(self.world.receiving_module_content.as_ref()) else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present(player.science_purchase_cost_with_facts(
            science, content.science_facts_ref(),
        ).is_some())
    }

    fn player_science_action(
        &mut self,
        player: &str,
        science: &str,
        grant: bool,
        current_player: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        use game_engine::common::message_stream::ReceivingRankScienceUi;

        let Some((player, _, _)) = self.script_science_source(player, 0, science, current_player)
        else {
            return Some(Ok(()));
        };
        let player = player.id;
        let is_local = self.world.local_player_id() == Some(player);
        let ui = &mut self.ui.ui;
        let view = &mut self.ui.view;
        let mut callback_error = None;
        self.world.apply_receiving_science_action(
            player,
            science,
            grant,
            self.interpreter,
            &mut |event| {
                if is_local || event == ReceivingRankScienceUi::ScienceAdded {
                    if let Some(ui) = ui.as_deref_mut() {
                        ui.rank_science_event(event);
                    } else if let Some(view) = view.as_deref_mut()
                        && let Err(error) = view.admission_rank_event(event)
                    {
                        callback_error.get_or_insert(error);
                    }
                }
            },
        );
        Some(match callback_error {
            Some(error) => Err(generals_sim::GameLogicError::Configuration(error)),
            None => Ok(()),
        })
    }

    fn player_science_availability(
        &mut self,
        player: &str,
        science: &str,
        availability: generals_sim::science::ScienceAvailabilityType,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<()> {
        let Some((player, _, _)) = self.script_science_source(player, 0, science, current_player)
        else {
            return ScriptOwnerQuery::Missing;
        };
        let player_id = player.id;
        let Some(player) = self.world.get_player_mut(player_id) else {
            return ScriptOwnerQuery::Missing;
        };
        player.apply_science_availability(science, availability);
        ScriptOwnerQuery::Present(())
    }

    fn player_rank(
        &mut self,
        request: &ScriptPlayerRankRequest,
        current_player: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        use ScriptPlayerRankRequest as Request;
        use game_engine::common::message_stream::{
            ReceivingRankScienceAction as Action, ReceivingRankScienceUi,
        };
        use generals_sim::GameLogicError;

        let token = match request {
            Request::SetRankLevelLimit { limit } => {
                self.world.set_rank_level_limit(*limit);
                return Some(Ok(()));
            }
            Request::AddSkillPoints { player, .. }
            | Request::AddRankLevel { player, .. }
            | Request::SetRankLevel { player, .. }
            | Request::AffectReceivingExperience { player, .. } => player,
        };
        let Some(id) = self.script_player_id(token, current_player) else {
            return Some(Ok(()));
        };
        let Some(player) = self.world.players.get_mut(&id) else {
            return Some(Ok(()));
        };
        let current = player.rank_level.max(1);
        let action = match request {
            Request::AddSkillPoints { delta, .. } => Action::AddSkillPoints(*delta),
            Request::AddRankLevel { delta, .. } => {
                let rank = (i64::from(current) + i64::from(*delta)).clamp(1, i64::from(u32::MAX));
                Action::SetRank(rank as u32)
            }
            Request::SetRankLevel { level, .. } => Action::SetRank((*level).max(1) as u32),
            Request::AffectReceivingExperience { modifier, .. } => {
                player.skill_points_modifier = *modifier;
                return Some(Ok(()));
            }
            Request::SetRankLevelLimit { .. } => unreachable!("handled without a player"),
        };
        let mut general_name = None;
        if let Action::SetRank(requested) = action {
            let count = self
                .world
                .world_services
                .common()
                .ini
                .rank
                .read()
                .get_rank_level_count();
            let target = requested
                .min(self.world.rank_level_limit() as u32)
                .min(count as u32);
            if count > 0 && target < current {
                let fetched = if let Some(ui) = self.ui.ui.as_deref_mut() {
                    ui.game_text("SCIENCE:GeneralName")
                } else if let Some(view) = self.ui.view.as_deref_mut() {
                    view.admission_rank_text("SCIENCE:GeneralName")
                        .map_err(GameLogicError::Configuration)
                } else {
                    Err(GameLogicError::Configuration(
                        "script rank reset requires receiving GameText".into(),
                    ))
                };
                match fetched {
                    Ok(name) => general_name = Some(name),
                    Err(error) => return Some(Err(error)),
                }
            }
        }

        let is_local = self.world.local_player_id() == Some(id);
        let ui = &mut self.ui.ui;
        let view = &mut self.ui.view;
        let mut callback_error = None;
        let result = self.world.apply_receiving_rank_science(
            id,
            action,
            general_name,
            self.interpreter,
            &mut |event| {
                if is_local || event == ReceivingRankScienceUi::ScienceAdded {
                    if let Some(ui) = ui.as_deref_mut() {
                        ui.rank_science_event(event);
                    } else if let Some(view) = view.as_deref_mut() {
                        if let Err(error) = view.admission_rank_event(event) {
                            callback_error.get_or_insert(error);
                        }
                    }
                }
            },
        );
        Some(
            callback_error
                .map_or(result, Err)
                .map_err(GameLogicError::Configuration),
        )
    }

    fn script_event_player_slot(
        &mut self,
        player: &str,
        mask: u32,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<usize> {
        if self.uses_legacy_network_player_bindings() {
            return ScriptOwnerQuery::Unavailable;
        }
        // The installed offline script list and notifications use the host ID.
        match self.condition_player_id(player, mask, current_player) {
            ScriptOwnerQuery::Present(id) => ScriptOwnerQuery::Present(id as usize),
            ScriptOwnerQuery::Missing => ScriptOwnerQuery::Missing,
            ScriptOwnerQuery::Unavailable => ScriptOwnerQuery::Unavailable,
        }
    }

    fn team_guard(
        &mut self,
        team: &str,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_guard(team, calling_team, condition_team),
        )
    }

    fn team_idle(
        &mut self,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<bool>> {
        Some(
            self.world
                .apply_owned_team_idle(team, calling, condition, &mut self.ui),
        )
    }

    fn team_guard_supply_center(
        &mut self,
        team: &str,
        supplies: i32,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_guard_supply_center(team, supplies, calling, condition),
        )
    }

    fn team_guard_tunnel(
        &mut self,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_guard_tunnel(team, calling, condition),
        )
    }

    fn player_units_idle(&mut self, idle: bool) -> Option<generals_sim::GameLogicResult<()>> {
        self.world.apply_owned_player_units_idle(idle);
        Some(Ok(()))
    }

    fn team_stop(
        &mut self,
        team: &str,
        calling: Option<gamelogic::team::TeamID>,
        condition: Option<gamelogic::team::TeamID>,
        disband: bool,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_team_stop_with_effects(
            team,
            calling,
            condition,
            disband,
            &mut self.ui,
        ))
    }

    fn team_attitude(
        &mut self,
        team: &str,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
        mood: i32,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_attitude(team, calling_team, condition_team, mood),
        )
    }

    fn team_for_sequence(
        &mut self,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
        starting: bool,
    ) -> Option<generals_sim::GameLogicResult<Option<u32>>> {
        Some((|| {
            let Some((id, members)) =
                self.world.resolve_owned_script_team(team, calling, condition)?
            else {
                return Ok(None);
            };
            if starting {
                // ScriptActions.cpp:4746 forms an AIGroup and idles it before
                // append, so readiness and later group commands see this roster.
                let group = self.world.create_team_ai_group(&members);
                for member in self.world.ai_group_members(group) {
                    self.world.apply_owned_script_idle(member);
                }
            }
            Ok(Some(id))
        })())
    }

    fn nearest_object_move(
        &mut self,
        request: generals_sim::scripting::world::ScriptNearestObjectRequest<'_>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_nearest_object_move(request))
    }

    fn named_attitude(
        &mut self,
        unit: &str,
        this_object: Option<gamelogic::common::ObjectID>,
        mood: i32,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_named_attitude(unit, this_object.map(ObjectId), mood))
    }

    fn force_select_object(
        &mut self,
        request: &generals_sim::scripting::object_control_actions::ForceSelection<'_>,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.receive_force_selection(request, calling_team, condition_team))
    }

    fn named_attack_priority(
        &mut self,
        name: &str,
        set: &str,
        this_object: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.receive_named_attack_priority(name, set, this_object)
    }

    fn named_evac_disposition(
        &mut self,
        name: &str,
        disposition: i32,
        this_object: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.receive_named_evac_disposition(name, disposition, this_object)
    }

    fn named_panel_flag(
        &mut self,
        name: &str,
        flag: ObjectPanelFlag,
        enabled: bool,
        this_object: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.receive_named_panel_flag(name, flag, enabled, this_object)
    }

    fn team_panel_flag(
        &mut self,
        team: &str,
        flag: ObjectPanelFlag,
        enabled: bool,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.receive_team_panel_flag(team, flag, enabled, calling, condition))
    }

    fn named_warehouse_value(
        &mut self,
        name: &str,
        cash: i32,
        this_object: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.receive_named_warehouse_value(name, cash, this_object)
    }

    fn named_runtime(
        &mut self,
        request: generals_sim::scripting::world::ScriptNamedRuntimeRequest<'_>,
        this_object: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.receive_named_runtime(request, this_object)
    }

    fn garrison_request(
        &mut self,
        request: generals_sim::scripting::world::HostScriptGarrisonEnterExitRequest,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.receive_garrison_request(request, this_object, calling, condition)
    }

    fn team_exit_all(
        &mut self,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_receiving_team_exit_all(team, calling, condition, &mut self.ui),
        )
    }

    fn named_special_power(
        &mut self,
        request: &generals_sim::scripting::object_control_actions::NamedSpecialPower<'_>,
        this_object: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.receive_named_special_power(request, this_object)
    }

    fn command_button_request(
        &mut self,
        request: &generals_sim::scripting::command_button_actions::CommandButtonAction<'_>,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.receive_command_button_request(request, this_object, calling, condition)
    }

    fn team_face(
        &mut self,
        team: &str,
        target: generals_sim::scripting::navigation_actions::TeamFaceTarget<'_>,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.receive_team_face(team, target, this_object, calling, condition))
    }

    fn named_command(
        &mut self,
        request: ScriptNamedCommand<'_>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.apply_owned_named_script_command(
            request,
            this_object.map(ObjectId),
            &mut self.ui,
        ))
    }

    fn condition_player_id(
        &mut self,
        player: &str,
        mask: u32,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<u32> {
        self.query_condition_player_id(player, mask, current_player)
            .filter(|id| self.world.players.contains_key(id))
            .map_or(ScriptOwnerQuery::Missing, ScriptOwnerQuery::Present)
    }

    fn player_faction(&self, player_id: u32) -> Option<&str> {
        use crate::game_logic::host_faction_skirmish_residual::{
            SIDE_AMERICA, SIDE_CHINA, SIDE_CIVILIAN, SIDE_GLA, SIDE_OBSERVER,
        };

        let player = self.world.get_player(player_id)?;
        if let Some(identity) = self.world.player_template_identity(player_id) {
            let template = identity.resolve_in(self.world.player_template_definitions.as_ref()?)?;
            return Some(template.get_side());
        }
        Some(if player.is_observer {
            SIDE_OBSERVER
        } else {
            match player.team {
                Team::USA => SIDE_AMERICA,
                Team::China => SIDE_CHINA,
                Team::GLA => SIDE_GLA,
                Team::Neutral => SIDE_CIVILIAN,
            }
        })
    }

    fn player_start_index(&self, player: u32) -> Option<i32> {
        self.world.get_player(player).map(|player| player.start_position)
    }

    fn script_area_exists(&mut self, area: &str, current_player: Option<&str>) -> Option<bool> {
        Some(
            self.world
                .qualified_script_area(area, current_player)
                .is_some_and(|name| self.world.has_qualified_script_area(&name)),
        )
    }

    fn supplies_value_in_perimeter(
        &mut self,
        owner: u32,
        distance: f32,
        area: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<f32> {
        let Some(area) = self.world.qualified_script_area(area, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let (bounds, radius) = {
            let triggers = self.world.host_trigger_world.lock().expect("owned script trigger query");
            let Some(trigger) = triggers.trigger_area_by_canonical_name(&area) else {
                return ScriptOwnerQuery::Missing;
            };
            (trigger.get_bounds(), trigger.get_radius() + distance)
        };
        let center = glam::Vec2::new(
            (bounds.lo.x + bounds.hi.x) as f32 / 2.0,
            (bounds.lo.y + bounds.hi.y) as f32 / 2.0,
        );
        let radius_squared = radius * radius;
        let price = self.world.supply_box_value() as u32;
        let max_value = self.world.host_objects().registered_values()
            .filter(|object| {
                if object.status.off_map
                    || !object.is_kind_of(KindOf::Structure)
                    || object.thing().template.dock_kind != crate::game_logic::DockKind::SupplyWarehouse
                {
                    return false;
                }
                let position = object.get_position();
                glam::Vec2::new(position.x, position.z).distance_squared(center) < radius_squared
                    && (self.world.player_owner_for_host_object(object) == Some(owner)
                        || GameLogic::object_relationship_from_owner_to_team_identity(
                            &self.world.team_factory,
                            &self.world.players,
                            Some(owner),
                            object.owner_player_id,
                            object.team_instance_id,
                            &object.team_instance_name,
                        ) == game_engine::common::game_common::Relationship::Neutral)
            })
            .map(|object| price.wrapping_mul(object.warehouse_boxes() as u32))
            .max()
            .unwrap_or(0);
        ScriptOwnerQuery::Present(max_value as f32)
    }

    fn tech_building_within_distance(
        &mut self,
        player_id: u32,
        distance: f32,
        area: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<bool> {
        use game_engine::common::game_common::Relationship;
        let Some(area) = self.world.qualified_script_area(area, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(player) = self.world.players.get(&player_id) else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(trigger) = self
            .world
            .host_trigger_world
            .lock()
            .expect("owned script trigger query")
            .trigger_area_by_canonical_name(&area)
        else {
            return ScriptOwnerQuery::Missing;
        };
        // Avoid get_center_point's ambient terrain-height lookup. FROM_CENTER_2D
        // uses only the polygon bounds; get_radius preserves CPP's formula.
        let bounds = trigger.get_bounds();
        let center_x = (bounds.lo.x + bounds.hi.x) as f32 / 2.0;
        let center_z = (bounds.lo.y + bounds.hi.y) as f32 / 2.0;
        let radius = trigger.get_radius() + distance;
        let radius_sq = radius * radius;
        let found = self.world.host_objects().registered_values().any(|object| {
            let position = object.get_position();
            if !object.is_kind_of(KindOf::TechBuilding)
                || object.owner_player_id == Some(player.id)
                || position.x < self.world.world_min.x
                || position.x > self.world.world_max.x
                || position.z < self.world.world_min.z
                || position.z > self.world.world_max.z
            {
                return false;
            }
            let team = if object.team_instance_name.is_empty() {
                self.world
                    .default_host_team_instance_name(object.owner_player_id, object.team)
            } else {
                object.team_instance_name.clone()
            };
            let relationship = GameLogic::team_id_for_relationship(
                &self.world.team_factory,
                object.team_instance_id,
                &team,
            )
            .and_then(|id| player.team_relationship_override(id))
            .unwrap_or_else(|| {
                let Some(target) = object
                    .owner_player_id
                    .and_then(|id| self.world.players.get(&id))
                else {
                    return Relationship::Neutral;
                };
                // CPP Player542 reads relationships even for defeated players.
                player.map_relationship(target.id).unwrap_or_else(|| {
                    if player.alliance_team >= 0 && player.alliance_team == target.alliance_team {
                        Relationship::Allies
                    } else if player.alliance_team >= 0 && target.alliance_team >= 0 {
                        Relationship::Enemies
                    } else {
                        Relationship::Neutral
                    }
                })
            });
            if relationship == Relationship::Allies {
                return false;
            }
            let dx = position.x - center_x;
            let dz = position.z - center_z;
            dx * dx + dz * dz < radius_sq
        });
        ScriptOwnerQuery::Present(found)
    }

    fn named_upgrade(
        &mut self,
        name: &str,
        upgrade: &str,
        this_object: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let Some(id) = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
        else {
            return Some(Ok(()));
        };
        // CPP ScriptActions5322: a missing template is an authoritative no-op.
        // Resolve only through this World's admitted UpgradeCenter; a peer or
        // standalone Core definition cannot authorize this action.
        let Some(definition) = self.world.upgrade_template(upgrade) else {
            return Some(Ok(()));
        };
        // The active action driver's interpreter remains retained throughout
        // this synchronous callback. Existing callback effects receive this
        // exact operation loan; no host queue/default effects or engine lookup.
        Some(self.world.apply_upgrade_to_object_with_interpreter_checked(
            id,
            definition.get_name().as_str(),
            self.interpreter,
            &mut self.ui,
        ))
    }

    fn named_special_power_countdown(
        &mut self,
        name: &str,
        power_name: &str,
        action: generals_sim::scripting::program::ScriptActionType,
        seconds: i32,
        this_object: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.receive_named_special_power_countdown(name, power_name, action, seconds, this_object)
    }

    fn water(
        &mut self,
        request: ScriptWaterRequest<'_>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_script_water_with_effects(request, &mut self.ui),
        )
    }

    fn radar_visibility(&mut self, enabled: bool) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.apply_script_radar_visibility(enabled))
    }

    fn waypoint_visibility(
        &mut self,
        operation: generals_sim::scripting::map_reveals::WaypointVisibility,
        waypoint: &str,
        radius: f32,
        player: &str,
        current_player: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.apply_receiving_waypoint_visibility(
            operation, waypoint, radius, player, current_player,
        ))
    }

    fn named_map_visibility(
        &mut self,
        operation: generals_sim::scripting::map_reveals::NamedRevealOperation,
        reveal: &generals_sim::scripting::map_reveals::NamedReveal,
        current_player: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.apply_receiving_named_reveal(operation, reveal, current_player))
    }

    fn map_visibility_all(
        &mut self,
        operation: generals_sim::scripting::world::ScriptMapVisibilityOperation,
        player_name: &str,
        current_player: Option<&str>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.apply_script_map_visibility(operation, player_name, current_player))
    }

    fn ai_player(
        &mut self,
        request: ScriptAiPlayerRequest<'_>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        match request {
            ScriptAiPlayerRequest::BuildUpgrade { player, upgrade } => {
                if let Some(id) = self.world.host_player_id_for_script_token(player) {
                    self.world.build_owned_script_ai_upgrade(id, upgrade);
                }
            }
            ScriptAiPlayerRequest::BuildNearest {
                player,
                building,
                team,
                calling,
                condition,
            } => {
                return Some(self.world.apply_owned_script_build_nearest(
                    player, building, team, calling, condition, &mut self.ui,
                ));
            }
            ScriptAiPlayerRequest::BaseDefense {
                player,
                structure,
                flank,
            } => {
                self.world
                    .apply_owned_script_base_defense(player, structure, flank);
            }
            ScriptAiPlayerRequest::FireSpecialPower { player, power } => {
                let mut manager = std::mem::take(&mut self.world.ai_manager);
                manager.fire_skirmish_special_power_at_most_cost(self.world, player, power);
                self.world.ai_manager = manager;
            }
            ScriptAiPlayerRequest::BuildTeam { team } => {
                self.world.apply_owned_script_team_build_with_effects(
                    team,
                    None,
                    self.interpreter,
                    &mut self.ui,
                )
            }
            ScriptAiPlayerRequest::RecruitTeam { team, radius } => {
                self.world.apply_owned_script_team_build_with_effects(
                    team,
                    Some(radius),
                    self.interpreter,
                    &mut self.ui,
                )
            }
            ScriptAiPlayerRequest::BuildSupplyCenter {
                player,
                building,
                cash,
                current_player,
            } => {
                let ordinary = self
                    .world
                    .host_player_id_for_script_token(player)
                    .filter(|id| {
                        self.world.ai_manager.ai_players.get(id)
                            .is_some_and(|ai| ai.ordinary_timing.is_some())
                    });
                if let Some(id) = ordinary {
                    let Some(build) = self.world.prepare_ordinary_script_supply_build(id, building, cash)
                    else {
                        return Some(Ok(()));
                    };
                    let enemy = if build.toward_enemy {
                        self.supply_build_script_enemy(current_player)
                    } else {
                        None
                    };
                    self.world.admit_ordinary_script_supply_build(
                        id, building, build, enemy, &mut self.ui,
                    );
                    return Some(Ok(()));
                }
                self.world
                    .apply_owned_script_supply_build(player, building, cash);
            }
            ScriptAiPlayerRequest::RepairNamed { player, structure } => {
                self.world.apply_owned_script_ai_repair(player, structure);
            }
            ScriptAiPlayerRequest::SelectSkillset {
                player,
                script_skillset,
            } => {
                if let Some(pid) = self.world.host_player_id_for_script_token(player) {
                    if let Some(ai) = self.world.ai_manager.ai_players.get_mut(&pid) {
                        // CPP doAffectPlayerSkillset decrements exactly once.
                        ai.select_skillset(script_skillset - 1);
                    }
                }
            }
            ScriptAiPlayerRequest::SetTeamDelay { player, seconds } => {
                if let Some(pid) = self.world.host_player_id_for_script_token(player) {
                    if let Some(ai) = self.world.ai_manager.ai_players.get_mut(&pid) {
                        ai.set_team_delay_seconds(seconds);
                    }
                }
            }
        }
        Some(Ok(()))
    }

    fn script_waypoint_position(
        &mut self,
        name: &str,
    ) -> Option<generals_sim::GameLogicResult<Option<glam::Vec3>>> {
        Some(self.receiving_map_waypoint(name))
    }

    fn camera_movement_finished(&mut self) -> Option<bool> {
        Some(self.world.camera_movement_finished())
    }

    fn world(
        &mut self,
        request: generals_sim::scripting::world::ScriptWorldRequest<'_>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        use generals_sim::scripting::map_actions::ScriptMapWorld;
        use generals_sim::scripting::media_actions::ScriptMediaWorld;
        use generals_sim::scripting::mission_actions::{ScriptMissionWorld, ScriptUiRequest as Ui};
        use generals_sim::scripting::world::ScriptWorldRequest as Request;

        Some(match request {
            Request::MapSwitchBoundary(_) | Request::RadarRefreshTerrain | Request::RadarForced(_) => {
                ScriptMapWorld::world(self, request)
            }
            Request::Music { track, fade_out, fade_in } => self.music(track, fade_out, fade_in),
            Request::MovieFullscreen(name) => self.movie(name, true),
            Request::Freeze(frozen) => self.apply_script_ui(Ui::Freeze(frozen)),
            Request::VisualSpeed(multiplier) => self.apply_script_ui(Ui::VisualSpeed(multiplier)),
            Request::Weather(visible) => self.apply_script_ui(Ui::Weather(visible)),
            Request::Input { enabled } => self.apply_script_ui(Ui::Input(enabled)),
            Request::Timer { name, text, countdown } => {
                self.apply_script_ui(Ui::AddTimer { name, text, countdown })
            }
        })
    }

    fn camera(
        &mut self,
        request: ScriptCameraRequest<'_>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.apply_script_camera(request))
    }

    fn display(
        &mut self,
        request: ScriptDisplayRequest<'_>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.apply_script_display(request))
    }

    fn create_reinforcements(
        &mut self,
        team: &str,
        waypoint: &str,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.world
            .host_script_create_reinforcement_team_with_effects(
                team,
                waypoint,
                self.interpreter,
                &mut self.ui,
            );
        Some(Ok(()))
    }

    fn merge_team(
        &mut self,
        source: &str,
        destination: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some(self.world.merge_owned_script_team_with_effects(
            source,
            destination,
            calling,
            condition,
            self.interpreter,
            &mut self.ui,
        ))
    }

    fn after_action(&mut self) -> generals_sim::GameLogicResult<()> {
        self.world
            .apply_script_action_requests_with_effects(self.interpreter, self.ui.reborrow())?;
        if let Some(ui) = self.ui.ui.as_deref_mut() {
            ui.flush_window_callbacks()?;
        }
        #[cfg(test)]
        if let Some(observer) = self.after_action_observer.as_deref_mut() {
            observer(self.world);
        }
        Ok(())
    }

    fn literal_template_exists(&self, name: &str) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Present(
            self.world
                .templates
                .values()
                .any(|template| template.name == name),
        )
    }

    fn priority_templates_with_kind(
        &self,
        kind: game_engine::common::system::kind_of::KindOfMask,
    ) -> ScriptOwnerQuery<Vec<String>> {
        use crate::game_logic::host_radar_stealth_vision_residual::kind_of_set_cpp_mask;

        ScriptOwnerQuery::Present(self.world.templates.values()
            .filter(|template| kind_of_set_cpp_mask(&template.kind_of) & kind.bits() != 0)
            .map(|template| template.name.clone())
            .collect())
    }

    fn unit_health_percent(&self, name: &str, this_object: Option<u32>) -> ScriptOwnerQuery<i32> {
        let Some(object) = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
            .and_then(|id| self.world.host_object(id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        // Existing admitted body baseline; never rediscover a global template
        // or substitute current maximum for original InitialHealth.
        if object.initial_health <= 0.0 || !object.initial_health.is_finite() {
            return ScriptOwnerQuery::Missing;
        }
        ScriptOwnerQuery::Present(
            ((object.health.current * 100.0 + object.initial_health / 2.0) / object.initial_health)
                as i32,
        )
    }

    fn player_census(
        &mut self,
        player: &str,
        player_mask: u32,
        current_player: Option<&str>,
        scope: generals_sim::scripting::world::ScriptPlayerCensusScope,
    ) -> ScriptOwnerQuery<(i32, generals_sim::scripting::world::HostScriptPlayerCensus)> {
        use generals_sim::scripting::world::ScriptPlayerCensusScope;

        let Some(id) = self.query_condition_player_id(player, player_mask, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let census = match scope {
            ScriptPlayerCensusScope::MoneyAndPower => self.world.owned_host_player_economy_for(id),
            ScriptPlayerCensusScope::ObjectCounts => self.world.owned_script_building_census_for(id),
            ScriptPlayerCensusScope::TemplateCounts => self.world.players.get(&id).map(|_| Default::default()),
        };
        match census {
            Some(census) => ScriptOwnerQuery::Present((id as i32, census)),
            None => ScriptOwnerQuery::Missing,
        }
    }

    fn player_template_count(
        &self,
        player: i32,
        templates: &[String],
        ignore_dead: bool,
    ) -> ScriptOwnerQuery<Option<i32>> {
        let templates: Vec<_> = templates
            .iter()
            .filter_map(|name| self.world.templates.get(name))
            .collect();
        if templates.is_empty() {
            return ScriptOwnerQuery::Present(None);
        }
        // Team.cpp:1593–1616 counts an object against its first equivalent
        // candidate, regardless of how many entries in the list match it.
        let count = self.world.host_objects().registered_values().filter(|object| {
            self.world.object_owned_by_player(object, player as u32)
                && !object.status.under_construction
                && (!ignore_dead || !object.status.effectively_dead)
                && templates.iter().any(|template| object.get_template().is_equivalent_to(template))
        }).count() as i32;
        ScriptOwnerQuery::Present(Some(count))
    }

    fn player_count_active(&self, player: i32) -> bool {
        self.world.players.get(&(player as u32)).is_some_and(Player::is_active)
    }

    fn player_has_any_objects(
        &self,
        player: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<bool> {
        let Some(player) = self
            .script_player_id(player, current_player)
            .filter(|id| self.world.players.contains_key(id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        // CPP Team::hasAnyObjects1751: exact status/kind filters. Other
        // legacy census exclusions do not alter this owner's condition.
        ScriptOwnerQuery::Present(self.world.host_objects().registered_values().any(|object| {
            self.world.object_owned_by_player(object, player)
                && !object.status.effectively_dead
                && !object.status.destroyed
                && !object.is_kind_of(KindOf::Projectile)
                && !object.is_kind_of(KindOf::Inert)
                && !object.is_kind_of(KindOf::Mine)
        }))
    }

    fn player_supply_source(
        &mut self,
        token: &str,
        player_mask: u32,
        current_player: Option<&str>,
        query: ScriptSupplyQuery,
    ) -> ScriptOwnerQuery<(u32, bool)> {
        let Some(player) = self
            .query_condition_player_id(token, player_mask, current_player)
            .filter(|id| self.world.players.contains_key(id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        let player_mask = 1u32.checked_shl(player).unwrap_or(0);
        let value = match query {
            ScriptSupplyQuery::Safe { minimum_cash } => {
                // AIPlayer.cpp:966,2164 checks getAiEnemy only within
                // findSupplyCenter, after the script condition cache admits this query.
                self.world.query_owned_ai_enemy(player);
                self.world
                    .ai_manager
                    .ai_players
                    .get(&player)
                    .is_none_or(|ai| ai.is_supply_source_safe(self.world, minimum_cash))
            }
            ScriptSupplyQuery::Attacked => {
                let Some(mut ai) = self.world.ai_manager.ai_players.remove(&player) else {
                    return ScriptOwnerQuery::Present((player_mask, false));
                };
                let attacked = ai.is_supply_source_attacked(self.world);
                self.world.ai_manager.ai_players.insert(player, ai);
                attacked
            }
        };
        ScriptOwnerQuery::Present((player_mask, value))
    }

    fn object_status(&self, id: u32) -> ScriptOwnerQuery<ScriptObjectStatus> {
        self.world
            .host_object(ObjectId(id))
            .map(Self::status)
            .map(ScriptOwnerQuery::Present)
            .unwrap_or(ScriptOwnerQuery::Missing)
    }

    fn condition_team_roster(
        &self,
        name: &str,
        calling: Option<u32>,
        contextual: Option<u32>,
    ) -> generals_sim::GameLogicResult<ScriptOwnerQuery<ScriptConditionTeam>> {
        let Some((id, members)) = self.world.resolve_owned_script_team(name, calling, contextual)?
        else {
            return Ok(ScriptOwnerQuery::Missing);
        };
        self.world
            .team_factory
            .with_read_without_flushing(|factory| {
                let Some(team) = factory.find_team_by_id(id) else {
                    return Ok(ScriptOwnerQuery::Missing);
                };
                let team = team.read().map_err(|error| {
                    generals_sim::GameLogicError::Threading(error.to_string())
                })?;
                Ok(ScriptOwnerQuery::Present(ScriptConditionTeam {
                    controller: team.get_controlling_player_id(),
                    members,
                }))
            })
            .map_err(|error| generals_sim::GameLogicError::Threading(error.to_string()))?
    }

    fn script_command_button(&self, name: &str) -> ScriptOwnerQuery<ScriptCommandButton> {
        self.world.script_command_button_definition(name)
    }

    fn script_command_button_power(&self, member: u32, power: &str) -> Option<bool> {
        self.world.script_command_button_power(ObjectId(member), power)
    }

    fn script_command_button_upgrade(&self, member: u32, upgrade: &str) -> bool {
        self.world.script_command_button_upgrade(ObjectId(member), upgrade)
    }

    fn script_last_damage_source(&self, member: u32) -> Option<ScriptDamageSource<'_>> {
        let object = self.world.host_object(ObjectId(member))?;
        let recorded = object.last_damage_source_template.as_str();
        let attacker = object
            .last_damage_source
            .and_then(|id| self.world.host_object(id));
        Some(ScriptDamageSource {
            recorded_template: (!recorded.is_empty()).then_some(recorded),
            live_attacker: attacker.map(|object| ScriptAttacker {
                template: &object.template_name,
                controller: self.world.player_owner_for_host_object(object),
            }),
        })
    }

    fn script_object_status_bits(&self, member: u32) -> Option<u64> {
        self.world
            .host_object(ObjectId(member))
            .map(host_query_object_status_bits)
    }

    fn script_discovery_candidate(&self, member: u32) -> bool {
        self.world.host_object(ObjectId(member)).is_some_and(|object| {
            !object.status.disabled_held && !object.is_effectively_stealthed()
        })
    }

    fn script_object_shroud(
        &self,
        member: u32,
        viewer: u32,
    ) -> Option<generals_sim::object::ObjectShroudStatus> {
        use generals_sim::object::ObjectShroudStatus;

        let object = self.world.host_object(ObjectId(member))?;
        // Object.cpp:1778 bypasses the partition for these objects.
        if object.get_template().always_visible || !self.is_in_script_partition(object) {
            return Some(ObjectShroudStatus::Clear);
        }
        let shroud = self.world.world_services.shroud().lock().ok()?;
        shroud.get_host_object_shroud_status(viewer, member).or_else(|| {
            (object.owner_player_id == Some(viewer)).then_some(ObjectShroudStatus::Clear)
        })
    }

    fn team_instances(&self, name: &str) -> ScriptOwnerQuery<Vec<u32>> {
        let factory = self
            .world
            .team_factory
            .lock()
            .expect("owned script team instances");
        if factory.find_team_prototype(name).is_none() {
            return ScriptOwnerQuery::Missing;
        }
        ScriptOwnerQuery::Present(
            factory
                .find_team_instances(name)
                .iter()
                .map(|team| team.read().expect("owned script team instance").get_id())
                .collect(),
        )
    }

    fn team_name(&self, id: u32) -> ScriptOwnerQuery<String> {
        let Some(team) = self.owned_team(id) else {
            return ScriptOwnerQuery::Missing;
        };
        let name = team
            .read()
            .expect("owned script team name")
            .get_name()
            .to_string();
        ScriptOwnerQuery::Present(name)
    }

    fn team_state(
        &self,
        name: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> ScriptOwnerQuery<String> {
        let Some((id, _)) = self
            .world
            .resolve_owned_script_team(name, calling, condition)
            .expect("owned team state selection")
        else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(team) = self.owned_team(id) else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present(
            team.read()
                .expect("owned script team state")
                .get_state()
                .to_string(),
        )
    }

    fn set_team_state(
        &mut self,
        name: &str,
        state: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        Some((|| {
            let Some((id, _)) = self
                .world
                .resolve_owned_script_team(name, calling, condition)?
            else {
                return Ok(());
            };
            let Some(team) = self.owned_team(id) else {
                return Ok(());
            };
            team.write()
                .map_err(|error| generals_sim::GameLogicError::Threading(error.to_string()))?
                .set_state(state.to_owned().into());
            Ok(())
        })())
    }

    fn condition_team_after(
        &self,
        name: &str,
        after: Option<u32>,
    ) -> ScriptOwnerQuery<Option<u32>> {
        let factory = self
            .world
            .team_factory
            .lock()
            .expect("owned condition-team cursor");
        if factory.find_team_prototype(name).is_none()
            || after.is_some_and(|id| {
                factory.find_team_by_id(id).is_none_or(|team| {
                    team.read()
                        .expect("owned condition-team identity")
                        .get_name()
                        != name
                })
            })
        {
            return ScriptOwnerQuery::Missing;
        }
        ScriptOwnerQuery::Present(factory.condition_team_after(name, after))
    }

    fn team_current_player(&self, id: u32) -> ScriptOwnerQuery<Option<String>> {
        let Some(team) = self.owned_team(id) else {
            return ScriptOwnerQuery::Missing;
        };
        let owner = team
            .read()
            .expect("owned script team controller")
            .get_controlling_player_id();
        ScriptOwnerQuery::Present(
            owner
                .and_then(|id| self.world.players.get(&id))
                .map(|player| Self::player_script_name(player).to_string()),
        )
    }

    fn team_contained(
        &self,
        name: &str,
        calling: Option<u32>,
        condition: Option<u32>,
        all: bool,
    ) -> ScriptOwnerQuery<bool> {
        let Some((_, members)) = self
            .world
            .resolve_owned_script_team(name, calling, condition)
            .expect("owned team containment selection")
        else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present(generals_sim::scripting::world::script_team_is_contained(
            members.into_iter().map(|id| {
                self.world
                    .host_object(ObjectId(id))
                    .map(|object| object.contained_by.is_some())
            }),
            all,
        ))
    }

    fn set_named_train_held(
        &mut self,
        name: &str,
        held: bool,
        this_object: Option<u32>,
    ) -> Option<()> {
        if let Some(id) = self.world.owned_named_script_object(name, this_object.map(ObjectId)) {
            self.world.set_railroad_held(id, held);
        }
        Some(())
    }

    fn set_named_cave_index(&mut self, name: &str, index: i32) -> Option<()> {
        self.world.set_named_cave_index(name, index);
        Some(())
    }
    fn team_presence(
        &self,
        name: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> ScriptOwnerQuery<ScriptTeamPresence> {
        let Some((id, members)) = self
            .world
            .resolve_owned_script_team(name, calling, condition)
            .expect("owned team presence selection")
        else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(team) = self.owned_team(id) else {
            return ScriptOwnerQuery::Missing;
        };
        let created = team.read().expect("owned team creation phase").is_created();
        let has_objects = members.into_iter().any(|id| {
            self.world.host_object(ObjectId(id)).is_some_and(|object| {
                !Self::status(object).effectively_dead
                    && !object.is_kind_of(KindOf::Projectile)
                    && !object.is_kind_of(KindOf::Inert)
                    && !object.is_kind_of(KindOf::Mine)
            })
        });
        ScriptOwnerQuery::Present(ScriptTeamPresence {
            created,
            has_objects,
        })
    }

    fn team_has_units(
        &self,
        name: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> ScriptOwnerQuery<bool> {
        let contextual = name == generals_sim::scripting::program::THIS_TEAM
            || calling.or(condition).is_some_and(|id| {
                self.owned_team(id).is_some_and(|team| {
                    team.read().expect("owned unit context").get_name().as_str() == name
                })
            });
        let ids = if contextual {
            match self
                .world
                .resolve_owned_script_team(name, calling, condition)
                .expect("owned unit team selection")
            {
                Some((id, _)) => vec![id],
                None => return ScriptOwnerQuery::Missing,
            }
        } else {
            match self.team_instances(name) {
                ScriptOwnerQuery::Present(ids) => ids,
                ScriptOwnerQuery::Missing => return ScriptOwnerQuery::Missing,
                ScriptOwnerQuery::Unavailable => unreachable!("Main owns its team factory"),
            }
        };
        for id in ids {
            let Some(team) = self.owned_team(id) else {
                continue;
            };
            let members = team
                .read()
                .expect("owned team unit roster")
                .get_members()
                .to_vec();
            if members.into_iter().any(|id| {
                self.world.host_object(ObjectId(id)).is_some_and(|object| {
                    !Self::status(object).effectively_dead
                        && !object.is_kind_of(KindOf::Structure)
                        && !object.is_kind_of(KindOf::Projectile)
                        && !object.is_kind_of(KindOf::Mine)
                })
            }) {
                return ScriptOwnerQuery::Present(true);
            }
        }
        ScriptOwnerQuery::Present(false)
    }

    fn team_status(&self, id: u32) -> ScriptOwnerQuery<ScriptTeamStatus> {
        let Some(team) = self.owned_team(id) else {
            return ScriptOwnerQuery::Missing;
        };
        let members = team
            .read()
            .expect("owned script team roster")
            .get_members()
            .to_vec();
        let mut idle = true;
        let mut dead = true;
        for id in members {
            let Some(object) = self.world.host_object(ObjectId(id)) else {
                continue;
            };
            let status = Self::status(object);
            // CPP AIGroup.cpp3086/3151: non-AI members do not affect idle;
            // every admitted live member affects all-dead. Empty groups are both.
            idle &= !status.has_ai || status.idle || status.effectively_dead;
            dead &= status.effectively_dead;
        }
        ScriptOwnerQuery::Present(ScriptTeamStatus {
            has_group: true,
            idle,
            dead,
        })
    }

    fn team_area_transition(
        &mut self,
        transition: generals_sim::scripting::world::ScriptTeamAreaTransition,
        team: &str,
        area: &str,
        current_player: Option<&str>,
        calling: Option<u32>,
        condition: Option<u32>,
        surfaces_allowed: u32,
    ) -> ScriptOwnerQuery<bool> {
        use generals_sim::scripting::world::ScriptTeamAreaTransition as Transition;
        let Some((team_id, members)) = self
            .world
            .resolve_owned_script_team(team, calling, condition)
            .expect("owned team trigger transition query")
        else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(area_name) = self.world.qualified_script_area(area, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(team_entered_or_exited) = self.world.team_factory.team_entered_or_exited(team_id)
        else {
            return ScriptOwnerQuery::Missing;
        };
        // Drop the receiving Team read guard before querying trigger slots;
        // no TeamFactory or Team lock is nested beneath the trigger owner.
        let now = self.world.frame;
        // C++ Team::m_enteredOrExited gates the query and is cleared by the
        // existing update_team_states hook after the sides finish.
        if !team_entered_or_exited {
            return ScriptOwnerQuery::Present(false);
        }
        let trigger_world = self
            .world
            .host_trigger_world
            .lock()
            .expect("owned team trigger transitions");
        let Some(trigger) = trigger_world.trigger_area_by_canonical_name(&area_name) else {
            return ScriptOwnerQuery::Missing;
        };
        let surfaces = ScriptSurfacesAllowed::from_bits_truncate(surfaces_allowed).locomotor_mask();
        let (mut any_considered, mut any_entered, mut any_outside) = (false, false, false);
        let (mut any_exited, mut any_inside) = (false, false);
        for id in members {
            let Some(object) = self.world.host_object(ObjectId(id)) else {
                continue;
            };
            let object_surfaces = if object.has_ai_update_interface() {
                object.locomotor_surfaces
            } else {
                generals_sim::pathfinding::SURFACE_GROUND
            };
            if (object_surfaces & surfaces) == 0
                || Self::status(object).effectively_dead
                || object.is_kind_of(KindOf::Inert)
            {
                continue;
            }
            any_considered = true;
            let entered = trigger_world.did_enter(id, &trigger, now);
            let exited = trigger_world.did_exit(id, &trigger, now);
            match transition {
                Transition::AllEntered | Transition::PartiallyEntered => {
                    any_entered |= entered;
                    any_outside |= !entered && !trigger_world.is_inside(id, &trigger);
                }
                Transition::AllExited | Transition::PartiallyExited => {
                    any_exited |= exited;
                    any_inside |= !exited && trigger_world.is_inside(id, &trigger);
                }
            }
        }
        ScriptOwnerQuery::Present(match transition {
            Transition::AllEntered => any_entered && !any_outside,
            Transition::PartiallyEntered => any_entered,
            Transition::AllExited => any_considered && any_exited && !any_inside,
            Transition::PartiallyExited => any_exited,
        })
    }

    fn bridge_damage_states_changed(&self) -> Option<bool> {
        Some(
            self.world
                .world_services
                .terrain()
                .read()
                .is_ok_and(|terrain| terrain.bridge_damage_states_changed()),
        )
    }

    fn bridge_status(
        &self,
        name: &str,
        this_object: Option<u32>,
    ) -> Option<ScriptBridgeStatus> {
        // Resolve only this world's name/ID pair. The shared named tracker may
        // contain the same ID from a foreign world and is not an owner.
        let bridge = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
            .filter(|id| self.world.host_object(*id).is_some())?;
        let terrain = self
            .world
            .world_services
            .terrain()
            .read()
            .ok()?;
        Some(ScriptBridgeStatus {
            broken: terrain.is_bridge_broken(bridge.0),
            repaired: terrain.is_bridge_repaired(bridge.0),
        })
    }

    fn sequential_current_player(
        &self,
        object_id: u32,
        team_id: Option<u32>,
    ) -> ScriptOwnerQuery<Option<String>> {
        let player = if let Some(object) = self.world.host_object(ObjectId(object_id)) {
            object.owner_player_id
        } else if let Some(id) = team_id {
            let Some(team) = self.owned_team(id) else {
                return ScriptOwnerQuery::Missing;
            };
            let owner = team
                .read()
                .expect("owned sequential team controller")
                .get_controlling_player_id();
            owner
        } else {
            None
        };
        // CPP7888: object controller precedes team controller; only SkirmishAI
        // becomes the sequential current player. Human/absent owners are final.
        ScriptOwnerQuery::Present(
            player
                .filter(|id| self.world.ai_manager.ai_players.contains_key(id))
                .and_then(|id| self.world.players.get(&id))
                .map(|player| Self::player_script_name(player).to_string()),
        )
    }
}

#[cfg(test)]
#[path = "map_visibility_owner_tests.rs"]
mod map_visibility_owner_tests;

#[cfg(test)]
#[path = "water_owner_tests.rs"]
mod water_owner_tests;

#[cfg(any(test, feature = "internal", feature = "game_client"))]
impl GameLogic {
    /// Execute an acceptance interpreter through the actual borrowed Main driver.
    /// No driver or engine is installed into process state.
    pub fn update_local_script_engine_with_owner(
        &mut self,
        engine: &gamelogic::scripting::engine::ScriptEngine,
    ) -> generals_sim::GameLogicResult<()> {
        self.with_own_services_mut(|world| {
            HostScriptExecutionDriver::with_interpreter(world, engine).advance_frame()
        })
    }

    #[cfg(test)]
    pub(crate) fn update_local_script_engine_with_effects(
        &mut self,
        engine: &gamelogic::scripting::engine::ScriptEngine,
        mut effects: crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        self.with_own_services_mut(|world| {
            HostScriptExecutionDriver::with_effects(world, engine, effects.reborrow())
                .advance_frame()
        })
    }
}

impl GameLogic {
    pub(crate) fn install_owned_script_engine(
        &mut self,
        engine: gamelogic::scripting::engine::ScriptEngine,
    ) {
        self.objects_receive_difficulty_bonus =
            engine.interpreter().frame().objects_should_receive_difficulty_bonus;
        self.named_script_cache.clear();
        self.owned_script_engine = Some(engine);
    }
    /// Observe only this session's interpreter at an input boundary.
    pub(crate) fn owned_script_game_ending(&self) -> bool {
        self.owned_script_engine
            .as_ref()
            .is_some_and(|engine| engine.is_game_ending())
    }

    /// Move the interpreter into one bounded operation; Drop restores it on unwind.
    pub(crate) fn with_owned_script_engine<R>(
        &mut self,
        operation: impl FnOnce(&gamelogic::scripting::engine::ScriptEngine, &mut GameLogic) -> R,
    ) -> Option<R> {
        struct Owner<'a> {
            world: &'a mut GameLogic,
            engine: Option<gamelogic::scripting::engine::ScriptEngine>,
        }
        impl Drop for Owner<'_> {
            fn drop(&mut self) {
                self.world.owned_script_engine = self.engine.take();
            }
        }
        let engine = self.owned_script_engine.take()?;
        let mut owner = Owner {
            world: self,
            engine: Some(engine),
        };
        Some(operation(owner.engine.as_ref().unwrap(), owner.world))
    }
    pub(crate) fn run_object_script_with_interpreter(
        &mut self,
        engine: &gamelogic::scripting::engine::ScriptEngine,
        name: &str,
        object: ObjectId,
    ) {
        self.run_object_script_with_effects(engine, name, object, &mut Default::default());
    }

    pub(crate) fn run_object_script_with_effects(
        &mut self,
        engine: &gamelogic::scripting::engine::ScriptEngine,
        name: &str,
        object: ObjectId,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let context = self.script_context();
        let mut driver = HostScriptExecutionDriver::with_effects(self, engine, ui.reborrow());
        let result = engine.with_script_program(context, &mut driver, |runtime, frame, program| {
            runtime.run_object_subroutine(name, object.0, frame, program)
        });
        if let Err(error) = result {
            log::error!("attached object script failed: {error}");
        }
    }
    // Explicit UI-only adapter for standalone callers.
    pub(crate) fn run_object_script_with_ui(
        &mut self,
        engine: &gamelogic::scripting::engine::ScriptEngine,
        name: &str,
        object: ObjectId,
        ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
    ) {
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            pristine_bones: None,
            script_events: None,
            partition: None,
            trigger_world: None,
            audio: None,
            terrain: None,
            ui: ui
                .as_mut()
                .map(|ui| &mut **ui as &mut dyn crate::game_logic::ScriptUiEffects),
            #[cfg(feature = "game_client")]
            headless_particles: None,
            view: None,
        };
        self.run_object_script_with_effects(engine, name, object, &mut effects)
    }

    pub(crate) fn run_owned_object_script(&mut self, name: &str, object: ObjectId) {
        self.run_owned_object_script_with_effects(name, object, &mut Default::default());
    }

    pub(crate) fn run_owned_object_script_with_effects(
        &mut self,
        name: &str,
        object: ObjectId,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if name.is_empty() || name == "<none>" {
            return;
        }
        assert!(
            self.owned_script_engine.is_some(),
            "a borrowed interpreter must be supplied explicitly for attached callbacks"
        );
        self.with_owned_script_engine(|engine, world| {
            world.run_object_script_with_effects(engine, name, object, ui)
        });
    }
}

impl GameLogic {
    pub(in crate::game_logic) fn script_context(&self) -> ScriptContext {
        ScriptContext::for_session_at_frame(self.frame, Arc::clone(&self.host_trigger_world))
    }

    pub(crate) fn evaluate_owned_script_conditions(
        &mut self,
        engine: &gamelogic::scripting::engine::ScriptEngine,
        script: &mut generals_sim::scripting::program::Script,
        player_name: Option<&str>,
    ) -> bool {
        let context = self.script_context();
        let mut driver = HostScriptExecutionDriver::with_interpreter(self, engine);
        engine.evaluate_conditions_with_driver(script, None, player_name, context, &mut driver)
    }
}

#[cfg(test)]
impl crate::game_logic::GameLogic {
    pub(in crate::game_logic) fn execute_owned_script_action_for_test(
        &mut self,
        action: &generals_sim::scripting::program::ScriptAction,
    ) {
        self.with_owned_script_engine(|engine, world| {
            let context = world.script_context();
            let mut driver = HostScriptExecutionDriver::with_interpreter(world, engine);
            engine
                .friend_execute_action_with_driver_checked(action, None, context, &mut driver)
                .unwrap();
        })
        .unwrap();
    }
}

#[cfg(test)]
#[path = "object_effect_owner_tests.rs"]
mod object_effect_owner_tests;

#[cfg(all(test, feature = "game_client"))]
#[path = "screen_shake_phase_tests.rs"]
mod screen_shake_phase_tests;

#[cfg(test)]
#[path = "target_selection_owner_tests.rs"]
mod target_selection_owner_tests;

#[cfg(test)]
#[path = "named_upgrade_owner_tests.rs"]
mod named_upgrade_owner_tests;

#[cfg(test)]
impl GameLogic {
    /// Component fixtures enter the original initial cache phase explicitly;
    /// no timer/team progression or ambient publication is added to setup.
    pub(crate) fn initialize_receiving_named_cache_for_fixture(&mut self) {
        self.with_owned_script_engine(|engine, world| {
            let mut driver = HostScriptExecutionDriver::with_interpreter(world, engine);
            engine.initialize_named_cache_on_first_update(&mut driver);
        })
        .expect("fixture owns its real interpreter");
    }
}

#[cfg(test)]
#[path = "named_cache_action_owner_tests.rs"]
mod named_cache_action_owner_tests;

#[path = "named_cache_mutation_actions.rs"]
mod named_cache_mutation_actions;

#[path = "named_cache_factory_actions.rs"]
mod named_cache_factory_actions;

#[path = "named_cache_ability_actions.rs"]
mod named_cache_ability_actions;

#[cfg(test)]
#[path = "named_cache_condition_owner_tests.rs"]
mod named_cache_condition_owner_tests;

#[cfg(test)]
#[path = "team_area_transition_owner_tests.rs"]
mod team_area_transition_owner_tests;
