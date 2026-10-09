//! Actual owning Main world/accepted-fire/publication boundary tests.
//! No AI replacement runner, queue injection, fake registry, or lock witness.
use super::*;
use crate::gameworld_shadow::GameWorldShadow;
use crate::presentation_frame::{PresentationEvent, PresentationFrame};
use crate::save_load::snapshot::SnapshotBuilder;

const SOURCE: ObjectId = ObjectId(93_701);
const VICTIM: ObjectId = ObjectId(93_702);

fn admitted_world(start_barrel: u8) -> GameLogic {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "local", true));
    logic.add_player(Player::new(1, Team::China, "enemy", false));
    let mut source_template = ThingTemplate::new("OwnedDischargeSource");
    source_template.add_kind_of(KindOf::Attackable);
    let mut source = Object::new(source_template, SOURCE, Team::USA);
    source.owner_player_id = Some(0);
    source.set_position(Vec3::ZERO);
    source.weapon = Some(Weapon {
        damage: 1.0,
        range: 500.0,
        reload_time: 0.0,
        last_fire_time: -1.0,
        ammo: Some(8),
        clip_size: 8,
        projectile_speed: 0.0,
        ..Weapon::default()
    });
    assert!(source.set_weapon_barrel_count_for_slot(0, 3));
    source.weapon_barrel_states[0].current_barrel = start_barrel;
    source.weapon_barrel_states[0].shots_left_on_barrel = 1;
    let mut victim_template = ThingTemplate::new("OwnedDischargeVictim");
    victim_template.add_kind_of(KindOf::Attackable);
    let mut victim = Object::new(victim_template, VICTIM, Team::China);
    victim.owner_player_id = Some(1);
    victim.set_position(Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(logic.add_object(source), SOURCE);
    assert_eq!(logic.add_object(victim), VICTIM);
    logic.frame = 30;
    logic
}

fn accept_shot(logic: &mut GameLogic, current_time: f32) {
    let ammo = logic
        .host_object(SOURCE)
        .unwrap()
        .weapon
        .as_ref()
        .unwrap()
        .ammo;
    assert_eq!(
        logic.attack_fire_weapon_update(SOURCE, VICTIM, current_time),
        AttackFireResult::Success,
        "use the normal accepted WeaponSet fire operation"
    );
    assert_eq!(
        logic
            .host_object(SOURCE)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        ammo.map(|n| n - 1),
        "the physical accepted operation must consume one shot"
    );
}

fn cues(frame: &PresentationFrame) -> Vec<(ObjectId, u64, u8, u8, u32)> {
    frame
        .events
        .iter()
        .filter_map(|event| match event {
            PresentationEvent::WeaponDischarged {
                source,
                sequence,
                weapon_slot,
                fired_barrel,
                logic_frame,
                ..
            } => Some((
                *source,
                *sequence,
                *weapon_slot,
                *fired_barrel,
                *logic_frame,
            )),
            _ => None,
        })
        .collect()
}

fn publish(logic: &mut GameLogic) -> PresentationFrame {
    PresentationFrame::publish_for_engine(logic, 0, None)
}

fn publish_shadow_case(
    logic: &mut GameLogic,
    shadow: &GameWorldShadow,
    case: u8,
) -> PresentationFrame {
    match case {
        0 => PresentationFrame::publish_for_engine(logic, 0, Some(shadow)),
        1 => PresentationFrame::publish_for_engine_with_runtime_heightmap(
            logic,
            0,
            Some(shadow),
            None,
        ),
        2 => PresentationFrame::build_with_victory_for_engine(logic, 0, Some(shadow)),
        3 => PresentationFrame::build_with_victory_for_engine_with_runtime_heightmap(
            logic,
            0,
            Some(shadow),
            None,
        ),
        _ => unreachable!(),
    }
}

