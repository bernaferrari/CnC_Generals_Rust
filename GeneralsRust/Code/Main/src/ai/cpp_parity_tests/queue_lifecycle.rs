use super::*;

#[test]
fn check_queued_teams_zero_idle_frames_never_expires() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut proto = gamelogic::team::TeamPrototype::new("HQ_80_Never".into());
    proto.set_initial_idle_frames(0);
    logic
        .team_factory
        .lock()
        .expect("team factory")
        .replace_team_prototype(proto);
    let mut ai = ai_for_world(&logic, 1, Team::USA, AIDifficulty::Medium);
    let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
    order.num_completed = 0;
    ai.team_queue.push_back(AITeamQueue::new(
        "HQ_80_Never".into(),
        vec![order],
        false,
        0,
    ));
    ai.check_queued_teams(&mut logic, 999.0);
    assert_eq!(
        ai.team_queue.len(),
        1,
        "InitialIdleFrames < 1 is unlimited; team must not expire"
    );
    assert!(ai.team_ready_queue.is_empty());
}

#[test]
fn check_queued_teams_disbands_expired_incomplete_team() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "USA AI",
        false,
    ));
    let mut ranger_t = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger_t.add_kind_of(crate::game_logic::KindOf::Infantry);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger_t);
    let ranger = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("ranger");
    if let Some(obj) = logic.host_object_mut(ranger) {
        obj.owner_player_id = Some(1);
        obj.team_instance_name = "HQ_9_Disband".into();
    }

    let mut inst_id = None;
    {
        let mut tf = logic.team_factory.lock().expect("world team factory");
        let mut proto = gamelogic::team::TeamPrototype::new("HQ_9_Disband".into());
        proto.set_initial_idle_frames(30);
        tf.replace_team_prototype(proto);
        if let Some(team) = tf.create_inactive_team("HQ_9_Disband") {
            if let Ok(mut tg) = team.write() {
                tg.add_member(ranger.0);
                inst_id = Some(tg.get_id());
            }
        }
    }

    let mut ai = ai_for_world(&logic, 1, Team::USA, AIDifficulty::Medium);
    let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
    order.num_completed = 0;
    order.observed_unit_ids.push(ranger);
    let mut q = AITeamQueue::new("HQ_9_Disband".into(), vec![order], false, 0);
    q.team_id = inst_id;
    ai.team_queue.push_back(q);

    ai.check_queued_teams(&mut logic, 2.0);
    assert!(
        ai.team_queue.is_empty() && ai.team_ready_queue.is_empty(),
        "expired team below minimum must disband"
    );
    let default = logic.default_host_team_instance_name(Some(1), Team::USA);
    assert_eq!(
        logic
            .host_object(ranger)
            .map(|o| o.team_instance_name.clone())
            .unwrap_or_default(),
        default,
        "disband must transfer recruits to the default team"
    );
    assert!(
        ai.leftover_team_instance_gone(inst_id),
        "non-singleton leftover instance must be deleted on disband"
    );
}
