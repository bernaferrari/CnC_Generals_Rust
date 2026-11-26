//! Actual owning Main world/accepted-fire/publication boundary tests.
//! No AI replacement runner, queue injection, fake registry, or lock witness.
use super::*;
use crate::presentation_frame::{
    PresentationEvent, PresentationFrame, PresentationRuntimeHeightmap,
};
use crate::save_load::snapshot::SnapshotBuilder;
use std::sync::Arc;

const SOURCE: ObjectId = ObjectId(93_701);
const VICTIM: ObjectId = ObjectId(93_702);

fn admitted_world(start_barrel: u8) -> GameLogic {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "local", true));
    logic.add_player(Player::new(1, Team::China, "enemy", false));
    // The firing source runs AIAttackFireWeaponState, so it owns an authored
    // AIUpdateInterface like any retail shooter (AIStates.cpp); the victim is
    // a plain attackable body.
    logic
        .admit_object_ini_sources([(
            "owned-discharge.ini".to_string(),
            "Object OwnedDischargeSource\nKindOf = CAN_ATTACK\nBody = ActiveBody ModuleTag_Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface ModuleTag_AI\nEnd\nEnd\nObject OwnedDischargeVictim\nKindOf = CAN_ATTACK\nBody = ActiveBody ModuleTag_Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
                .to_string(),
        )])
        .expect("authored discharge source admits");
    logic.set_next_object_id_for_restore(SOURCE);
    assert_eq!(
        logic.create_object("OwnedDischargeSource", Team::USA, Vec3::ZERO),
        Some(SOURCE)
    );
    assert_eq!(
        logic.create_object("OwnedDischargeVictim", Team::China, Vec3::new(10.0, 0.0, 0.0)),
        Some(VICTIM)
    );
    let source = logic.host_object_mut(SOURCE).unwrap();
    crate::fixture_content::arm_slot_with_stats(
        source,
        0,
        Weapon {
            damage: 1.0,
            range: 500.0,
            reload_time: 0.0,
            last_fire_time: -1.0,
            ammo: Some(8),
            clip_size: 8,
            projectile_speed: 0.0,
            ..Weapon::default()
        },
    );
    assert!(source.set_weapon_barrel_count_for_slot(0, 3));
    source.weapon_barrel_states[0].current_barrel = start_barrel;
    source.weapon_barrel_states[0].shots_left_on_barrel = 1;
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
    PresentationFrame::try_publish_for_engine_with_runtime_heightmap(logic, 0, None, false).unwrap()
}

fn publish_with_terrain(
    logic: &mut GameLogic,
    with_victory: bool,
    with_terrain: bool,
) -> PresentationFrame {
    let terrain = with_terrain.then(|| {
        Arc::new(PresentationRuntimeHeightmap {
            width: 2,
            height: 2,
            heights: vec![0.0, 1.0, 2.0, 3.0],
            scale: 1.0,
            ..Default::default()
        })
    });
    let frame = PresentationFrame::try_publish_for_engine_with_runtime_heightmap(
        logic,
        0,
        terrain.clone(),
        with_victory,
    )
    .unwrap();
    #[cfg(feature = "game_client")]
    if let Some(terrain) = terrain {
        assert!(Arc::ptr_eq(
            frame.world_env.runtime_heightmap.as_ref().unwrap(),
            &terrain
        ));
    }
    frame
}