#[test]
fn accepted_discharge_publication_fifo_duplicates_and_same_id_worlds() {
    let mut first = admitted_world(2);
    let mut second = admitted_world(1);
    accept_shot(&mut first, 1.0);
    accept_shot(&mut second, 1.0);
    accept_shot(&mut first, 2.0);
    assert_eq!(first.weapon_discharge_next_sequence_for_snapshot(), 3);
    assert_eq!(second.weapon_discharge_next_sequence_for_snapshot(), 2);
    assert_eq!(first.weapon_discharge_log.len(), 2);
    assert_eq!(second.weapon_discharge_log.len(), 1);
    assert_eq!(cues(&publish(&mut second)), vec![(SOURCE, 1, 0, 1, 30)]);
    assert_eq!(first.weapon_discharge_log.len(), 2);
    assert_eq!(
        cues(&publish(&mut first)),
        vec![(SOURCE, 1, 0, 2, 30), (SOURCE, 2, 0, 0, 30)]
    );
    assert!(cues(&publish(&mut first)).is_empty());
    assert!(cues(&publish(&mut second)).is_empty());
    assert_eq!(
        first
            .host_object(SOURCE)
            .unwrap()
            .weapon_barrel_state_for_slot(0)
            .unwrap()
            .current_barrel,
        1
    );
}

#[test]
fn accepted_discharge_borrowed_queries_leave_pending_cues_for_publication() {
    let mut logic = admitted_world(2);
    accept_shot(&mut logic, 1.0);
    let marker = logic.host_object(SOURCE).unwrap().weapon_discharge_marker();
    let source_query_clone = logic.host_object(SOURCE).unwrap().clone();
    assert_eq!(source_query_clone.weapon_discharge_marker(), marker);
    let mut shadow = GameWorldShadow::new(2);
    shadow.sync_from_host(&logic);
    let frames = [
        PresentationFrame::build_from_logic(&logic, 0),
        PresentationFrame::build_for_engine(&logic, 0, Some(&shadow)),
        PresentationFrame::build_from_gameworld(&shadow, 0, Some(&logic)),
    ];
    for frame in frames {
        assert!(
            cues(&frame).is_empty(),
            "a frozen query must not publish accepted cues"
        );
    }
    assert_eq!(logic.weapon_discharge_log.len(), 1);
    assert_eq!(
        logic.host_object(SOURCE).unwrap().weapon_discharge_marker(),
        marker
    );
    assert_eq!(cues(&publish(&mut logic)), vec![(SOURCE, 1, 0, 2, 30)]);
    assert!(cues(&publish(&mut logic)).is_empty());
}

fn assert_synchronized_empty_roster_fallback(case: u8) {
    assert!(
        crate::presentation_frame::presentation_from_gameworld_enabled(),
        "this fixture requires the shipped default roster route"
    );
    let mut logic = admitted_world(2);
    accept_shot(&mut logic, 1.0);
    accept_shot(&mut logic, 2.0);
    for id in [SOURCE, VICTIM] {
        assert!(
            logic
                .host_object_mut(id)
                .unwrap()
                .take_damage_from_immediate(1_000_000.0, None)
        );
        let object = logic.host_object(id).unwrap();
        assert!(object.status.destroyed && object.health.current <= 0.0);
    }
    // This is the real accepted-fire + immediate-damage + synchronized
    // builder API boundary before deferred destruction is drained. It is
    // not a claim that the ordinary engine tick retains this roster window.
    let mut shadow = GameWorldShadow::new(2);
    shadow.sync_from_host(&logic);
    let mut sparse = PresentationFrame::build_from_gameworld(&shadow, 0, None);
    assert_eq!(sparse.rebuild_objects_from_gameworld(&shadow), 0);
    assert_eq!(
        logic.weapon_discharge_log.len(),
        2,
        "no-host probe must not touch this world"
    );
    let frame = publish_shadow_case(&mut logic, &shadow, case);
    assert_eq!(frame.objects.len(), 2, "host fallback roster, case {case}");
    assert_eq!(
        cues(&frame),
        vec![(SOURCE, 1, 0, 2, 30), (SOURCE, 2, 0, 0, 30)],
        "the host fallback must retain the one published batch, case {case}"
    );
    assert_eq!(logic.weapon_discharge_log.len(), 0);
    assert!(cues(&publish_shadow_case(&mut logic, &shadow, case)).is_empty());
}

