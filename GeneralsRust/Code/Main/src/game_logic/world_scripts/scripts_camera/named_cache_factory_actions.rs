//! Synchronous receiving named ScriptActions; standalone Core fallback lives in Core.
use super::*;
use generals_sim::scripting::world::ScriptObjectCreation;

impl<'a> HostScriptExecutionDriver<'a> {
    pub(super) fn receive_create_named_object(
        &mut self,
        name: &str,
        request: ScriptObjectCreation<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        self.world.apply_owned_script_spawn_with_effects(
            Some(name),
            request,
            calling,
            condition,
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
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<generals_sim::GameLogicResult<()>> {
        let position = match self.world.world_services.terrain().read() {
            Ok(terrain) => terrain
                .get_waypoint_by_name(&waypoint.into())
                .map(|waypoint| *waypoint.get_location()),
            Err(_) => {
                return Some(Err(generals_sim::GameLogicError::Threading(
                    "receiving named-create terrain poisoned".into(),
                )));
            }
        };
        // Source createUnitOnTeamAt uses the origin when the waypoint is absent.
        let position = position.unwrap_or_default();
        self.world.apply_owned_script_spawn_with_effects(
            name,
            ScriptObjectCreation {
                template,
                team,
                x: position.x,
                y: position.y,
                z: position.z,
                angle: 0.0,
            },
            calling,
            condition,
            self.interpreter,
            &mut self.ui,
        );
        Some(Ok(()))
    }
}
