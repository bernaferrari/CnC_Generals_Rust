//! Current onDie input is distinct from ActiveBody's preferred hit history.
//! CPP ActiveBody.cpp:545–566/649, CrushDie.cpp:142–148,
//! StructureToppleUpdate.cpp:119–148 and InactiveBody.cpp:53–86.
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;
use crate::game_logic::{GameLogic, ObjectId, Team};

fn authored(name: &str, body: &str, kind: &str, behaviors: &str) -> String {
    let health = if body == "InactiveBody" {
        ""
    } else {
        "MaxHealth = 100\nInitialHealth = 100"
    };
    format!(
        "Object {name}\nKindOf = {kind}\nGeometry = BOX\nGeometryMajorRadius = 20\nGeometryMinorRadius = 10\nGeometryHeight = 30\nBody = {body} BodyTag\n{health}\nEnd\n{behaviors}\nEnd"
    )
}

fn setup(body: &str, toppler: bool) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.frame = 20;
    let victim = authored(
        "IncomingVictim",
        body,
        if toppler { "STRUCTURE" } else { "INERT" },
        if toppler {
            "Behavior = StructureToppleUpdate ToppleTag\nStructuralIntegrity = 0.1\nStructuralDecay = 0\nEnd"
        } else {
            "Behavior = CrushDie CrushTag\nDeathTypes = ALL\nEnd"
        },
    );
    let source = [
        victim,
        authored("PreferredSource", "ActiveBody", "VEHICLE", ""),
        authored("IncomingSource", "ActiveBody", "INERT IMMOBILE", ""),
    ]
    .join("\n");
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "incoming-death-owner.ini".to_string(),
        source,
    )])
    .unwrap();
    world.admit_receiving_module_content(content).unwrap();
    let victim = world
        .create_object("IncomingVictim", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let preferred = world
        .create_object(
            "PreferredSource",
            Team::China,
            glam::Vec3::new(-20.0, 0.0, 0.0),
        )
        .unwrap();
    let incoming = world
        .create_object(
            "IncomingSource",
            Team::China,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .unwrap();
    (world, victim, preferred, incoming)
}

fn hit(
    world: &mut GameLogic,
    victim: ObjectId,
    source: Option<ObjectId>,
    amount: f32,
    kind: DamageType,
    death: HostDeathType,
) -> bool {
    let context = DamageHitContext::new(source.and_then(|id| world.host_object(id)), None, kind);
    world
        .apply_owned_damage(victim, amount, source, kind, death, None, &context)
        .unwrap()
        .destroyed
}

fn preferred_hit(world: &mut GameLogic, victim: ObjectId, source: ObjectId) {
    assert!(!hit(
        world,
        victim,
        Some(source),
        1.0,
        DamageType::Unresistable,
        HostDeathType::Normal
    ));
    let object = world.host_object(victim).unwrap();
    assert_eq!(object.health.current, 99.0);
    assert_eq!(object.last_damage_source, Some(source));
}

#[test]
fn lethal_crush_uses_current_source_without_replacing_preferred_body_history() {
    for next_frame in [false, true] {
        let (mut owner, victim, preferred, incoming) = setup("ActiveBody", false);
        let (mut peer, peer_victim, _, _) = setup("ActiveBody", false);
        assert_eq!(victim, peer_victim);
        preferred_hit(&mut owner, victim, preferred);
        if next_frame {
            owner.frame += 1;
        }
        assert!(hit(
            &mut owner,
            victim,
            Some(incoming),
            200.0,
            DamageType::Crush,
            HostDeathType::Crushed
        ));
        let object = owner.host_object(victim).unwrap();
        assert_eq!(object.last_damage_source, Some(preferred));
        assert!(object.status.on_die_started);
        assert!(object.front_crushed);
        assert!(!object.back_crushed);
        assert!(!peer.host_object(peer_victim).unwrap().status.on_die_started);
        peer.frame += 100;
        assert_eq!(
            owner.host_object(victim).unwrap().last_damage_source,
            Some(preferred)
        );
    }
}

#[test]
fn crush_callback_requires_damage_type_independently_of_death_type() {
    for (kind, death, crushed) in [
        (DamageType::Flame, HostDeathType::Crushed, false),
        (DamageType::Crush, HostDeathType::Normal, true),
    ] {
        let (mut owner, victim, _, incoming) = setup("ActiveBody", false);
        assert!(hit(&mut owner, victim, Some(incoming), 200.0, kind, death));
        let object = owner.host_object(victim).unwrap();
        assert_eq!(object.front_crushed, crushed);
        assert!(!object.back_crushed);
    }
}

#[test]
fn initial_topple_direction_uses_incoming_source_and_preserves_history() {
    let (mut owner, victim, preferred, incoming) = setup("ActiveBody", true);
    preferred_hit(&mut owner, victim, preferred);
    assert!(hit(
        &mut owner,
        victim,
        Some(incoming),
        200.0,
        DamageType::Unresistable,
        HostDeathType::Normal
    ));
    let object = owner.host_object(victim).unwrap();
    assert_eq!(object.last_damage_source, Some(preferred));
    let topple = object
        .structure_topple_data
        .as_ref()
        .expect("authored topple admitted");
    assert!(
        topple.dir_x < -0.9,
        "east source must topple west: {}",
        topple.dir_x
    );
    assert!(topple.dir_y.abs() < 0.4);
}

#[test]
fn inactive_body_delivers_incoming_source_without_manufacturing_hit_history() {
    let (mut owner, victim, _, incoming) = setup("InactiveBody", true);
    assert!(hit(
        &mut owner,
        victim,
        Some(incoming),
        0.0,
        DamageType::Unresistable,
        HostDeathType::Normal
    ));
    let object = owner.host_object(victim).unwrap();
    assert_eq!(object.last_damage_source, None);
    assert_eq!(object.last_damage_timestamp, None);
    let topple = object
        .structure_topple_data
        .as_ref()
        .expect("inactive onDie forwards input");
    assert!(topple.dir_x < -0.9);
    assert!(topple.dir_y.abs() < 0.4);
}

#[test]
fn invalid_current_source_does_not_reuse_preferred_crusher() {
    let (mut owner, victim, preferred, _) = setup("ActiveBody", false);
    preferred_hit(&mut owner, victim, preferred);
    assert!(hit(
        &mut owner,
        victim,
        None,
        200.0,
        DamageType::Crush,
        HostDeathType::Crushed
    ));
    let object = owner.host_object(victim).unwrap();
    assert_eq!(object.last_damage_source, Some(preferred));
    // CPP release behavior: absent incoming dealer produces TOTAL_CRUSH.
    assert!(object.front_crushed && object.back_crushed);
}

#[test]
fn immortal_body_rejects_both_ordinary_detonation_and_explicit_kill() {
    // This supported receiving profile supplies the actual ImmortalBody floor;
    // generic subclass definition admission is a separate tracked obligation.
    let name = "SupplyWarehouseImmortalWitness";
    let mut owner = GameLogic::new();
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "incoming-immortal-owner.ini".to_string(),
        authored(name, "ImmortalBody", "INERT", ""),
    )])
    .unwrap();
    owner.admit_receiving_module_content(content).unwrap();
    let victim = owner
        .create_object(name, Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert!(!hit(
        &mut owner,
        victim,
        None,
        100.0,
        DamageType::Unresistable,
        HostDeathType::Detonated
    ));
    assert_eq!(owner.host_object(victim).unwrap().health.current, 1.0);
    assert!(!owner.host_object(victim).unwrap().status.on_die_started);
    assert!(
        !owner
            .apply_owned_kill(victim, DamageType::Unresistable, HostDeathType::Detonated)
            .unwrap()
            .destroyed
    );
    assert_eq!(owner.host_object(victim).unwrap().health.current, 1.0);
    assert!(!owner.host_object(victim).unwrap().status.on_die_started);
}

