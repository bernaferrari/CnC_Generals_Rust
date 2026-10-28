//! Live synchronous Main owner for one ScriptEngine action walk.
use super::*;
use gamelogic::common::Relationship;
use gamelogic::scripting::engine::{
    ScriptAiPlayerRequest, ScriptBridgeStatus, ScriptCameraRequest, ScriptDisplayRequest,
    ScriptExecutionDriver, ScriptNamedCommand, ScriptObjectStatus, ScriptOwnerQuery,
    ScriptPlayerAreaQuery, ScriptPlayerEventSource, ScriptSightingFilter, ScriptSkirmishCensus,
    ScriptSpecialPowerReadiness, ScriptTeamPresence, ScriptTeamStatus, ScriptWaterRequest,
};
use std::sync::{Arc, RwLock};

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
            audio: None,
            terrain: None,
            ui: ui,
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

    fn player_script_name(player: &Player) -> &str {
        &player.map_side.map_player_name
    }

    fn named_player_id(&self, name: &str) -> Option<u32> {
        // CPP Player::initFromDict stores displayName and playerName
        // separately; PlayerList::findPlayerWithNameKey selects the latter.
        if name.is_empty() {
            return None;
        }
        self.world
            .players
            .values()
            .find(|player| player.map_side.map_player_name == name)
            .map(|player| player.id)
    }

    fn requires_prepared_player_bindings(&self) -> bool {
        // These paths still consume PlayerList side ordinals. The tech query
        // uses exact admitted Main Player names instead and does not infer IDs
        // from a side index.
        matches!(
            self.world.game_mode,
            GameMode::Skirmish
                | GameMode::Multiplayer
                | GameMode::Lan
                | GameMode::Internet
                | GameMode::Replay
        )
    }

    pub(super) fn script_player_id(&self, token: &str, current_player: Option<&str>) -> Option<u32> {
        use gamelogic::scripting::core::{
            LOCAL_PLAYER, THE_PLAYER, THIS_PLAYER, THIS_PLAYER_ENEMY,
        };
        match token {
            THIS_PLAYER => self.named_player_id(current_player?),
            LOCAL_PLAYER => self.world.local_player_id(),
            THIS_PLAYER_ENEMY => {
                let current = self.named_player_id(current_player?)?;
                self.world
                    .ai_manager
                    .ai_players
                    .get(&current)
                    .and_then(|ai| ai.enemy_player_id)
                    .filter(|id| self.world.players.contains_key(id))
                    .or_else(|| {
                        self.world
                            .players
                            .values()
                            .filter(|p| {
                                p.is_human
                                    && !(self.world.is_challenge_session()
                                        && Self::player_script_name(p) == THE_PLAYER)
                            })
                            .min_by_key(|p| p.id)
                            .map(|p| p.id)
                    })
            }
            // Retain the existing Challenge classification. Campaign metadata
            // ownership is separate from this query's player/geometry owner.
            THE_PLAYER if self.world.is_challenge_session() => self.world.local_player_id(),
            _ => self.named_player_id(token),
        }
    }

    fn qualified_area(&self, area: &str, current_player: Option<&str>) -> Option<String> {
        let (enemy, perimeter) = match area {
            "[Skirmish]MyInnerPerimeter" => (false, "InnerPerimeter"),
            "[Skirmish]MyOuterPerimeter" => (false, "OuterPerimeter"),
            "[Skirmish]EnemyInnerPerimeter" => (true, "InnerPerimeter"),
            "[Skirmish]EnemyOuterPerimeter" => (true, "OuterPerimeter"),
            _ => return Some(area.into()),
        };
        let current = current_player.and_then(|name| self.named_player_id(name));
        let player = if enemy {
            current
                .and_then(|id| self.world.ai_manager.ai_players.get(&id))
                .and_then(|ai| ai.enemy_player_id)
        } else {
            Some(current?)
        };
        let index = player
            .and_then(|id| self.world.players.get(&id))
            .map(|p| p.start_position + 1);
        Some(format!("{perimeter}{}", index.unwrap_or(-1)))
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
        if let Some(view) = self.ui.view.as_deref() {
            view.visit_owned_ambient_audio_handles(&mut |handle| extra_handles.push(handle));
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
    ) -> gamelogic::GameLogicResult<()> {
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
            .map_err(gamelogic::GameLogicError::Configuration)
    }

    /// CPP OpenContain::addOrRemoveObjFromWorld removes enclosed riders and
    /// their descendants from the partition. Non-enclosing riders stay visible.
    fn sighting_candidate_is_in_partition(&self, candidate: &Object) -> bool {
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
    fn music_track_completed(
        &mut self,
        track: &str,
        number_of_times: i32,
    ) -> Option<gamelogic::GameLogicResult<bool>> {
        let extra_handles = self.owned_audio_extra_receipts();
        Some(match self.ui.audio.as_deref_mut() {
            Some(audio) => audio
                .music_track_completed(track, number_of_times, &extra_handles)
                .map_err(gamelogic::GameLogicError::Configuration),
            None => Ok(false),
        })
    }

    fn speech_play(
        &mut self,
        name: &str,
        allow_overlap: bool,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let mut event = game_engine::common::audio::AudioEventRts::with_event_name(name);
        event.set_is_logical_audio(true);
        event.set_uninterruptable(!allow_overlap);
        let player = self
            .world
            .local_player_id()
            .map(|id| id as i32)
            .unwrap_or(-1);
        event.set_player_index(player);
        let result = self.admit_owned_script_audio(&event, player);
        if result.is_ok() {
            self.world.mission_scripts.admit_speech_caption(name);
        }
        Some(result)
    }

    fn audio(
        &mut self,
        request: gamelogic::scripting::engine::ScriptAudioRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        use gamelogic::scripting::engine::ScriptAudioRequest as Request;
        let valid = match request {
            Request::SetVolume { volume, .. } | Request::NamedVolume { volume, .. } => {
                volume.is_finite()
            }
            _ => true,
        };
        if !valid {
            return Some(Err(gamelogic::GameLogicError::Configuration(
                "nonfinite script audio policy".into(),
            )));
        }
        match request {
            Request::SetVolume { volume, affect } => {
                self.world.queued_audio_events.set_volume(volume, affect)
            }
            Request::NamedVolume { name, volume } => self
                .world
                .queued_audio_events
                .set_named_volume(name, volume),
            Request::Pause(_) => self.world.queued_audio_events.pause_pending(),
            _ => {}
        }
        let extra_handles = self.owned_audio_extra_receipts();
        Some(if let Some(audio) = self.ui.audio.as_deref_mut() {
            audio
                .script_audio(
                    request,
                    self.world.queued_audio_events.policy(),
                    &extra_handles,
                )
                .map_err(gamelogic::GameLogicError::Configuration)
        } else {
            // Explicit non-presentation/headless execution owns policy but no playing receipts.
            Ok(())
        })
    }

    fn set_campaign_victorious(
        &mut self,
        victorious: bool,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(match self.ui.ui.as_deref_mut() {
            Some(ui) => ui.set_campaign_victorious(victorious),
            None if self.world.has_campaign_session() => {
                Err(gamelogic::GameLogicError::Configuration(
                    "campaign victory requires the receiving application's UI owner".into(),
                ))
            }
            None => Ok(()),
        })
    }

    fn create_win_lose_window(
        &mut self,
        layout_filename: &str,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(match self.ui.ui.as_deref_mut() {
            Some(ui) => ui.create_win_lose_window(layout_filename),
            None => Ok(()),
        })
    }

    fn destroy_win_lose_window(&mut self) -> Option<gamelogic::GameLogicResult<()>> {
        Some(match self.ui.ui.as_deref_mut() {
            Some(ui) => ui.destroy_win_lose_window(),
            None => Ok(()),
        })
    }
    fn close_game_windows(&mut self) -> Option<gamelogic::GameLogicResult<()>> {
        Some(match self.ui.ui.as_deref_mut() {
            Some(ui) => ui.close_game_windows(),
            None => Ok(()),
        })
    }

    fn initialize_named_cache(&mut self) -> Option<()> {
        self.world.initialize_receiving_named_script_cache();
        Some(())
    }

    fn set_buildable_status_override(
        &mut self,
        template: &str,
        status: i32,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.world.script_set_buildable_override(template, status);
        Some(Ok(()))
    }
    fn command_bar_override(
        &mut self,
        button: &str,
        template: &str,
        slot: Option<i32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.world
            .script_command_bar_override(button, template, slot);
        Some(Ok(()))
    }

    fn local_victory_state(
        &self,
    ) -> gamelogic::scripting::engine::ScriptOwnerQuery<(bool, bool, bool)> {
        let victory = &self.world.victory_conditions;
        let players = &self.world.players;
        gamelogic::scripting::engine::ScriptOwnerQuery::Present((
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

    fn set_draw_icon_ui(&mut self, enabled: bool) -> Option<gamelogic::GameLogicResult<()>> {
        self.world.set_draw_icon_ui(enabled);
        Some(Ok(()))
    }

    fn override_hulk_lifetime(&mut self, frames: i32) -> Option<gamelogic::GameLogicResult<()>> {
        self.world.native_hulk_max_lifetime_override = frames;
        Some(Ok(()))
    }
    fn delete_or_kill(
        &mut self,
        request: &gamelogic::scripting::HostScriptKillDeleteDamageRequest,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(self.world.apply_owned_script_lifecycle_with_effects(
            request,
            this_object.map(ObjectId),
            calling,
            condition,
            self.interpreter,
            &mut self.ui,
        ))
    }

    fn kill_player(
        &mut self,
        player: &str,
        current: Option<&str>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if let Some(id) = self.script_player_id(player, current) {
            self.world.kill_player_with_interpreter_with_effects(
                id,
                self.interpreter,
                &mut self.ui,
            );
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
        request: &gamelogic::scripting::HostScriptMoneyRequest,
        current_player: Option<&str>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        use gamelogic::scripting::HostScriptMoneyRequest;
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

    fn object_effect(
        &mut self,
        request: gamelogic::scripting::engine::ScriptObjectEffect<'_>,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        use gamelogic::scripting::engine::ScriptObjectEffect;
        use gamelogic::scripting::{
            HostScriptObjectSoundRequest as Sound, HostScriptRadarEventRequest as Radar,
            HostScriptStealthEnabledRequest as Stealth, HostScriptUnmannedRequest as Unmanned,
        };
        let named = |world: &GameLogic, name: &str| {
            world.owned_named_script_object(name, this_object.map(ObjectId))
        };
        let members = |world: &GameLogic, name: &str| -> gamelogic::GameLogicResult<Vec<u32>> {
            Ok(world
                .resolve_owned_script_team(name, calling, condition)?
                .map(|(_, ids)| ids)
                .unwrap_or_default())
        };
        let result = (|| -> gamelogic::GameLogicResult<()> {
            match request {
                ScriptObjectEffect::Sound(Sound::PlayNamed { sound, unit }) => {
                    if let Some(id) = named(self.world, unit) {
                        // The native audio service is a real external boundary;
                        // identity is selected only from the driving simulation.
                        let mut event =
                            game_engine::common::audio::AudioEventRts::with_object_id(sound, id.0);
                        event.set_is_logical_audio(true);
                        let object = self.world.host_object(id).ok_or_else(|| {
                            gamelogic::GameLogicError::Configuration(
                                "named audio object left its receiving world during admission"
                                    .into(),
                            )
                        })?;
                        let position = object.get_position();
                        event.set_receiving_object_spatial_owner(
                            id.0,
                            &object.visual_draw_identity_token,
                            game_engine::common::audio::audio_event_rts::Coord3D {
                                x: position.x,
                                y: position.z,
                                z: position.y,
                            },
                        );
                        let player = object.owner_player_id.map(|p| p as i32).unwrap_or(-1);
                        event.set_player_index(player);
                        self.admit_owned_script_audio(&event, player)?;
                    }
                }
                ScriptObjectEffect::Sound(Sound::Enable { unit, enable }) => {
                    if let Some(id) = named(self.world, unit) {
                        self.world.enable_owned_object_sound(id, *enable);
                    }
                }
                ScriptObjectEffect::Unmanned(request) => {
                    let ids = match request {
                        Unmanned::Named { unit } => {
                            named(self.world, unit).into_iter().map(|id| id.0).collect()
                        }
                        Unmanned::Team { team } => members(self.world, team)?,
                        Unmanned::DeleteAll => self
                            .world
                            .host_objects()
                            .registered_values()
                            .filter(|o| o.status.disabled_unmanned && !o.status.destroyed)
                            .map(|o| o.id.0)
                            .collect(),
                    };
                    for id in ids {
                        if matches!(request, Unmanned::DeleteAll) {
                            self.world
                                .destroy_object_with_effects(ObjectId(id), &mut self.ui);
                        } else {
                            self.world.host_script_set_unmanned(ObjectId(id));
                        }
                    }
                }
                ScriptObjectEffect::Stealth(request) => {
                    let (ids, enabled) = match request {
                        Stealth::Named { unit, enabled } => (
                            named(self.world, unit).into_iter().map(|id| id.0).collect(),
                            *enabled,
                        ),
                        Stealth::Team { team, enabled } => (members(self.world, team)?, *enabled),
                    };
                    for id in ids {
                        self.world
                            .host_script_set_stealth_enabled(ObjectId(id), enabled);
                    }
                }
                ScriptObjectEffect::Radar(request) => {
                    let (position, kind) = match request {
                        Radar::Object { unit, event_type } => (
                            named(self.world, unit)
                                .and_then(|id| self.world.host_object(id))
                                .map(|o| o.get_position()),
                            *event_type,
                        ),
                        Radar::Team { team, event_type } => (
                            members(self.world, team)?.into_iter().find_map(|id| {
                                self.world
                                    .host_object(ObjectId(id))
                                    .filter(|o| o.is_alive() && !o.status.destroyed)
                                    .map(|o| o.get_position())
                            }),
                            *event_type,
                        ),
                    };
                    if let Some(position) = position {
                        self.world.host_create_radar_event(
                            position,
                            GameLogic::host_script_radar_event_type(kind),
                        );
                    }
                }
            }
            Ok(())
        })();
        Some(result)
    }

    fn named_containment(
        &self,
        unit: &str,
        this_object: Option<u32>,
    ) -> ScriptOwnerQuery<gamelogic::scripting::engine::ScriptContainmentStatus> {
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
    ) -> ScriptOwnerQuery<gamelogic::scripting::engine::ScriptNamedLifecycle> {
        let object = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
            .and_then(|id| self.world.host_object(id));
        ScriptOwnerQuery::Present(gamelogic::scripting::engine::ScriptNamedLifecycle {
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
        engine: &gamelogic::scripting::engine::ScriptEngine,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(
            self.world
                .update_owned_script_team_states_with_effects(engine, &mut self.ui),
        )
    }

    fn local_player_observer(&self) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Present(
            self.world
                .local_player_id()
                .and_then(|id| self.world.get_player(id))
                .is_some_and(|player| player.is_observer),
        )
    }

    fn create_named_object(
        &mut self,
        name: Option<&str>,
        request: gamelogic::scripting::engine::ScriptObjectCreation<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_create_named_object(name, request)
    }

    fn create_named_object_at_waypoint(
        &mut self,
        name: Option<&str>,
        template: &str,
        team: &str,
        waypoint: &str,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_create_named_object_at_waypoint(name, template, team, waypoint)
    }

    fn create_object(
        &mut self,
        request: gamelogic::scripting::engine::ScriptObjectCreation<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(self.world.apply_owned_script_create_object_with_effects(
            request,
            calling,
            condition,
            self.interpreter,
            &mut self.ui,
        ))
    }

    fn named_presentation(
        &mut self,
        request: gamelogic::scripting::engine::ScriptNamedPresentation<'_>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        use gamelogic::scripting::engine::ScriptNamedPresentation;
        let name = match request {
            ScriptNamedPresentation::Flash { unit, .. }
            | ScriptNamedPresentation::CustomColor { unit, .. } => unit,
        };
        let Some(id) = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
        else {
            return Some(Ok(()));
        };
        match request {
            ScriptNamedPresentation::Flash { seconds, white, .. } => {
                let color = self.world.host_script_flash_color(id, white);
                if let Some(object) = self.world.host_object_mut(id) {
                    object.set_script_flash(seconds, color);
                }
            }
            ScriptNamedPresentation::CustomColor { color, .. } => {
                if let Some(object) = self.world.host_object_mut(id) {
                    object.set_custom_indicator_color_raw(color);
                }
            }
        }
        Some(Ok(()))
    }

    fn team_attack_named(
        &mut self,
        team: &str,
        target: &str,
        this_object: Option<gamelogic::common::ObjectID>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(self.world.apply_owned_team_attack_named(
            team,
            target,
            this_object.map(ObjectId),
            calling,
            condition,
            &mut self.ui,
        ))
    }

    fn named_topple_direction(
        &mut self,
        name: &str,
        direction: gamelogic::common::Coord3D,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if let Some(engine) = self.interpreter.or(self.world.owned_script_engine.as_ref()) {
            engine.set_topple_direction(name, Some(direction));
        }
        Some(Ok(()))
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
    fn team_guard_object(
        &mut self,
        team: &str,
        target: &str,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(self.world.apply_owned_team_guard_object(
            team,
            target,
            this_object.map(ObjectId),
            calling,
            condition,
        ))
    }
    fn team_damage(
        &mut self,
        team: &str,
        amount: f32,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(self.world.apply_owned_team_damage_with_effects(
            team,
            amount,
            calling,
            condition,
            self.interpreter,
            &mut self.ui,
        ))
    }
    fn team_attack_team(
        &mut self,
        source: &str,
        target: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
    fn named_waypoint(
        &mut self,
        unit: &str,
        waypoint: &str,
        follow: Option<bool>,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.world
            .apply_owned_named_waypoint(unit, waypoint, follow, this_object.map(ObjectId));
        Some(Ok(()))
    }
    fn named_fire_weapon_following_waypoint_path(
        &mut self,
        unit: &str,
        waypoint_path: &str,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.world
            .host_script_named_fire_weapon_following_path_with_effects(
                unit,
                waypoint_path,
                &mut self.ui,
            );
        Some(Ok(()))
    }
    fn team_hunt_button(
        &mut self,
        team: &str,
        button: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(self.world.apply_owned_team_hunt(team, calling, condition))
    }
    fn team_load_transports(
        &mut self,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_load_transports(team, calling, condition),
        )
    }
    fn team_relationship_override(
        &mut self,
        action: gamelogic::scripting::engine::ScriptTeamRelationship<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_relationship_override(action, calling, condition),
        )
    }
    fn player_team_relationship_override(
        &mut self,
        action: gamelogic::scripting::engine::ScriptPlayerTeamRelationship<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if team == gamelogic::scripting::core::TEAM_THE_PLAYER && self.world.is_challenge_session()
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let Some(player) = self.script_player_id(player, current_player) else {
            return Some(Ok(()));
        };
        Some(
            self.world
                .apply_owned_team_transfer(team, player, calling, condition),
        )
    }
    fn sell_everything(
        &mut self,
        player: &str,
        current_player: Option<&str>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if let Some(player) = self.script_player_id(player, current_player) {
            self.world
                .apply_owned_sell_everything_with_effects(player, &mut self.ui);
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if team == gamelogic::scripting::core::TEAM_THE_PLAYER && self.world.is_challenge_session()
        {
            let Some(player_id) = self.world.local_player_id() else {
                return Some(Err(gamelogic::GameLogicError::Configuration(
                    "challenge TeamMove has no receiving local Player".into(),
                )));
            };
            let members = match self.world.resolve_owned_player_default_team(player_id) {
                Ok(Some((_, members))) => members,
                Ok(None) => {
                    return Some(Err(gamelogic::GameLogicError::Configuration(
                        "challenge TeamMove has no admitted local default Team".into(),
                    )));
                }
                Err(error) => return Some(Err(error)),
            };
            let Some(destination) = self.world.host_script_waypoint_position(waypoint) else {
                return Some(Ok(()));
            };
            for member in members {
                self.world
                    .unit_command_move_to(ObjectId(member), destination);
            }
            return Some(Ok(()));
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let Some(area) = self.qualified_area(area, current_player) else {
            return Some(Ok(()));
        };
        Some(self.world.apply_owned_named_attack_area_with_effects(
            unit,
            &area,
            this_object.map(ObjectId),
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
        request: &gamelogic::scripting::HostScriptUseCommandButtonRequest,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_team_partial_button(team, button, percentage, calling, condition),
        )
    }
    fn team_panic(
        &mut self,
        team: &str,
        path: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(
            self.world.apply_owned_skirmish_approach_path(
                team, path, as_team, follow, calling, condition,
            ),
        )
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
        &self,
        team: &str,
        area: &str,
        current_player: Option<&str>,
        calling: Option<u32>,
        condition: Option<u32>,
        which_to_consider: u32,
    ) -> ScriptOwnerQuery<gamelogic::scripting::engine::ScriptTeamAreaMembership> {
        use gamelogic::scripting::engine::ScriptTeamAreaMembership;

        let Some((_, members)) = self
            .world
            .resolve_owned_script_team(team, calling, condition)
            .expect("owned team area query")
        else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(area_name) = self.qualified_area(area, current_player) else {
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
        let remapped = (which_to_consider & 0x01) | ((which_to_consider & 0x02) << 2);
        for member_id in members {
            let Some(object) = self.world.host_object(ObjectId(member_id)) else {
                continue;
            };
            let surfaces = if object.has_ai_update_interface() {
                object.locomotor_surfaces
            } else {
                gamelogic::path::SURFACE_GROUND
            };
            if (surfaces & remapped) == 0
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

    fn skirmish_special_power_readiness(
        &self,
        player: &str,
        power_name: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<ScriptSpecialPowerReadiness> {
        let Some(owner) = self.script_player_id(player, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
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
        let mut state = ScriptSpecialPowerReadiness {
            template_exists,
            ready: false,
            next_frame: self.world.frame.wrapping_add(300),
        };
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
                    !self
                        .world
                        .get_player(owner)
                        .unwrap()
                        .has_unlocked_science(science)
                }) {
                    continue;
                }
                let Some(power) = module.command_power.as_ref() else {
                    continue;
                };
                if self.world.is_special_power_ready_for(object.id, power) {
                    state.ready = true;
                    return ScriptOwnerQuery::Present(state);
                }
                let seconds = if module.shared_n_sync {
                    self.world
                        .get_player(owner)
                        .unwrap()
                        .shared_special_power_remaining(power)
                } else {
                    object
                        .special_power_cooldowns
                        .get(power)
                        .copied()
                        .unwrap_or(0.0)
                };
                let ready_frame = self
                    .world
                    .frame
                    .wrapping_add((seconds.max(0.0) * 30.0) as u32);
                state.next_frame = state.next_frame.min(ready_frame);
            }
        }
        ScriptOwnerQuery::Present(state)
    }

    fn skirmish_census(
        &self,
        player: &str,
        current_player: Option<&str>,
        query: ScriptSkirmishCensus,
    ) -> ScriptOwnerQuery<i32> {
        let owner = match query {
            ScriptSkirmishCensus::NeutralUnmanned => self
                .world
                .get_players()
                .values()
                .find(|player| player.team == Team::Neutral)
                .map(|player| player.id),
            _ => self.script_player_id(player, current_player),
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
                ScriptSkirmishCensus::Garrisoned => {
                    object.is_garrison_contain() && !object.contained_units().is_empty()
                }
                ScriptSkirmishCensus::Captured => object.status.private_captured,
            })
            .count() as i32;
        ScriptOwnerQuery::Present(count)
    }
    fn player_discovered_by(
        &self,
        player: &str,
        viewer: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<bool> {
        let (Some(owner), Some(viewer)) = (
            self.script_player_id(player, current_player),
            self.script_player_id(viewer, current_player),
        ) else {
            return ScriptOwnerQuery::Missing;
        };
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
                        gamelogic::common::ObjectShroudStatus::Clear
                            | gamelogic::common::ObjectShroudStatus::PartialClear
                    )
                )
            });
        ScriptOwnerQuery::Present(found)
    }

    fn player_area_value(
        &self,
        player: &str,
        area: &str,
        current_player: Option<&str>,
        query: ScriptPlayerAreaQuery<'_>,
    ) -> ScriptOwnerQuery<(i32, bool)> {
        let Some(owner) = self.script_player_id(player, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(area) = self.qualified_area(area, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let geometry = self
            .world
            .host_trigger_world
            .lock()
            .expect("owned player polygon census");
        let Some(trigger) = geometry.trigger_area_by_canonical_name(&area) else {
            return ScriptOwnerQuery::Missing;
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
            let point = gamelogic::common::ICoord3D::new(
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
                        && names
                            .iter()
                            .any(|name| object.template_name.eq_ignore_ascii_case(name))
                    {
                        1
                    } else {
                        0
                    }
                }
                ScriptPlayerAreaQuery::Kind(kind) => {
                    let mask = crate::game_logic::host_radar_stealth_vision_residual::live_object_kind_of_cpp_mask(object.thing().template.kind_of.iter());
                    let matches = gamelogic::common::kind_of_indices(*kind)
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
        ScriptOwnerQuery::Present((value, changed))
    }

    fn named_query_fact(
        &self,
        name: &str,
        this_object: Option<u32>,
    ) -> ScriptOwnerQuery<(u32, gamelogic::scripting::HostScriptQueryObject)> {
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
        this_object: Option<u32>,
    ) -> ScriptOwnerQuery<gamelogic::scripting::engine::ScriptSelectionStatus> {
        ScriptOwnerQuery::Present(gamelogic::scripting::engine::ScriptSelectionStatus {
            multiplayer: self.world.isInMultiplayerGame(),
            changed_frame: self.world.selection_changed_frame,
            selected: self
                .world
                .owned_named_script_object(literal_name, this_object.map(ObjectId))
                .is_some_and(|id| self.world.selected_objects.contains(&id)),
        })
    }

    fn named_inside_area(
        &self,
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
        let Some(area) = self.qualified_area(area, current_player) else {
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
        let pos = object.get_position();
        let point = gamelogic::common::ICoord3D::new(pos.x as i32, pos.z as i32, pos.y as i32);
        ScriptOwnerQuery::Present(trigger.point_in_trigger_int(&point))
    }

    fn named_entered_area(
        &self,
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
        let Some(area) = self.qualified_area(area, current_player) else {
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
            object.id.0,
            &trigger,
            self.world.frame,
        ))
    }

    fn named_exited_area(
        &self,
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
        let Some(area) = self.qualified_area(area, current_player) else {
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
            object.id.0,
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

    fn sighted(
        &self,
        unit: &str,
        player: &str,
        filter: ScriptSightingFilter<'_>,
        current_player: Option<&str>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> ScriptOwnerQuery<bool> {
        let Some(looker) = self
            .world
            .owned_named_script_object(unit, this_object.map(ObjectId))
            .and_then(|id| self.world.host_object(id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(player) = self
            .script_player_id(player, current_player)
            .filter(|id| self.world.players.contains_key(id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        // CPP Object1757/2800 updates OFF_MAP from the current terrain extent.
        // Main canonical poses and admitted extent supply the same 2D test.
        let off_map = |object: &Object| {
            let pos = object.get_position();
            pos.x < self.world.world_min.x
                || pos.x > self.world.world_max.x
                || pos.z < self.world.world_min.z
                || pos.z > self.world.world_max.z
        };
        let source = looker.get_position();
        let source_off_map = off_map(looker);
        let found = self
            .world
            .host_objects()
            .registered_values()
            .any(|candidate| {
                if candidate.id == looker.id
                    || candidate.status.effectively_dead
                    || !self.sighting_candidate_is_in_partition(candidate)
                    || candidate.is_effectively_stealthed()
                    || off_map(candidate) != source_off_map
                    || candidate.owner_player_id != Some(player)
                {
                    return false;
                }
                let position = candidate.get_position();
                let dx = position.x - source.x;
                let dz = position.z - source.z;
                // CPP FROM_CENTER_2D rejects equality; altitude is irrelevant.
                if !(dx * dx + dz * dz < looker.vision_range * looker.vision_range) {
                    return false;
                }
                match filter {
                    ScriptSightingFilter::Types(names) => {
                        names.iter().any(|name| *name == candidate.template_name)
                    }
                    ScriptSightingFilter::Relationship(alliance) => {
                        let relationship =
                            self.world.script_sighting_relationship(looker, candidate);
                        match alliance {
                            0 => relationship == Relationship::Enemies,
                            1 => relationship == Relationship::Neutral,
                            2 => relationship == Relationship::Allies,
                            _ => false,
                        }
                    }
                }
            });
        // CPP filters candidate life, not the looking object's life.
        ScriptOwnerQuery::Present(found)
    }

    fn skirmish_player_can_build_any(
        &self,
        player: &str,
        object_types: &[String],
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<bool> {
        if self.requires_prepared_player_bindings() {
            return ScriptOwnerQuery::Unavailable;
        }
        let Some(player) = self
            .script_player_id(player, current_player)
            .and_then(|id| self.world.players.get(&id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present(
            object_types
                .iter()
                .any(|name| self.world.script_player_can_build_template(player, name)),
        )
    }

    fn player_event_source(
        &self,
        player: &str,
        source: Option<&str>,
        current_player: Option<&str>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> ScriptOwnerQuery<ScriptPlayerEventSource> {
        // Prepared map-side indices still require explicit admission (hq-fxwkm).
        // Ordinary Main notifications use the controlling host player ID.
        if self.requires_prepared_player_bindings() {
            return ScriptOwnerQuery::Unavailable;
        }
        let Some(player_index) = self.script_player_id(player, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let source_object = match source {
            None => gamelogic::common::INVALID_ID,
            Some(gamelogic::scripting::core::THIS_OBJECT) => {
                let Some(id) =
                    this_object.filter(|id| self.world.host_object(ObjectId(*id)).is_some())
                else {
                    return ScriptOwnerQuery::Missing;
                };
                id
            }
            Some(name) => {
                let Some(id) = self
                    .world
                    .host_object_id_by_script_name(name)
                    .map(|id| id.0)
                else {
                    return ScriptOwnerQuery::Missing;
                };
                id
            }
        };
        ScriptOwnerQuery::Present(ScriptPlayerEventSource {
            player_index: player_index as usize,
            source_object,
        })
    }

    fn team_guard(
        &mut self,
        team: &str,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        // CPP5936 returns the exact default Team pointer in Challenge mode.
        // That identity is not yet admitted by Main (hq-dryhs/hq-9bqdm);
        // retain the existing adapter for this capability rather than invent
        // a team from a display name or choose an unrelated same-name instance.
        if team == gamelogic::scripting::core::TEAM_THE_PLAYER && self.world.is_challenge_session()
        {
            return None;
        }
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(self.world.apply_owned_team_idle(team, calling, condition))
    }

    fn team_stop(
        &mut self,
        team: &str,
        calling: Option<gamelogic::team::TeamID>,
        condition: Option<gamelogic::team::TeamID>,
        disband: bool,
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if team == gamelogic::scripting::core::TEAM_THE_PLAYER && self.world.is_challenge_session()
        {
            return None;
        }
        Some(
            self.world
                .apply_owned_team_attitude(team, calling_team, condition_team, mood),
        )
    }

    fn team_sequential(
        &mut self,
        team: &str,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
        script: Option<(gamelogic::scripting::core::Script, i32)>,
        engine: &gamelogic::scripting::engine::ScriptEngine,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if team == gamelogic::scripting::core::TEAM_THE_PLAYER && self.world.is_challenge_session()
        {
            return None;
        }
        Some(self.world.apply_owned_team_sequential(
            team,
            calling_team,
            condition_team,
            script,
            engine,
        ))
    }

    fn nearest_object_move(
        &mut self,
        request: gamelogic::scripting::engine::ScriptNearestObjectRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let Some(trigger) = self.qualified_area(request.trigger, request.current_player) else {
            return Some(Ok(()));
        };
        Some(
            self.world
                .apply_owned_nearest_object_move(request, &trigger),
        )
    }

    fn named_attitude(
        &mut self,
        unit: &str,
        this_object: Option<gamelogic::common::ObjectID>,
        mood: i32,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let Some(id) = self
            .world
            .owned_named_script_object(unit, this_object.map(ObjectId))
        else {
            return Some(Ok(()));
        };
        let attitude = match mood {
            -2 => -2,
            -1 => -1,
            1 => 1,
            2 => 2,
            _ => 0,
        };
        if let Some(object) = self.world.host_object_mut(id) {
            if object.has_ai_update_interface() {
                object.set_ai_attitude_i8(attitude);
            }
        }
        Some(Ok(()))
    }

    fn named_attack_priority(
        &mut self,
        name: &str,
        set: &str,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_named_attack_priority(name, set, this_object)
    }

    fn named_evac_disposition(
        &mut self,
        name: &str,
        disposition: i32,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_named_evac_disposition(name, disposition, this_object)
    }

    fn named_panel_flag(
        &mut self,
        name: &str,
        flag: &str,
        enabled: bool,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_named_panel_flag(name, flag, enabled, this_object)
    }

    fn named_warehouse_value(
        &mut self,
        name: &str,
        cash: i32,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_named_warehouse_value(name, cash, this_object)
    }

    fn named_runtime(
        &mut self,
        request: gamelogic::scripting::engine::ScriptNamedRuntimeRequest<'_>,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_named_runtime(request, this_object)
    }

    fn garrison_request(
        &mut self,
        request: gamelogic::scripting::HostScriptGarrisonEnterExitRequest,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_garrison_request(request, this_object, calling, condition)
    }

    fn team_exit_all(
        &mut self,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(
            self.world
                .apply_receiving_team_exit_all(team, calling, condition, &mut self.ui),
        )
    }

    fn named_special_power(
        &mut self,
        request: gamelogic::scripting::HostScriptNamedFireSpecialPowerRequest,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_named_special_power(request, this_object)
    }

    fn command_button_request(
        &mut self,
        request: gamelogic::scripting::HostScriptUseCommandButtonRequest,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_command_button_request(request, this_object, calling, condition)
    }

    fn team_face_named(
        &mut self,
        team: &str,
        target: &str,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_team_face_named(team, target, this_object, calling, condition)
    }

    fn named_command(
        &mut self,
        request: ScriptNamedCommand<'_>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(self.world.apply_owned_named_script_command(
            request,
            this_object.map(ObjectId),
            &mut self.ui,
        ))
    }

    fn skirmish_player_exists(
        &self,
        player: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<()> {
        if self.script_player_id(player, current_player).is_some() {
            ScriptOwnerQuery::Present(())
        } else {
            ScriptOwnerQuery::Missing
        }
    }

    fn tech_building_within_distance(
        &self,
        player: &str,
        distance: f32,
        area: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<bool> {
        use gamelogic::common::Relationship;
        let Some(player) = self
            .script_player_id(player, current_player)
            .and_then(|id| self.world.players.get(&id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(area) = self.qualified_area(area, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(trigger) = self
            .world
            .host_trigger_world
            .lock()
            .expect("owned script trigger query")
            .trigger_area_by_name(&area)
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
        action: gamelogic::scripting::core::ScriptActionType,
        seconds: i32,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.receive_named_special_power_countdown(name, power_name, action, seconds, this_object)
    }

    fn named_damage(
        &mut self,
        name: &str,
        amount: i32,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        // A foreign Core registry/tracker entry must never select the receiver.
        let id = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId));
        if let Some(id) = id {
            self.world.host_script_damage_with_interpreter_with_effects(
                id,
                amount as f32,
                false,
                self.interpreter,
                &mut self.ui,
            );
        }
        Some(Ok(()))
    }

    fn water(&mut self, request: ScriptWaterRequest<'_>) -> Option<gamelogic::GameLogicResult<()>> {
        Some(
            self.world
                .apply_owned_script_water_with_effects(request, &mut self.ui),
        )
    }

    fn map_visibility_all(
        &mut self,
        operation: gamelogic::scripting::engine::ScriptMapVisibilityOperation,
        player_name: &str,
        current_player: Option<&str>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let player_id = if player_name.is_empty() {
            None
        } else {
            self.script_player_id(player_name, current_player)
        };
        // Core's find_player_by_name branch returns after a match; a failed
        // non-empty lookup falls through to the same all-human behavior as an
        // omitted parameter. script_player_id also handles LocalPlayer,
        // ThisPlayer, ThisPlayerEnemy, and the challenge-only ThePlayer alias.
        let mut player_ids: Vec<u32> = match player_id {
            Some(player_id) => vec![player_id],
            None => self
                .world
                .players
                .values()
                .filter(|player| player.is_human)
                .map(|player| player.id)
                .collect(),
        };
        player_ids.sort_unstable();
        let Ok(mut shroud) = self.world.world_services.shroud().lock() else {
            return Some(Err(gamelogic::GameLogicError::Threading(
                "Failed to lock receiving ShroudManager".to_string(),
            )));
        };
        for player_id in player_ids {
            // C++ ignores individual invalid-player results after resolving the
            // recipient; valid Main player IDs are already range-checked on admission.
            use gamelogic::scripting::engine::ScriptMapVisibilityOperation as Op;
            match operation {
                Op::Reveal => {
                    let _ = shroud.reveal_map_for_player(player_id);
                }
                Op::RevealPermanently => {
                    let _ = shroud.reveal_map_for_player_permanently(player_id);
                }
                Op::UndoPermanentReveal => {
                    let _ = shroud.undo_reveal_map_for_player_permanently(player_id);
                }
                Op::Shroud => {
                    let _ = shroud.shroud_map_for_player(player_id);
                }
            }
        }
        Some(Ok(()))
    }

    fn ai_player(
        &mut self,
        request: ScriptAiPlayerRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
                self.world
                    .apply_owned_script_build_nearest(player, building, team, calling, condition);
            }
            ScriptAiPlayerRequest::BaseDefense {
                player,
                structure,
                flank,
            } => {
                self.world
                    .apply_owned_script_base_defense(player, structure, flank);
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
            } => {
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

    fn camera_waypoint_position(
        &mut self,
        name: &str,
    ) -> Option<gamelogic::GameLogicResult<Option<gamelogic::common::Coord3D>>> {
        Some(
            self.world
                .world_services
                .terrain()
                .read()
                .map(|terrain| {
                    // CPP doCameraMotionBlurJump copies raw Waypoint::location;
                    // the general host waypoint helper changes ground Z.
                    terrain
                        .get_waypoint_by_name(&name.into())
                        .map(|waypoint| *waypoint.get_location())
                })
                .map_err(|_| {
                    gamelogic::GameLogicError::Threading(
                        "receiving camera waypoint terrain poisoned".into(),
                    )
                }),
        )
    }

    fn camera_movement_finished(&mut self) -> Option<bool> {
        Some(self.world.camera_movement_finished())
    }

    fn world(
        &mut self,
        request: gamelogic::scripting::engine::ScriptWorldRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        use gamelogic::scripting::engine::ScriptWorldRequest;
        // These effects belong to the receiving match, regardless of which
        // interpreter/service adapter invoked it. The existing after_action
        // drain makes them observable before the next authored statement.
        match request {
            ScriptWorldRequest::RadarRefreshTerrain => {
                self.world.host_radar_refresh_terrain();
                return Some(Ok(()));
            }
            ScriptWorldRequest::Music {
                track,
                fade_out,
                fade_in,
            } => {
                let stopped = self
                    .audio(
                        gamelogic::scripting::engine::ScriptAudioRequest::StopMusic {
                            fade: fade_out,
                        },
                    )
                    .unwrap_or(Ok(()));
                if let Err(error) = stopped {
                    return Some(Err(error));
                }
                let mut event = game_engine::common::audio::AudioEventRts::with_event_name(track);
                event.set_should_fade(fade_in);
                let player = self
                    .world
                    .local_player_id()
                    .map(|id| id as i32)
                    .unwrap_or(-1);
                event.set_player_index(player);
                return Some(self.admit_owned_script_audio(&event, player));
            }
            ScriptWorldRequest::Freeze(frozen) => {
                self.world.mission_scripts.push_script_freeze_time(frozen);
                return Some(Ok(()));
            }
            ScriptWorldRequest::VisualSpeed(multiplier) => {
                self.world.mission_scripts.push_visual_speed_multiplier(
                    crate::game_logic::mission_scripts::VisualSpeedMultiplierRequest { multiplier },
                );
                return Some(Ok(()));
            }
            ScriptWorldRequest::Weather(visible) => {
                self.world.mission_scripts.push_weather_visible(visible);
                return Some(Ok(()));
            }
            _ => {}
        }
        if let ScriptWorldRequest::Input { enabled } = request {
            if let Some(ui) = self.ui.ui.as_mut() {
                if !ui.has_cinematic_input() {
                    return Some(Err(gamelogic::GameLogicError::Configuration(
                        "native presentation lacks receiving cinematic input capability".into(),
                    )));
                }
                let was_enabled = self.world.script_input_enabled();
                self.world.set_script_input_enabled(enabled);
                let world = &mut self.world;
                return Some(ui.cinematic_input(enabled, was_enabled, &mut |player| {
                    world.select_objects_no_sound(player, Vec::new());
                }));
            }
            if self.ui.view.is_some() {
                return Some(Err(gamelogic::GameLogicError::Configuration(
                    "native view operation cannot fall back to ambient cinematic input".into(),
                )));
            }
        }
        let handler = self
            .interpreter
            .or(self.world.owned_script_engine.as_ref())
            .and_then(|engine| engine.action_handler());
        match request {
            ScriptWorldRequest::Input { enabled } => self.world.set_script_input_enabled(enabled),
            ScriptWorldRequest::RadarForced(forced) => self.world.set_radar_forced(forced),
            ScriptWorldRequest::RadarRefreshTerrain => unreachable!("already admitted above"),
            _ => {}
        }
        // Clone the explicit interpreter's external service adapter before
        // callbacks. No engine/store borrow survives a synchronous reentry.
        Some(if let Some(handler) = handler {
            match request {
                ScriptWorldRequest::Input { enabled } => handler.input_enabled(enabled),
                ScriptWorldRequest::MovieFullscreen(name) => handler.movie_play_fullscreen(name),
                ScriptWorldRequest::RadarForced(forced) => handler.set_radar_forced(forced),
                ScriptWorldRequest::RadarRefreshTerrain => {
                    unreachable!("already admitted above")
                }
                ScriptWorldRequest::Timer {
                    name,
                    text,
                    countdown,
                } => handler.add_named_timer(name, text, countdown),
                // Already admitted to the receiving hooks above.
                ScriptWorldRequest::Freeze(_)
                | ScriptWorldRequest::VisualSpeed(_)
                | ScriptWorldRequest::Weather(_) => Ok(()),
                ScriptWorldRequest::Music { .. } => unreachable!("native music admitted above"),
            }
        } else {
            Ok(())
        })
    }

    fn camera(
        &mut self,
        request: ScriptCameraRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        match request {
            ScriptCameraRequest::ScreenShake { intensity } => {
                self.world.admit_script_screen_shake(intensity);
                Some(Ok(()))
            }
            _ => Some(self.world.mission_scripts.apply_camera_request(request)),
        }
    }

    fn display(
        &mut self,
        request: ScriptDisplayRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        // Queue on this world, at the same drain boundary as the legacy
        // callback. Do not use an engine's retained presentation owner.
        match request {
            ScriptDisplayRequest::Text(text) => {
                self.world.mission_scripts.push_message(text.into())
            }
            ScriptDisplayRequest::Cinematic {
                text,
                font,
                duration_seconds,
            } => {
                self.world.mission_scripts.push_cinematic_text(
                    text.into(),
                    font.into(),
                    duration_seconds,
                );
            }
            ScriptDisplayRequest::MilitaryCaption { text, duration_ms } => {
                self.world
                    .mission_scripts
                    .push_military_caption(text.into(), duration_ms);
            }
        }
        Some(Ok(()))
    }

    fn create_reinforcements(
        &mut self,
        team: &str,
        waypoint: &str,
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(self.world.merge_owned_script_team_with_effects(
            source,
            destination,
            calling,
            condition,
            self.interpreter,
            &mut self.ui,
        ))
    }

    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
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
        &self,
        player: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<(i32, gamelogic::scripting::HostScriptPlayerCensus)> {
        let Some(id) = self.script_player_id(player, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(player) = self.world.players.get(&id) else {
            return ScriptOwnerQuery::Missing;
        };
        match self
            .world
            .owned_host_player_census()
            .remove(&player.map_side.map_player_name)
        {
            Some(census) => ScriptOwnerQuery::Present((id as i32, census)),
            None => ScriptOwnerQuery::Missing,
        }
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

    fn object_status(&self, id: u32) -> ScriptOwnerQuery<ScriptObjectStatus> {
        self.world
            .host_object(ObjectId(id))
            .map(Self::status)
            .map(ScriptOwnerQuery::Present)
            .unwrap_or(ScriptOwnerQuery::Missing)
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
    ) -> Option<gamelogic::GameLogicResult<()>> {
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
                .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
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
        ScriptOwnerQuery::Present(gamelogic::scripting::engine::script_team_is_contained(
            members.into_iter().map(|id| {
                self.world
                    .host_object(ObjectId(id))
                    .map(|object| object.contained_by.is_some())
            }),
            all,
        ))
    }

    fn set_named_train_held(&mut self, name: &str, held: bool) -> Option<()> {
        if let Some(id) = self.world.host_object_id_by_script_name(name) {
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
        let contextual = name == gamelogic::scripting::core::THIS_TEAM
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
        &self,
        transition: gamelogic::scripting::engine::ScriptTeamAreaTransition,
        team: &str,
        area: &str,
        current_player: Option<&str>,
        calling: Option<u32>,
        condition: Option<u32>,
        surfaces_allowed: u32,
    ) -> ScriptOwnerQuery<bool> {
        use gamelogic::scripting::engine::ScriptTeamAreaTransition as Transition;
        let Some((team_id, members)) = self
            .world
            .resolve_owned_script_team(team, calling, condition)
            .expect("owned team trigger transition query")
        else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(area_name) = self.qualified_area(area, current_player) else {
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
        let surfaces = (surfaces_allowed & 0x01) | ((surfaces_allowed & 0x02) << 2);
        let (mut any_considered, mut any_entered, mut any_outside) = (false, false, false);
        let (mut any_exited, mut any_inside) = (false, false);
        for id in members {
            let Some(object) = self.world.host_object(ObjectId(id)) else {
                continue;
            };
            let object_surfaces = if object.has_ai_update_interface() {
                object.locomotor_surfaces
            } else {
                gamelogic::path::SURFACE_GROUND
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

    fn bridge_status(
        &self,
        name: &str,
        this_object: Option<u32>,
    ) -> ScriptOwnerQuery<ScriptBridgeStatus> {
        // Resolve only this world's name/ID pair. The shared named tracker may
        // contain the same ID from a foreign world and is not an owner.
        let Some(object) = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
            .and_then(|id| self.world.host_object(id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        let terrain = self
            .world
            .world_services
            .terrain()
            .read()
            .expect("owned script bridge query");
        let changed = terrain.bridge_damage_states_changed();
        ScriptOwnerQuery::Present(ScriptBridgeStatus {
            broken: changed && terrain.is_bridge_broken(object.id.0),
            repaired: changed && terrain.is_bridge_repaired(object.id.0),
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
mod map_visibility_owner_tests {
    use super::*;

    fn world() -> GameLogic {
        let mut world = GameLogic::new();
        world.add_player(
            crate::game_logic::Player::new(1, crate::game_logic::Team::USA, "Human", true)
                .with_script_identity("AuthoredPlayer"),
        );
        world
    }

    fn queue_permanent_reveal(world: &GameLogic, player: u32) {
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .reveal_map_for_player_permanently(player)
            .unwrap();
    }

    #[test]
    fn named_map_shroud_mutates_only_the_receiving_world() {
        let mut owner = world();
        let peer = world();
        queue_permanent_reveal(&owner, 1);
        queue_permanent_reveal(&peer, 1);

        {
            let mut driver = HostScriptExecutionDriver::new(&mut owner);
            assert!(
                driver
                    .map_visibility_all(
                        gamelogic::scripting::engine::ScriptMapVisibilityOperation::Shroud,
                        "AuthoredPlayer",
                        None,
                    )
                    .is_some()
            );
        }

        assert!(
            owner
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state()
                .pending_permanent_reveal_players
                .is_empty()
        );
        assert_eq!(
            peer.world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state()
                .pending_permanent_reveal_players,
            vec![1]
        );
    }

    #[test]
    fn unnamed_map_shroud_targets_human_players_only() {
        let mut owner = world();
        owner.add_player(crate::game_logic::Player::new(
            2,
            crate::game_logic::Team::China,
            "AI",
            false,
        ));
        queue_permanent_reveal(&owner, 1);
        queue_permanent_reveal(&owner, 2);

        {
            let mut driver = HostScriptExecutionDriver::new(&mut owner);
            assert!(
                driver
                    .map_visibility_all(
                        gamelogic::scripting::engine::ScriptMapVisibilityOperation::Shroud,
                        "",
                        None,
                    )
                    .is_some()
            );
        }

        let pending = owner
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .snapshot_state()
            .pending_permanent_reveal_players;
        assert_eq!(pending, vec![2]);
    }

    #[test]
    fn map_reveal_operations_mutate_only_the_receiving_world() {
        use gamelogic::scripting::engine::ScriptMapVisibilityOperation as Op;

        for operation in [Op::Reveal, Op::RevealPermanently, Op::UndoPermanentReveal] {
            let mut owner = world();
            let peer = world();
            if matches!(operation, Op::UndoPermanentReveal) {
                queue_permanent_reveal(&owner, 1);
                queue_permanent_reveal(&peer, 1);
            }

            assert!(
                HostScriptExecutionDriver::new(&mut owner)
                    .map_visibility_all(operation, "AuthoredPlayer", None)
                    .is_some()
            );

            let owner_state = owner
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state();
            let peer_state = peer
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state();
            match operation {
                Op::Reveal => {
                    assert_eq!(owner_state.pending_full_reveal_players, vec![1]);
                    assert!(peer_state.pending_full_reveal_players.is_empty());
                }
                Op::RevealPermanently => {
                    assert_eq!(owner_state.pending_permanent_reveal_players, vec![1]);
                    assert!(peer_state.pending_permanent_reveal_players.is_empty());
                }
                Op::UndoPermanentReveal => {
                    assert!(owner_state.pending_permanent_reveal_players.is_empty());
                    assert_eq!(peer_state.pending_permanent_reveal_players, vec![1]);
                }
                Op::Shroud => unreachable!(),
            }
        }
    }

    #[test]
    fn map_shroud_resolves_script_player_aliases_in_the_receiving_world() {
        use gamelogic::scripting::core::{
            LOCAL_PLAYER, THE_PLAYER, THIS_PLAYER, THIS_PLAYER_ENEMY,
        };

        let mut owner = world();
        owner.add_player(
            crate::game_logic::Player::new(2, crate::game_logic::Team::China, "Current AI", false)
                .with_script_identity("CurrentAI"),
        );
        owner.add_player(
            crate::game_logic::Player::new(3, crate::game_logic::Team::GLA, "Enemy", true)
                .with_script_identity("Enemy"),
        );
        owner.add_player(
            crate::game_logic::Player::new(4, crate::game_logic::Team::USA, "ThePlayer", true)
                .with_script_identity(THE_PLAYER),
        );
        owner.ai_manager.add_ai_player(
            2,
            crate::game_logic::Team::China,
            crate::ai::AIDifficulty::Medium,
        );
        owner
            .ai_manager
            .ai_players
            .get_mut(&2)
            .unwrap()
            .enemy_player_id = Some(3);
        owner.admit_challenge_session(true);

        for id in [1, 2, 3, 4] {
            queue_permanent_reveal(&owner, id);
        }
        assert!(
            HostScriptExecutionDriver::new(&mut owner)
                .map_visibility_all(
                    gamelogic::scripting::engine::ScriptMapVisibilityOperation::Shroud,
                    LOCAL_PLAYER,
                    None,
                )
                .is_some()
        );
        assert_eq!(pending_reveals(&owner), vec![2, 3, 4]);
        assert!(
            HostScriptExecutionDriver::new(&mut owner)
                .map_visibility_all(
                    gamelogic::scripting::engine::ScriptMapVisibilityOperation::Shroud,
                    THIS_PLAYER,
                    Some("CurrentAI"),
                )
                .is_some()
        );
        assert_eq!(pending_reveals(&owner), vec![3, 4]);
        assert!(
            HostScriptExecutionDriver::new(&mut owner)
                .map_visibility_all(
                    gamelogic::scripting::engine::ScriptMapVisibilityOperation::Shroud,
                    THIS_PLAYER_ENEMY,
                    Some("CurrentAI"),
                )
                .is_some()
        );
        assert_eq!(pending_reveals(&owner), vec![4]);
        queue_permanent_reveal(&owner, 1);
        assert!(
            HostScriptExecutionDriver::new(&mut owner)
                .map_visibility_all(
                    gamelogic::scripting::engine::ScriptMapVisibilityOperation::Shroud,
                    THE_PLAYER,
                    Some("CurrentAI"),
                )
                .is_some()
        );

        assert_eq!(pending_reveals(&owner), vec![4]);
    }

    fn pending_reveals(world: &GameLogic) -> Vec<u32> {
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .snapshot_state()
            .pending_permanent_reveal_players
    }
}

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
    ) -> gamelogic::GameLogicResult<()> {
        engine.update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(self.frame)
                .with_session_world(self.host_trigger_world.clone()),
            &mut HostScriptExecutionDriver::with_interpreter(self, engine),
        )
    }
}

impl GameLogic {
    pub(crate) fn install_owned_script_engine(
        &mut self,
        engine: gamelogic::scripting::engine::ScriptEngine,
    ) {
        self.objects_receive_difficulty_bonus =
            engine.get_objects_should_receive_difficulty_bonus();
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
        let context = gamelogic::scripting::executor::ScriptContext::at_frame(self.frame)
            .with_session_world(self.host_trigger_world.clone());
        if let Err(error) = engine.run_object_script_with_driver(
            name,
            object.0,
            context,
            &mut HostScriptExecutionDriver::with_effects(self, engine, ui.reborrow()),
        ) {
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
            audio: None,
            terrain: None,
            ui: ui
                .as_mut()
                .map(|ui| &mut **ui as &mut dyn crate::game_logic::ScriptUiEffects),
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
    pub(crate) fn evaluate_owned_script_conditions(
        &mut self,
        engine: &gamelogic::scripting::engine::ScriptEngine,
        script: &mut gamelogic::scripting::core::Script,
        player_name: Option<&str>,
    ) -> bool {
        let mut context = gamelogic::scripting::executor::ScriptContext::at_frame(self.frame)
            .with_session_world(self.host_trigger_world.clone());
        context.host_trigger_world = self.host_trigger_world.clone();
        let mut driver = HostScriptExecutionDriver::with_interpreter(self, engine);
        engine.evaluate_conditions_with_driver(script, None, player_name, context, &mut driver)
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
