//! Deliver authored radar and map effects to the receiving match.

use super::ScriptActionDispatcher;
use crate::player::PlayerType;
use crate::scripting::engine::ScriptEngine;
use crate::system::engine_stores::with_world_services;
use game_engine::common::common_services::with_common_services;
use game_engine::common::system::radar::{
    Coord3D as RadarPosition, RadarEventType, radar_system_for,
};
use generals_sim::scripting::driver::ScriptExecutionDriver;
use generals_sim::scripting::map_actions::{ScriptMapWorld, execute_map_action};
use generals_sim::scripting::map_reveals::{NamedReveal, NamedRevealOperation, WaypointVisibility};
use generals_sim::scripting::program::ScriptAction;
use generals_sim::scripting::radar::RadarEventTarget;
use generals_sim::scripting::world::{ScriptMapVisibilityOperation, ScriptWorldRequest};
use generals_sim::scripting::{ScriptActionResult, ScriptError};
use generals_sim::{GameLogicError, GameLogicResult};

impl ScriptActionDispatcher<'_> {
    pub(super) fn execute_map_action(
        &self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        execute_map_action(
            action,
            self.context.borrowed_engine().interpreter(),
            &mut MapWorld {
                dispatcher: self,
                driver,
            },
        )
    }
}

struct MapWorld<'request, 'engine> {
    dispatcher: &'request ScriptActionDispatcher<'engine>,
    driver: &'request mut dyn ScriptExecutionDriver<ScriptEngine>,
}

impl MapWorld<'_, '_> {
    fn engine(&self) -> &ScriptEngine {
        self.dispatcher.context.borrowed_engine()
    }

    fn with_legacy_radar_scope<T>(&self, apply: impl FnOnce() -> T) -> T {
        let engine = self.engine();
        // The standalone radar's team, terrain and client adapters still call TheXxx.
        if let Some(world) = engine.owning_world_services() {
            with_world_services(&world, apply)
        } else {
            with_common_services(engine.common_services(), apply)
        }
    }

    fn named_player_key(&self, name: &str) -> Option<u32> {
        if name.is_empty() {
            return None;
        }
        let common = self.engine().common_services();
        let mut names = common
            .name_key_namespace
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        Some(names.name_to_key(name))
    }
}

impl ScriptMapWorld for MapWorld<'_, '_> {
    fn radar_event(
        &mut self,
        target: RadarEventTarget<'_>,
        kind: RadarEventType,
    ) -> GameLogicResult<()> {
        let engine = self.dispatcher.context.borrowed_engine();
        let object = engine.script_object_id();
        if let Some(result) = self.driver.radar_event(
            target,
            kind,
            object,
            engine.get_calling_team_id(),
            engine.get_condition_team_id(),
        ) {
            return result;
        }
        let position = match target {
            RadarEventTarget::Position(position) => Some(position),
            RadarEventTarget::Object(name) => {
                let id = if name == crate::scripting::core::THIS_OBJECT {
                    object
                } else {
                    engine.cached_named_object_id(name)
                };
                id.and_then(|id| engine.standalone_radar_object_position(id))
            }
            RadarEventTarget::Team(name) => engine.owning_world_services().and_then(|world| {
                with_world_services(&world, || {
                    let team = self.dispatcher.get_team_by_name(name).ok()?;
                    let team = team.read().ok()?;
                    team.has_any_units()
                        .then(|| team.get_estimate_team_position())
                        .flatten()
                })
            }),
        };
        if let Some(position) = position
            && let Ok(mut radar) = radar_system_for(engine.common_services()).write()
        {
            radar.observe_frame(
                self.dispatcher
                    .context
                    .with_state(|state| state.current_frame),
            );
            radar.create_event(
                &RadarPosition::new(position.x, position.y, position.z),
                kind,
                4.0,
            );
        }
        Ok(())
    }

    fn radar_visibility(&mut self, enabled: bool) -> GameLogicResult<()> {
        if let Some(result) = self.driver.radar_visibility(enabled) {
            return result;
        }
        if let Ok(mut radar) = radar_system_for(self.engine().common_services()).write() {
            radar.hide(!enabled);
        }
        self.with_legacy_radar_scope(|| {
            self.dispatcher
                .current_script_action_handler()
                .map_or(Ok(()), |handler| handler.set_radar_enabled(enabled))
        })
    }

