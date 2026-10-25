//! Executable command and lifecycle regressions.

use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn owned_dozer_phase_completes_structure_without_ready_writeback() {
    use crate::game_logic::{AIState, Player};
    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "OwnedBuilder", true);
    // CPP Energy consumption==0 returns production; zero production
    // invokes the low-energy slowdown rather than full-speed construction.
    player.power_produced = 1;
    logic.add_player(player);
    ensure_template(&mut logic, "SfBarracks");
    let mut template = ThingTemplate::new("SfDozer");
    template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert(template.name.clone(), template);
    let id = logic
        .create_object_for_player("SfBarracks", 0, Vec3::ZERO)
        .unwrap();
    let dozer = logic
        .create_object_for_player("SfDozer", 0, Vec3::ZERO)
        .unwrap();
    logic.dozer_new_task_build(dozer, id);
    let action = logic.host_object(dozer).unwrap().dozer_dock_action.unwrap();
    let object = logic.host_object_mut(id).unwrap();
    object.set_status_under_construction(true);
    object.construction_percent = 0.99;
    object.builder_id = Some(dozer);
    {
        let (builder, partition) = logic.host_object_and_partition_mut(dozer).unwrap();
        builder.set_position_with_partition_owner(action, 0, &mut Default::default(), partition);
    }
    let (builder, attack_events) = logic.host_object_and_attack_events_mut(dozer).unwrap();
    builder.set_target(
        Some(id),
        attack_events,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    builder.set_ai_state(
        AIState::Constructing,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    builder.movement.path.clear();
    builder.waiting_for_path = false;
    logic.update_construction(&[id, dozer], 0.2);
    assert_eq!(logic.host_object(id).unwrap().construction_percent, 1.0);
    assert!(!logic.host_object(id).unwrap().status.under_construction);
    assert_eq!(
        logic.host_object(dozer).unwrap().dozer_task_build_target,
        None
    );
}

fn ensure_template(logic: &mut GameLogic, name: &str) {
    let mut t = ThingTemplate::new(name);
    t.set_health(200.0);
    t.build_time = 10.0;
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert(name.into(), t);
}
