//! Synchronous receiving named ScriptActions; standalone Core fallback lives in Core.
use super::*;

impl<'a> HostScriptExecutionDriver<'a> {
    fn receiving_script_player_name(&self, token: &str) -> Option<String> {
        let current_player = self
            .interpreter
            .and_then(|engine| engine.get_current_player_name());
        let id = self.script_player_id(token, current_player.as_deref())?;
        self.world
            .players
            .get(&id)
            .map(|player| player.map_side.map_player_name.clone())
    }

    pub(super) fn receive_garrison_request(
        &mut self,
        request: gamelogic::scripting::HostScriptGarrisonEnterExitRequest,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        use gamelogic::scripting::HostScriptGarrisonEnterExitRequest as Request;
        let request = match request {
            Request::PlayerGarrisonAll { player } => {
                let Some(player) = self.receiving_script_player_name(&player) else {
                    return Some(Ok(()));
                };
                Request::PlayerGarrisonAll { player }
            }
            Request::PlayerExitAll { player } => {
                let Some(player) = self.receiving_script_player_name(&player) else {
                    return Some(Ok(()));
                };
                Request::PlayerExitAll { player }
            }
            request => request,
        };
        Some(self.world.apply_receiving_garrison_request(
            request,
            this_object.map(ObjectId),
            calling,
            condition,
            &mut self.ui,
        ))
    }

    pub(super) fn receive_named_special_power(
        &mut self,
        request: gamelogic::scripting::HostScriptNamedFireSpecialPowerRequest,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        use gamelogic::scripting::HostScriptNamedFireSpecialPowerRequest as Request;
        let (unit, power, target, waypoint) = match request {
            Request::AtNamed {
                unit,
                power,
                target,
            } => {
                let Some(target) = self
                    .world
                    .owned_named_script_object(&target, this_object.map(ObjectId))
                else {
                    return Some(Ok(()));
                };
                (
                    unit,
                    power,
                    crate::command_system::PowerTarget::Object(target),
                    None,
                )
            }
            Request::AtWaypoint {
                unit,
                power,
                waypoint,
            } => {
                let Some((id, position)) = self.world.host_script_leftover_waypoint(&waypoint)
                else {
                    return Some(Ok(()));
                };
                (
                    unit,
                    power,
                    crate::command_system::PowerTarget::Location(position),
                    Some(id),
                )
            }
        };
        if let Some(id) = self
            .world
            .owned_named_script_object(&unit, this_object.map(ObjectId))
        {
            let interpreter = self
                .interpreter
                .expect("named special-power action retains interpreter");
            self.world.fire_receiving_named_special_power(
                id,
                &power,
                target,
                waypoint,
                Some(interpreter),
                &mut self.ui,
            );
        }
        Some(Ok(()))
    }

    pub(super) fn receive_command_button_request(
        &mut self,
        request: gamelogic::scripting::HostScriptUseCommandButtonRequest,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let interpreter = self
            .interpreter
            .expect("named command-button action retains interpreter");
        self.world.apply_receiving_command_button_request(
            request,
            this_object.map(ObjectId),
            calling,
            condition,
            interpreter,
            &mut self.ui,
        );
        Some(Ok(()))
    }

    pub(super) fn receive_team_face_named(
        &mut self,
        team: &str,
        target: &str,
        this_object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let result = (|| -> gamelogic::GameLogicResult<()> {
            let Some(target) = self
                .world
                .owned_named_script_object(target, this_object.map(ObjectId))
            else {
                return Ok(());
            };
            let Some(position) = self
                .world
                .host_object(target)
                .filter(|object| object.is_alive())
                .map(|object| object.get_position())
            else {
                return Ok(());
            };
            if let Some((_, members)) = self
                .world
                .resolve_owned_script_team(team, calling, condition)?
            {
                for member in members {
                    self.world.host_script_face_unit_with_effects(
                        ObjectId(member),
                        position,
                        &mut self.ui,
                    );
                }
            }
            Ok(())
        })();
        Some(result)
    }

    pub(super) fn receive_named_special_power_countdown(
        &mut self,
        name: &str,
        power_name: &str,
        action: gamelogic::scripting::core::ScriptActionType,
        seconds: i32,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let Some(id) = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
        else {
            return Some(Ok(()));
        };
        let Some(power) = crate::command_system::special_power_type_from_template_name(power_name)
        else {
            return Some(Ok(()));
        };
        use gamelogic::scripting::core::ScriptActionType;
        match action {
            ScriptActionType::NamedStopSpecialPowerCountdown => {
                self.world
                    .script_pause_special_power_countdown(id, &power, true);
            }
            ScriptActionType::NamedStartSpecialPowerCountdown => {
                self.world
                    .script_pause_special_power_countdown(id, &power, false);
            }
            ScriptActionType::NamedSetSpecialPowerCountdown => {
                self.world
                    .script_set_special_power_countdown(id, &power, seconds);
            }
            ScriptActionType::NamedAddSpecialPowerCountdown => {
                self.world
                    .script_add_special_power_countdown(id, &power, seconds);
            }
            _ => unreachable!("named countdown driver accepts only countdown actions"),
        }
        Some(Ok(()))
    }
}