#[test]
fn accepted_discharge_synchronized_empty_roster_fallback_keeps_first_batch() {
    assert_synchronized_empty_roster_fallback(0);
}

#[test]
fn accepted_discharge_synchronized_empty_roster_runtime_fallback_keeps_first_batch() {
    assert_synchronized_empty_roster_fallback(1);
}

#[test]
fn accepted_discharge_synchronized_empty_roster_victory_fallback_keeps_first_batch() {
    assert_synchronized_empty_roster_fallback(2);
}

#[test]
fn accepted_discharge_synchronized_empty_roster_victory_runtime_fallback_keeps_first_batch() {
    assert_synchronized_empty_roster_fallback(3);
}

#[test]
fn accepted_discharge_source_retirement_keeps_the_frozen_shot_once() {
    let mut logic = admitted_world(2);
    accept_shot(&mut logic, 1.0);
    logic.destroy_object(SOURCE);
    logic.process_destroy_list();
    assert!(logic.host_object(SOURCE).is_none());
    let frame = publish(&mut logic);
    assert!(frame.objects.iter().all(|object| object.id != SOURCE));
    assert_eq!(
        cues(&frame),
        vec![(SOURCE, 1, 0, 2, 30)],
        "an accepted cue must not be filtered by the later live source roster"
    );
    assert!(cues(&publish(&mut logic)).is_empty());
}

#[test]
fn accepted_discharge_restore_and_reset_clear_only_the_driving_queue() {
    let mut first = admitted_world(2);
    let mut second = admitted_world(1);
    accept_shot(&mut first, 1.0);
    accept_shot(&mut second, 1.0);
    let marker = first.host_object(SOURCE).unwrap().weapon_discharge_marker();
    first.restore_weapon_discharge_next_sequence(41);
    assert_eq!(first.weapon_discharge_next_sequence_for_snapshot(), 41);
    assert_eq!(
        first.host_object(SOURCE).unwrap().weapon_discharge_marker(),
        marker
    );
    assert!(
        cues(&publish(&mut first)).is_empty(),
        "transient pre-load shot must not replay"
    );
    assert_eq!(second.weapon_discharge_log.len(), 1);
    accept_shot(&mut first, 2.0);
    assert_eq!(cues(&publish(&mut first)), vec![(SOURCE, 41, 0, 0, 30)]);
    accept_shot(&mut first, 3.0);
    first.reset();
    assert_eq!(first.weapon_discharge_next_sequence_for_snapshot(), 1);
    assert!(first.host_objects().is_empty());
    assert!(cues(&publish(&mut first)).is_empty());
    assert_eq!(cues(&publish(&mut second)), vec![(SOURCE, 1, 0, 1, 30)]);
}

