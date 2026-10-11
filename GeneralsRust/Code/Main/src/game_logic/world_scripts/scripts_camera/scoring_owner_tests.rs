//! CPP GameLogic396/458/4797 and ScriptActions2819/6792: match-owned scoring.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::ScriptEngine;

#[cfg(test)]
impl GameLogic {
    pub(crate) fn execute_scoring_script_for_test(&mut self, enabled: bool) {
        let engine = ScriptEngine::new().expect("local script engine");
        let action = ScriptAction::new(if enabled {
            ScriptActionType::EnableScoring
        } else {
            ScriptActionType::DisableScoring
        });
        engine.friend_execute_action_with_driver(
            &action,
            None,
            gamelogic::scripting::executor::ScriptContext::at_frame(self.frame),
            &mut HostScriptExecutionDriver::new(self),
        );
        assert_eq!(self.is_scoring_enabled(), enabled);
    }
}

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let mut killer = Player::new(0, Team::USA, "USA", true);
    killer.alliance_team = 1;
    killer.cash_bounty_percent = 0.2;
    world.add_player(killer);
    let mut victim = Player::new(2, Team::GLA, "GLA", false);
    victim.alliance_team = 2;
    world.add_player(victim);
    let mut template = ThingTemplate::new("OwnedScoringTank");
    template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Score)
        .set_health(10.0);
    template.build_cost.supplies = 500;
    template.experience_values = [25.0; 4];
    template.skill_point_values = [25; 4];
    world.templates.insert("OwnedScoringTank".into(), template);
    let killer = world
        .create_object("OwnedScoringTank", Team::USA, Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object("OwnedScoringTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    (world, killer, victim)
}

fn kill(world: &mut GameLogic, killer: ObjectId, victim: ObjectId) {
    world.host_object_mut(victim).unwrap().last_damage_source = Some(killer);
    world.mark_object_for_destruction(victim, Some(Team::USA));
    world.process_destroy_list();
    assert!(world.host_object(victim).is_none());
}

#[test]
fn scoring_owner_script_toggle_same_ids_and_reset_preserve_other_world_statistics() {
    let (mut first, killer, victim) = world();
    first.execute_scoring_script_for_test(false);
    let (mut second, other_killer, other_victim) = world();
    assert_eq!((killer, victim), (other_killer, other_victim));
    assert!(
        second.is_scoring_enabled(),
        "constructing another world starts TRUE"
    );
    assert!(
        !first.is_scoring_enabled(),
        "constructor must not publish a scoring flag"
    );
    second.execute_scoring_script_for_test(false);
    second.execute_scoring_script_for_test(true);
    kill(&mut second, other_killer, other_victim);
    assert_eq!(second.get_player(0).unwrap().statistics.units_destroyed, 1);
    assert_eq!(second.get_player(2).unwrap().statistics.units_lost, 1);
    second.reset();
    assert!(second.is_scoring_enabled());
    assert!(!first.is_scoring_enabled());
    kill(&mut first, killer, victim);
    assert_eq!(first.get_player(0).unwrap().statistics.units_destroyed, 0);
    assert_eq!(first.get_player(2).unwrap().statistics.units_lost, 0);
    assert!(first.get_player(0).unwrap().statistics.money_earned > 0);
    assert!(first.get_player(0).unwrap().skill_points > 0);
}

#[test]
fn scoring_owner_encoded_save_and_failed_candidate_preserve_actual_kill_gates() {
    use crate::save_load::snapshot::{SnapshotBuilder, decode_bincode_world_snapshot};
    let (mut live, killer, victim) = world();
    live.execute_scoring_script_for_test(false);
    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&live).unwrap();
    assert!(!saved.is_scoring_enabled);
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let saved = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = GameLogic::new();
    restored.templates = live.templates.clone();
    builder
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    assert!(!restored.is_scoring_enabled());
    let mut candidate = GameLogic::new();
    candidate.execute_scoring_script_for_test(false);
    let mut malformed = decode_bincode_world_snapshot(&bytes).unwrap();
    malformed.version = u32::MAX;
    assert!(
        builder
            .restore_from_snapshot(&malformed, &mut candidate)
            .is_err()
    );
    candidate.reset();
    drop(candidate);
    assert!(!live.is_scoring_enabled());
    assert!(!restored.is_scoring_enabled());
    kill(&mut live, killer, victim);
    kill(&mut restored, killer, victim);
    assert_eq!(live.get_player(0).unwrap().statistics.units_destroyed, 0);
    assert_eq!(
        restored.get_player(0).unwrap().statistics.units_destroyed,
        0
    );
    assert_eq!(live.get_player(2).unwrap().statistics.units_lost, 0);
    assert_eq!(restored.get_player(2).unwrap().statistics.units_lost, 0);
    assert_eq!(
        live.get_player(0).unwrap().statistics.money_earned,
        restored.get_player(0).unwrap().statistics.money_earned
    );
    restored.execute_scoring_script_for_test(true);
    assert!(!live.is_scoring_enabled());
    let next = restored
        .create_object("OwnedScoringTank", Team::GLA, Vec3::splat(20.0))
        .unwrap();
    kill(&mut restored, killer, next);
    assert_eq!(
        restored.get_player(0).unwrap().statistics.units_destroyed,
        1
    );
    assert_eq!(restored.get_player(2).unwrap().statistics.units_lost, 1);
    assert_eq!(live.get_player(0).unwrap().statistics.units_destroyed, 0);
}
