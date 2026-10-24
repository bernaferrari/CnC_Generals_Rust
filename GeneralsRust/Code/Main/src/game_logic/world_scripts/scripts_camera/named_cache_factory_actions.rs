//! Synchronous receiving named ScriptActions; standalone Core fallback lives in Core.
use super::*;

impl<'a> HostScriptExecutionDriver<'a> {
    pub(super) fn receive_create_named_object(
        &mut self,
        name: Option<&str>,
        request: gamelogic::scripting::engine::ScriptObjectCreation<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.world.host_script_create_object_with_effects(
            name,
            request.template,
            request.team,
            request.x,
            request.y,
            request.z,
            request.angle,
            self.interpreter,
            &mut self.ui,
        );
        Some(Ok(()))
    }

    pub(super) fn receive_create_named_object_at_waypoint(
        &mut self,
        name: Option<&str>,
        template: &str,
        team: &str,
        waypoint: &str,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let position = match self.world.world_services.terrain().read() {
            Ok(terrain) => terrain
                .get_waypoint_by_name(&waypoint.into())
                .map(|waypoint| *waypoint.get_location()),
            Err(_) => {
                return Some(Err(gamelogic::GameLogicError::Threading(
                    "receiving named-create terrain poisoned".into(),
                )));
            }
        };
        // Source createUnitOnTeamAt uses the origin when the waypoint is absent.
        let position = position.unwrap_or_default();
        self.world.host_script_create_object_with_effects(
            name,
            template,
            team,
            position.x,
            position.y,
            position.z,
            0.0,
            self.interpreter,
            &mut self.ui,
        );
        Some(Ok(()))
    }
}