#[test]
fn accepted_discharge_snapshot_builder_restores_baseline_without_replaying_cues() {
    let mut source = admitted_world(2);
    // Restore must receive the exact fixture definitions through the same
    // catalog setup existing SnapshotBuilder tests use, not synthesized names.
    for id in [SOURCE, VICTIM] {
        let definition = source.host_object(id).unwrap().thing().template.clone();
        source.templates.insert(definition.name.clone(), definition);
    }
    accept_shot(&mut source, 1.0);
    let saved_marker = source
        .host_object(SOURCE)
        .unwrap()
        .weapon_discharge_marker();
    assert_eq!(saved_marker.sequence, 1);
    assert_eq!(saved_marker.fired_barrel, 2);
    assert_eq!(source.weapon_discharge_log.len(), 1);

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("actual snapshot capture");
    assert_eq!(snapshot.next_weapon_discharge_sequence, 2);
    assert_eq!(snapshot.objects[&SOURCE].last_weapon_discharge_sequence, 1);
    assert_eq!(snapshot.objects[&SOURCE].last_weapon_discharge_barrel, 2);
    assert_eq!(
        source.weapon_discharge_log.len(),
        1,
        "capture is not publication"
    );

    let mut restored = admitted_world(1);
    accept_shot(&mut restored, 1.0);
    assert_eq!(
        restored.weapon_discharge_log.len(),
        1,
        "real pre-load pending shot"
    );
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("actual restore");
    assert_eq!(restored.weapon_discharge_next_sequence_for_snapshot(), 2);
    assert_eq!(
        restored
            .host_object(SOURCE)
            .unwrap()
            .weapon_discharge_marker(),
        saved_marker
    );
    assert_eq!(
        restored
            .host_object(SOURCE)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(7)
    );
    assert!(
        cues(&publish(&mut restored)).is_empty(),
        "neither saved nor displaced pre-load cue replays"
    );
    assert_eq!(
        source.weapon_discharge_log.len(),
        1,
        "restoring another owner cannot drain the source"
    );
    assert_eq!(cues(&publish(&mut source)), vec![(SOURCE, 1, 0, 2, 30)]);

    // Draw topology is a runtime configuration: validate it with the actual
    // existing API, then C++'s next pre-fire guard normalizes the saved raw3.
    assert!(
        restored
            .host_object_mut(SOURCE)
            .unwrap()
            .set_weapon_barrel_count_for_slot(0, 3)
    );
    accept_shot(&mut restored, 2.0);
    assert_eq!(restored.weapon_discharge_next_sequence_for_snapshot(), 3);
    assert_eq!(
        restored
            .host_object(SOURCE)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(6)
    );
    assert_eq!(cues(&publish(&mut restored)), vec![(SOURCE, 2, 0, 0, 30)]);
    assert!(cues(&publish(&mut restored)).is_empty());
}

#[test]
fn accepted_discharge_victory_fallback_freezes_post_defeat_cash() {
    assert!(crate::presentation_frame::presentation_from_gameworld_enabled());
    for case in [2, 3] {
        let mut logic = admitted_world(2);
        logic.game_mode = GameMode::Skirmish;
        logic.get_player_mut(0).unwrap().resources.supplies = 1_234;
        accept_shot(&mut logic, 1.0);
        for id in [SOURCE, VICTIM] {
            assert!(
                logic
                    .host_object_mut(id)
                    .unwrap()
                    .take_damage_from_immediate(1_000_000.0, None)
            );
            assert!(logic.host_object(id).unwrap().status.destroyed);
        }
        // Use the documented sync-before-builder API boundary. All mapped
        // objects are dead, so the empty-roster fallback is exercised.
        let mut shadow = GameWorldShadow::new(2);
        shadow.sync_from_host(&logic);
        let mut sparse = PresentationFrame::build_from_gameworld(&shadow, 0, None);
        assert_eq!(sparse.rebuild_objects_from_gameworld(&shadow), 0);
        assert_eq!(logic.get_player(0).unwrap().resources.supplies, 1_234);
        // Match production order: the logic phase updates before presentation.
        assert!(logic.evaluate_victory_condition().is_some());
        let frame = publish_shadow_case(&mut logic, &shadow, case);
        assert!(
            frame.match_over,
            "actual Skirmish victory evaluator, case {case}"
        );
        assert_eq!(
            logic.get_player(0).unwrap().resources.supplies,
            0,
            "actual killPlayer cash mutation must run, case {case}"
        );
        assert_eq!(
            frame.local_supplies, 0,
            "freeze the completed post-defeat cash, case {case}"
        );
        // Keep the original accepted-cue acceptance as well. Original OLD
        // fallback loses this cue; a repair must satisfy both assertions.
        assert_eq!(
            cues(&frame),
            vec![(SOURCE, 1, 0, 2, 30)],
            "retain the one accepted batch, case {case}"
        );
        assert_eq!(logic.weapon_discharge_log.len(), 0);
    }
}
