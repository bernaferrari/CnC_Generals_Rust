//! Apply authored waypoint circles to the receiving terrain and shroud.
use super::HostScriptExecutionDriver;
use crate::game_logic::GameLogic;
use generals_sim::scripting::map_reveals::{NamedReveal, NamedRevealOperation, WaypointVisibility};
use generals_sim::{GameLogicError, GameLogicResult};

impl HostScriptExecutionDriver<'_> {
    pub(super) fn receiving_map_waypoint(&self, name: &str) -> GameLogicResult<Option<glam::Vec3>> {
        let waypoint = gamelogic::common::AsciiString::from(name);
        let terrain =
            self.world.world_services.terrain().read().map_err(|_| {
                GameLogicError::Threading("Failed to lock receiving terrain".into())
            })?;
        Ok(terrain
            .get_waypoint_by_name(&waypoint)
            .map(|waypoint| *waypoint.get_location()))
    }

    pub(super) fn apply_receiving_waypoint_visibility(
        &mut self,
        operation: WaypointVisibility,
        waypoint: &str,
        radius: f32,
        player: &str,
        current_player: Option<&str>,
    ) -> GameLogicResult<()> {
        let Some(position) = self.receiving_map_waypoint(waypoint)? else {
            return Ok(());
        };
        let player_id = if player.is_empty() {
            None
        } else {
            self.query_script_player_id(player, current_player)
        };
        let mask = if let Some(id) = player_id {
            1u32.checked_shl(id).unwrap_or(0)
        } else {
            self.world
                .players
                .values()
                .filter(|player| player.is_human)
                .filter_map(|player| 1u32.checked_shl(player.id))
                .fold(0, |mask, player_bit| mask | player_bit)
        };
        let mut shroud = self.world.world_services.shroud().lock().map_err(|_| {
            GameLogicError::Threading("Failed to lock receiving ShroudManager".into())
        })?;
        shroud.apply_temporary_waypoint_visibility(operation, &position, radius, mask);
        Ok(())
    }

    pub(super) fn apply_receiving_named_reveal(
        &mut self,
        operation: NamedRevealOperation,
        reveal: &NamedReveal,
        current_player: Option<&str>,
    ) -> GameLogicResult<()> {
        let Some(position) = self.receiving_map_waypoint(&reveal.waypoint_name)? else {
            return Ok(());
        };
        let Some(mask) = self
            .query_script_player_id(&reveal.player_name, current_player)
            .and_then(|id| 1u32.checked_shl(id))
        else {
            return Ok(());
        };
        let mut shroud = self.world.world_services.shroud().lock().map_err(|_| {
            GameLogicError::Threading("Failed to lock receiving ShroudManager".into())
        })?;
        match operation {
            NamedRevealOperation::Reveal => {
                shroud.do_shroud_reveal(&position, reveal.radius_to_reveal, mask)
            }
            NamedRevealOperation::Undo => {
                shroud.undo_shroud_reveal(&position, reveal.radius_to_reveal, mask)
            }
        }
        Ok(())
    }
}

impl GameLogic {
    pub(crate) fn recover_legacy_named_reveal_lookers(&mut self) -> GameLogicResult<()> {
        self.with_owned_script_engine(|engine, world| {
            engine.reapply_named_reveals_with_driver(&mut HostScriptExecutionDriver::new(world))
        })
        .unwrap_or(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team};
    use crate::save_load::SnapshotBuilder;
    use crate::save_load::snapshot::persist_v18::restore_persist_v18;
    use game_engine::map_data::{MapData, MapWaypoint};
    use gamelogic::scripting::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    };

    const AUTHORED_SIDE: &str = "AuthoredPlayer";

    fn world(location: f32) -> GameLogic {
        let mut world = GameLogic::new();
        let mut player = Player::new(1, Team::USA, "Display Player", true);
        player.map_side.map_player_name = AUTHORED_SIDE.into();
        world.add_player(player);
        world
            .apply_camera_movement_overrides(&["GameData\n PartitionCellSize = 40\n End\n".into()])
            .unwrap();
        let mut map = MapData::new();
        map.width = 64;
        map.height = 64;
        map.heightmap = vec![0; 64 * 64];
        map.boundaries.push(glam::IVec2::new(63, 63));
        for (id, name, x) in [(1, "Look", location), (2, "Other", 350.0)] {
            map.waypoints.push(MapWaypoint {
                id,
                name: name.into(),
                location: glam::Vec3::new(x, x, 0.0),
                path_label1: String::new(),
                path_label2: String::new(),
                path_label3: String::new(),
                bi_directional: false,
            });
        }
        world
            .world_services
            .terrain()
            .write()
            .unwrap()
            .load_map_data(map);
        world.initialize_receiving_partition_grid().unwrap();
        world
    }

