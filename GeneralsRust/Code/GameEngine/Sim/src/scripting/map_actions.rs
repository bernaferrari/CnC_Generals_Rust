//! Authored radar and map visibility actions from ScriptActions.cpp.

use super::interpreter::ScriptInterpreter;
use super::map_reveals::{NamedReveal, NamedRevealOperation, WaypointVisibility};
use super::program::{ScriptAction, ScriptActionType};
use super::radar::RadarEventTarget;
use super::world::{ScriptMapVisibilityOperation, ScriptWorldRequest};
use super::{ScriptActionResult, ScriptError};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::system::radar::RadarEventType;

pub trait ScriptMapWorld {
    fn radar_event(
        &mut self,
        target: RadarEventTarget<'_>,
        kind: RadarEventType,
        interpreter: &ScriptInterpreter,
    ) -> GameLogicResult<()>;
    fn radar_visibility(&mut self, enabled: bool) -> GameLogicResult<()>;
    fn world(&mut self, request: ScriptWorldRequest<'_>) -> GameLogicResult<()>;
    fn map_visibility_all(
        &mut self,
        operation: ScriptMapVisibilityOperation,
        player: &str,
        interpreter: &ScriptInterpreter,
    ) -> GameLogicResult<()>;
    fn waypoint_visibility(
        &mut self,
        operation: WaypointVisibility,
        waypoint: &str,
        radius: f32,
        player: &str,
        interpreter: &ScriptInterpreter,
    ) -> GameLogicResult<()>;
    fn named_map_visibility(
        &mut self,
        operation: NamedRevealOperation,
        reveal: &NamedReveal,
        interpreter: &ScriptInterpreter,
    ) -> GameLogicResult<()>;
    fn border_shroud(&mut self, enabled: bool) -> GameLogicResult<()>;
}

pub fn try_execute_map_action(
    action: &ScriptAction,
    interpreter: &ScriptInterpreter,
    world: &mut (impl ScriptMapWorld + ?Sized),
) -> Result<Option<ScriptActionResult>, ScriptError> {
    use ScriptActionType::*;
    let param = |index| {
        action
            .get_parameter(index)
            .ok_or_else(|| ScriptError::missing_parameter(index))
    };
    let string = |index| param(index).map(|value| value.get_string());
    let kind = action.get_action_type();
    let effect = match kind {
        RadarCreateEvent | ObjectCreateRadarEvent | TeamCreateRadarEvent => {
            let target = match kind {
                RadarCreateEvent => RadarEventTarget::Position(param(0)?.get_coord()),
                ObjectCreateRadarEvent => RadarEventTarget::Object(string(0)?),
                _ => RadarEventTarget::Team(string(0)?),
            };
            world.radar_event(
                target,
                RadarEventType::from_ordinal(param(1)?.get_int()),
                interpreter,
            )
        }
        RadarEnable | RadarDisable => {
            warn_on_error(kind, world.radar_visibility(kind == RadarEnable));
            Ok(())
        }
        RadarForceEnable | RadarRevertToNormal => {
            let request = ScriptWorldRequest::RadarForced(kind == RadarForceEnable);
            warn_on_error(kind, world.world(request));
            Ok(())
        }
        RefreshRadar => world.world(ScriptWorldRequest::RadarRefreshTerrain),
        MapSwitchBorder => world.world(ScriptWorldRequest::MapSwitchBoundary(param(0)?.get_int())),
        EnableBorderShroud | DisableBorderShroud => {
            warn_on_error(kind, world.border_shroud(kind == EnableBorderShroud));
            Ok(())
        }
        MapRevealAtWaypoint | MapShroudAtWaypoint => {
            let operation = if kind == MapRevealAtWaypoint {
                WaypointVisibility::Reveal
            } else {
                WaypointVisibility::Shroud
            };
            world.waypoint_visibility(
                operation,
                string(0)?,
                param(1)?.get_real(),
                string(2)?,
                interpreter,
            )
        }
        MapRevealAll | MapRevealAllPerm | MapRevealAllUndoPerm | MapShroudAll => {
            use ScriptMapVisibilityOperation as Op;
            let operation = match kind {
                MapRevealAll => Op::Reveal,
                MapRevealAllPerm => Op::RevealPermanently,
                MapRevealAllUndoPerm => Op::UndoPermanentReveal,
                _ => Op::Shroud,
            };
            world.map_visibility_all(operation, string(0)?, interpreter)
        }
        MapRevealPermanentlyAtWaypoint => {
            let (waypoint, radius, player, name) =
                (string(0)?, param(1)?.get_real(), string(2)?, string(3)?);
            interpreter.register_named_reveal(name, waypoint, radius, player);
            let reveal = interpreter.named_reveal(name);
            reveal.map_or(Ok(()), |reveal| {
                world.named_map_visibility(NamedRevealOperation::Reveal, &reveal, interpreter)
            })
        }
        MapUndoRevealPermanentlyAtWaypoint => {
            let name = string(0)?;
            let effect = interpreter.named_reveal(name).map_or(Ok(()), |reveal| {
                world.named_map_visibility(NamedRevealOperation::Undo, &reveal, interpreter)
            });
            if effect.is_ok() {
                interpreter.remove_named_reveal(name);
            }
            effect
        }
        _ => return Ok(None),
    };
    effect.map_err(|error: GameLogicError| ScriptError::ExecutionFailed(error.to_string()))?;
    Ok(Some(ScriptActionResult::Success))
}

fn warn_on_error(kind: ScriptActionType, result: GameLogicResult<()>) {
    if let Err(error) = result {
        log::warn!("Script map action {kind:?} failed: {error}");
    }
}
