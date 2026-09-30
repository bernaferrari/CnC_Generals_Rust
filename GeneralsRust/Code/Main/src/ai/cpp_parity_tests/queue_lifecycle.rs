use super::*;

#[test]
fn check_queued_teams_zero_idle_frames_never_expires() {
    let team_factory = gamelogic::team::TeamFactoryHandle::new();
    let mut proto = gamelogic::team::TeamPrototype::new("HQ_80_Never".into());
    proto.set_initial_idle_frames(0);
    team_factory
        .lock()
        .expect("team factory")
        .replace_team_prototype(proto);
    let mut ai = AIPlayer::new_with_team_factory(1, Team::USA, AIDifficulty::Medium, team_factory);
    let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
    order.num_completed = 0;
    ai.team_queue.push_back(AITeamQueue::new(
        "HQ_80_Never".into(),
        vec![order],
        false,
        0,
    ));
    let mut logic = crate::game_logic::GameLogic::new();
    ai.check_queued_teams(&mut logic, 999.0);
    assert_eq!(
        ai.team_queue.len(),
        1,
        "InitialIdleFrames < 1 is unlimited; team must not expire"
    );
    assert!(ai.team_ready_queue.is_empty());
}