    fn reveal(waypoint: &str, player: &str) -> ScriptAction {
        let mut action = ScriptAction::new(ScriptActionType::MapRevealPermanentlyAtWaypoint);
        for param in [
            Parameter::with_string(ParameterType::Waypoint, waypoint.into()),
            Parameter::with_real(ParameterType::Real, 1.0),
            Parameter::with_string(ParameterType::Side, player.into()),
            Parameter::with_string(ParameterType::Revealname, "PermanentLook".into()),
        ] {
            action.add_parameter(param).unwrap();
        }
        action
    }

    fn undo() -> ScriptAction {
        let mut action = ScriptAction::new(ScriptActionType::MapUndoRevealPermanentlyAtWaypoint);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Revealname,
                "PermanentLook".into(),
            ))
            .unwrap();
        action
    }

    fn install(world: &GameLogic, action: ScriptAction) {
        let mut condition = OrCondition::new();
        condition
            .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
        let mut script = Script::new();
        script.script_name = "RevealFixture".into();
        script.is_active = true;
        script.is_one_shot = true;
        script.condition = Some(Box::new(condition));
        script.action = Some(Box::new(action));
        let mut scripts = ScriptList::new();
        scripts.append_script(Box::new(script));
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .set_script_list_for_player(1, Some(Box::new(scripts)))
            .unwrap();
    }

    fn execute(world: &mut GameLogic, action: ScriptAction) {
        install(world, action);
        world
            .with_owned_script_engine(|engine, world| {
                world.update_local_script_engine_with_owner(engine).unwrap();
            })
            .unwrap();
    }

    fn counters(world: &GameLogic) -> Vec<i32> {
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .snapshot_state()
            .grid
            .unwrap()
            .cells
            .into_iter()
            .map(|cell| cell.current_shroud[1])
            .collect()
    }

    #[test]
    fn authored_named_reveal_uses_the_receiving_world_and_keeps_first_definition() {
        let mut owner = world(32.0);
        let mut peer = world(208.0);
        let peer_before = counters(&peer);
        peer.with_own_services(|| {
            execute(
                &mut owner,
                reveal("Look", generals_sim::scripting::program::LOCAL_PLAYER),
            );
        });
        let once = counters(&owner);
        assert_eq!(once.iter().min(), Some(&-1));
        assert_eq!(counters(&peer), peer_before);

        execute(&mut owner, reveal("Other", "MissingSide"));
        let twice = counters(&owner);
        for (before, after) in once.iter().zip(twice) {
            assert_eq!(after, if *before == -1 { -2 } else { *before });
        }

        execute(&mut peer, reveal("Look", AUTHORED_SIDE));
        assert_eq!(counters(&peer).iter().min(), Some(&-1));
        assert_ne!(counters(&peer), once);
    }

    #[test]
    fn authored_temporary_waypoint_reveal_explores_only_the_receiving_players_cells() {
        let mut owner = world(32.0);
        let peer = world(208.0);
        let before = counters(&owner);
        let peer_before = counters(&peer);
        let mut action = ScriptAction::new(ScriptActionType::MapRevealAtWaypoint);
        for param in [
            Parameter::with_string(ParameterType::Waypoint, "Look".into()),
            Parameter::with_real(ParameterType::Real, 1.0),
            Parameter::with_string(
                ParameterType::Side,
                generals_sim::scripting::program::LOCAL_PLAYER.into(),
            ),
        ] {
            action.add_parameter(param).unwrap();
        }

        peer.with_own_services(|| execute(&mut owner, action));

        assert_ne!(counters(&owner), before);
        assert_eq!(counters(&owner).iter().min(), Some(&0));
        assert_eq!(counters(&peer), peer_before);
    }

    #[test]
    fn named_reveal_save_restores_exact_lookers_and_undo_removes_one() {
        let mut owner = world(32.0);
        execute(&mut owner, reveal("Look", AUTHORED_SIDE));
        let active = counters(&owner);
        assert_eq!(active.iter().min(), Some(&-1));
        let builder = SnapshotBuilder::new();
        let saved = builder.create_world_snapshot(&owner).unwrap();
        assert_eq!(saved.persist_v18.script_named_reveals.len(), 1);

        let mut restored = world(32.0);
        install(&restored, reveal("Look", AUTHORED_SIDE));
        builder
            .restore_from_snapshot(&saved, &mut restored)
            .unwrap();
        assert_eq!(counters(&restored), active);
        execute(&mut restored, undo());
        assert_eq!(counters(&restored).iter().min(), Some(&0));

        // The v18 adapter receives definitions without a grid or scalar history.
        let mut recovered = world(32.0);
        restore_persist_v18(&saved.persist_v18, &mut recovered).unwrap();
        assert_eq!(counters(&recovered), active);
    }
}