#[test]
fn missing_incoming_source_id_does_not_reuse_captured_pose_or_preferred_history() {
    let (mut owner, victim, preferred, incoming) = setup("ActiveBody", false);
    preferred_hit(&mut owner, victim, preferred);
    let context = DamageHitContext::new(owner.host_object(incoming), None, DamageType::Crush);
    // Source-lifetime admission control: this supplies a context captured while
    // B existed, but B is absent when the real owned hit reaches onDie. It does
    // not pretend to execute an original callback that deleted the source.
    let removed = owner.objects.remove(&incoming).unwrap();
    assert_eq!(removed.id, incoming);
    assert!(owner.host_object(incoming).is_none());
    assert!(
        owner
            .apply_owned_damage(
                victim,
                200.0,
                Some(incoming),
                DamageType::Crush,
                HostDeathType::Crushed,
                None,
                &context,
            )
            .unwrap()
            .destroyed
    );
    let object = owner.host_object(victim).unwrap();
    assert_eq!(object.last_damage_source, Some(preferred));
    // CPP CrushDie release behavior resolves the current source ID and uses
    // TOTAL_CRUSH when absent. Neither the captured east pose nor A is valid.
    assert!(object.front_crushed && object.back_crushed);
}

#[test]
fn retained_mark_adapter_does_not_fabricate_current_input_from_body_history() {
    let (mut owner, victim, preferred, _) = setup("ActiveBody", false);
    assert!(!hit(
        &mut owner,
        victim,
        Some(preferred),
        1.0,
        DamageType::Flame,
        HostDeathType::Normal,
    ));
    {
        let object = owner.host_object_mut(victim).unwrap();
        assert_eq!(object.last_damage_info_type, Some(DamageType::Flame));
        assert_eq!(object.last_damage_source, Some(preferred));
        object.status.death_type = HostDeathType::Crushed;
    }
    // Compatibility-boundary control only: this old mark API has no accepted
    // Body operation. Preserve its old status/source semantics instead of
    // pretending preferred Flame history is a new incoming lethal input.
    // This is not C++ native onDie fidelity evidence; caller migration remains.
    owner.mark_object_for_destruction(victim, None);
    let object = owner.host_object(victim).unwrap();
    assert!(object.status.on_die_started);
    assert!(!object.front_crushed);
    assert!(object.back_crushed);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Flame));
    assert_eq!(object.last_damage_source, Some(preferred));
}