    fn world(&mut self, request: ScriptWorldRequest<'_>) -> GameLogicResult<()> {
        if let Some(result) = self.driver.world(request) {
            return result;
        }
        let common = self.engine().common_services();
        match request {
            ScriptWorldRequest::RadarForced(forced) => {
                if let Ok(mut radar) = radar_system_for(common).write() {
                    radar.force_on(forced);
                }
                self.with_legacy_radar_scope(|| {
                    self.dispatcher
                        .current_script_action_handler()
                        .map_or(Ok(()), |handler| handler.set_radar_forced(forced))
                })
            }
            ScriptWorldRequest::RadarRefreshTerrain => {
                let radar = radar_system_for(common);
                self.with_legacy_radar_scope(|| {
                    if let Ok(mut radar) = radar.write() {
                        radar.refresh_terrain();
                    }
                });
                Ok(())
            }
            ScriptWorldRequest::MapSwitchBoundary(border) => {
                let world = self.engine().owning_world_services().ok_or_else(|| {
                    GameLogicError::Configuration("boundary script has no world".into())
                })?;
                let logic = world.object_registry().owning_logic().ok_or_else(|| {
                    GameLogicError::Configuration("boundary script has no GameLogic".into())
                })?;
                logic
                    .try_lock()
                    .map_err(|error| {
                        GameLogicError::Threading(format!("boundary GameLogic: {error}"))
                    })?
                    .switch_script_map_boundary(border)
            }
            _ => Err(GameLogicError::Configuration(
                "unexpected map script request".into(),
            )),
        }
    }

    fn map_visibility_all(
        &mut self,
        operation: ScriptMapVisibilityOperation,
        player: &str,
    ) -> GameLogicResult<()> {
        let current = self.engine().get_current_player_name();
        if let Some(result) = self
            .driver
            .map_visibility_all(operation, player, current.as_deref())
        {
            return result;
        }
        let engine = self.engine();
        let Some(world) = engine.owning_world_services() else {
            return Ok(());
        };
        let key = self.named_player_key(player);
        let Ok(players) = engine.players().read() else {
            return Ok(());
        };
        let mut shroud = world.shroud().lock().map_err(|_| {
            GameLogicError::Threading("Failed to lock receiving ShroudManager".into())
        })?;
        if let Some(player) = key.and_then(|key| players.find_player_by_name_key(key)) {
            if let Ok(player) = player.read() {
                shroud.apply_whole_map_visibility(operation, player.get_player_index() as u32);
            }
            return Ok(());
        }
        for player in players.iter().filter_map(|player| player.read().ok()) {
            if player.get_player_type() == PlayerType::Human {
                shroud.apply_whole_map_visibility(operation, player.get_player_index() as u32);
            }
        }
        Ok(())
    }

    fn waypoint_visibility(
        &mut self,
        operation: WaypointVisibility,
        waypoint: &str,
        radius: f32,
        player: &str,
    ) -> GameLogicResult<()> {
        let current = self.engine().get_current_player_name();
        if let Some(result) =
            self.driver
                .waypoint_visibility(operation, waypoint, radius, player, current.as_deref())
        {
            return result;
        }
        let engine = self.engine();
        let Some(position) = engine.authored_waypoint_position(waypoint) else {
            return Ok(());
        };
        let Some(world) = engine.owning_world_services() else {
            return Ok(());
        };
        let key = self.named_player_key(player);
        let Ok(players) = engine.players().read() else {
            return Ok(());
        };
        let mask = if let Some(player) = key.and_then(|key| players.find_player_by_name_key(key)) {
            player
                .read()
                .ok()
                .map_or(0, |player| player.get_player_mask().bits())
        } else {
            players
                .iter()
                .filter_map(|player| player.read().ok())
                .filter(|player| player.get_player_type() == PlayerType::Human)
                .fold(0, |mask, player| mask | player.get_player_mask().bits())
        };
        drop(players);
        if let Ok(mut shroud) = world.shroud().lock() {
            shroud.apply_temporary_waypoint_visibility(operation, &position, radius, mask);
        }
        Ok(())
    }

    fn named_map_visibility(
        &mut self,
        operation: NamedRevealOperation,
        reveal: &NamedReveal,
    ) -> GameLogicResult<()> {
        self.dispatcher
            .context
            .borrowed_engine()
            .deliver_named_reveal(operation, reveal, self.driver)
    }

    fn border_shroud(&mut self, enabled: bool) -> GameLogicResult<()> {
        let data = self
            .engine()
            .common_services()
            .system
            .global_data
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let level = if let Some(data) = data {
            let data = data.read();
            if enabled {
                data.shroud_alpha
            } else {
                data.clear_alpha
            }
        } else if enabled {
            0
        } else {
            255
        };
        self.dispatcher
            .current_script_action_handler()
            .map_or(Ok(()), |handler| handler.set_border_shroud_level(level))
    }
}
