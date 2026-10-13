//! CPP ScriptActions4537-4567: stop, recruit, then merge to the controller's default team.
use super::*;

impl GameLogic {
    pub(super) fn apply_owned_team_stop(
        &mut self,
        raw_team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
        disband: bool,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((source_id, members)) =
            self.resolve_owned_script_team(raw_team, calling, condition)?
        else {
            return Ok(());
        };
        // groupIdle completes before setIsRecruitable and team transfer.
        for id in &members {
            if self
                .host_object(ObjectId(*id))
                .is_some_and(|object| object.has_ai_update_interface() && !object.is_contained())
            {
                self.unit_command_stop_with_source(
                    ObjectId(*id),
                    crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT,
                );
            }
        }
        if !disband {
            return Ok(());
        }
        let controller = {
            let factory = self
                .team_factory
                .lock()
                .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
            let Some(source) = factory.find_team_by_id(source_id) else {
                return Ok(());
            };
            let source = source
                .read()
                .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
            source.get_controlling_player_id()
        };
        for id in &members {
            if let Some(object) = self.host_object_mut(ObjectId(*id)) {
                if object.has_ai_update_interface() {
                    object.is_recruitable = true;
                }
            }
        }
        let Some(player) = controller.and_then(|id| self.players.get(&id)) else {
            return Ok(());
        };
        let default_name = self.default_host_team_instance_name(Some(player.id), player.team);
        let (source, destination) = {
            let mut factory = self
                .team_factory
                .lock()
                .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
            let Some(source) = factory.find_team_by_id(source_id) else {
                return Ok(());
            };
            let Some(destination) = factory.find_team(&default_name) else {
                return Ok(());
            };
            (source, destination)
        };
        if std::sync::Arc::ptr_eq(&source, &destination) {
            return Ok(());
        }
        let team_factory = self.team_factory.clone();
        for id in members {
            let Some(object) = self.host_object_mut(ObjectId(id)) else {
                continue;
            };
            object.team_instance_name = default_name.clone();
            object.apply_named_team_ai_profile(&team_factory, true);

            // No controlling-player change occurs in this operation. The
            // original member iterator observes each transfer before the next.
            source
                .write()
                .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
                .remove_member(id);
            destination
                .write()
                .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
                .add_member(id);
        }
        destination
            .write()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
            .set_active();
        Ok(())
    }
}