#[test]
fn accepted_discharge_publication_fifo_duplicates_and_same_id_worlds() {
    let publish = |logic: &mut GameLogic, with_victory| {
        PresentationFrame::try_publish_for_engine_with_runtime_heightmap(
            logic,
            0,
            None,
            with_victory,
        )
        .unwrap()
    };
    let mut first = admitted_world(2);
    let mut second = admitted_world(1);
    accept_shot(&mut first, 1.0);
    accept_shot(&mut second, 1.0);
    accept_shot(&mut first, 2.0);
    assert_eq!(first.weapon_discharge_next_sequence_for_snapshot(), 3);
    assert_eq!(second.weapon_discharge_next_sequence_for_snapshot(), 2);
    assert_eq!(first.weapon_discharge_log.len(), 2);
    assert_eq!(second.weapon_discharge_log.len(), 1);
    assert_eq!(cues(&publish(&mut second, false)), vec![(SOURCE, 1, 0, 1, 30)]);
    assert_eq!(first.weapon_discharge_log.len(), 2);
    assert_eq!(
        cues(&publish(&mut first, true)),
        vec![(SOURCE, 1, 0, 2, 30), (SOURCE, 2, 0, 0, 30)]
    );
    assert!(cues(&publish(&mut first, true)).is_empty());
    assert!(cues(&publish(&mut second, false)).is_empty());
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
    let frames = [
        PresentationFrame::build_from_logic(&mut logic, 0),
        PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap(),
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

fn assert_cues_with_destroyed_registered_sources(with_victory: bool, with_terrain: bool) {
    let mut logic = admitted_world(2);
    accept_shot(&mut logic, 1.0);
    accept_shot(&mut logic, 2.0);
    for id in [SOURCE, VICTIM] {
        let (object, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
        assert!(object.take_damage_from_immediate(1_000_000.0, None, 0, health_events));
        let object = logic.host_object(id).unwrap();
        assert!(object.status.destroyed && object.health.current <= 0.0);
    }
    // Capture the boundary before physically retiring the destroyed objects.
    assert_eq!(logic.weapon_discharge_log.len(), 2);
    let frame = publish_with_terrain(&mut logic, with_victory, with_terrain);
    assert_eq!(
        frame
            .objects
            .iter()
            .map(|row| (row.id, row.destroyed))
            .collect::<Vec<_>>(),
        [(SOURCE, true), (VICTIM, true)]
    );
    assert_eq!(
        cues(&frame),
        vec![(SOURCE, 1, 0, 2, 30), (SOURCE, 2, 0, 0, 30)],
        "each accepted cue is frozen in its original order"
    );
    assert_eq!(logic.weapon_discharge_log.len(), 0);
    assert!(
        cues(&publish_with_terrain(
            &mut logic,
            with_victory,
            with_terrain
        ))
        .is_empty()
    );
}

#[test]
fn accepted_discharge_with_destroyed_registered_sources_ordinary() {
    assert_cues_with_destroyed_registered_sources(false, false);
}

#[test]
fn accepted_discharge_with_destroyed_registered_sources_with_terrain() {
    assert_cues_with_destroyed_registered_sources(false, true);
}

#[test]
fn accepted_discharge_with_destroyed_registered_sources_at_victory() {
    assert_cues_with_destroyed_registered_sources(true, false);
}

#[test]
fn accepted_discharge_with_destroyed_registered_sources_at_victory_with_terrain() {
    assert_cues_with_destroyed_registered_sources(true, true);
}

#[test]
fn accepted_discharge_source_retirement_keeps_the_frozen_shot_once() {
    let mut logic = admitted_world(2);
    accept_shot(&mut logic, 1.0);
    logic.destroy_object(SOURCE).unwrap();
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
    assert!(first.host_objects().registered_is_empty());
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
fn accepted_discharge_victory_freezes_post_defeat_cash() {
    for with_terrain in [false, true] {
        let mut logic = admitted_world(2);
        logic.game_mode = GameMode::Skirmish;
        logic.get_player_mut(0).unwrap().resources.supplies = 1_234;
        accept_shot(&mut logic, 1.0);
        for id in [SOURCE, VICTIM] {
            let (object, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
            assert!(object.take_damage_from_immediate(1_000_000.0, None, 0, health_events));
            assert!(logic.host_object(id).unwrap().status.destroyed);
        }
        assert_eq!(logic.get_player(0).unwrap().resources.supplies, 1_234);
        // Match production order: the logic phase updates before presentation.
        assert!(logic.evaluate_victory_condition().unwrap().is_some());
        let frame = publish_with_terrain(&mut logic, true, with_terrain);
        assert!(frame.match_over, "actual Skirmish victory evaluator");
        assert_eq!(
            logic.get_player(0).unwrap().resources.supplies,
            0,
            "actual killPlayer cash mutation must run"
        );
        assert_eq!(
            frame.local_supplies, 0,
            "freeze the completed post-defeat cash"
        );
        assert_eq!(
            cues(&frame),
            vec![(SOURCE, 1, 0, 2, 30)],
            "retain the one accepted batch"
        );
        assert_eq!(logic.weapon_discharge_log.len(), 0);
    }
}
