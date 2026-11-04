//! hq-mqi4c: JetAI attack/return locomotor types come from authored module data
//! (C++ JetAIUpdate.cpp:1636 defaults NORMAL; Aurora authors SET_SUPERSONIC).

use crate::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use crate::game_logic::object::Object;
use glam::Vec3;

fn admit_jet(world: &mut GameLogic, name: &str, jet_ai_body: &str) {
    let ini = format!(
        "Object {name}\n\
KindOf = AIRCRAFT CAN_ATTACK SELECTABLE\n\
Body = ActiveBody Body\n\
MaxHealth = 100\n\
InitialHealth = 100\n\
End\n\
Behavior = JetAIUpdate JetAI\n\
{jet_ai_body}\
End\n\
Locomotor = SET_NORMAL RaptorJetLocomotor\n\
Locomotor = SET_SUPERSONIC AuroraJetSupersonicLocomotor\n\
Locomotor = SET_TAXIING BasicJetTaxiLocomotor\n\
End\n"
    );
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                format!("{name}-jet-ai.ini"),
                ini,
            )])
            .unwrap(),
        )
        .unwrap();
    assert!(world.ensure_host_spawn_template(name));
}

#[test]
fn raptor_default_attack_locomotor_is_normal_not_invented_supersonic() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "USA", true));
    // Retail Raptor omits AttackLocomotorType → C++ LOCOMOTORSET_NORMAL.
    admit_jet(
        &mut world,
        "RaptorAttackLocoOwner",
        "    ReturnToBaseIdleTime = 10000\n",
    );
    let id = world
        .create_object_for_player("RaptorAttackLocoOwner", 0, Vec3::new(10.0, 0.0, 10.0))
        .expect("spawn raptor");
    let jet = world.host_object(id).expect("jet");
    assert_eq!(
        jet.jet_attack_locomotor_token_for_test(),
        "SET_NORMAL",
        "missing AttackLocomotorType must keep C++ NORMAL default"
    );
    assert_eq!(jet.jet_attack_loco_persist_frames(), 0);
    assert_eq!(jet.jet_sneaky_offset_when_attacking(), 0.0);
    assert_eq!(jet.jet_attackers_miss_persist_frames(), 0);
    assert_eq!(jet.jet_return_to_base_idle_frames(), 300);
}

#[test]
fn aurora_authored_attack_locomotor_is_supersonic_from_module() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "USA", true));
    admit_jet(
        &mut world,
        "AuroraAttackLocoOwner",
        "    SneakyOffsetWhenAttacking = -20.0\n\
    AttackLocomotorType = SET_SUPERSONIC\n\
    AttackLocomotorPersistTime = 100\n\
    AttackersMissPersistTime = 2000\n\
    ReturnForAmmoLocomotorType = SET_SLUGGISH\n\
    ReturnToBaseIdleTime = 10000\n",
    );
    let id = world
        .create_object_for_player("AuroraAttackLocoOwner", 0, Vec3::new(20.0, 0.0, 20.0))
        .expect("spawn aurora");
    let jet = world.host_object(id).expect("jet");
    assert_eq!(jet.jet_attack_locomotor_token_for_test(), "SET_SUPERSONIC");
    assert_eq!(jet.jet_attack_loco_persist_frames(), 3); // 100ms → 3 frames
    assert!((jet.jet_sneaky_offset_when_attacking() - (-20.0)).abs() < 0.01);
    assert_eq!(jet.jet_attackers_miss_persist_frames(), 60); // 2000ms
    assert_eq!(jet.jet_return_to_base_idle_frames(), 300);
}

#[test]
fn assetless_jet_does_not_invent_supersonic_or_aurora_persist() {
    let mut t = ThingTemplate::new("AmericaJetRaptor");
    t.add_kind_of(KindOf::Aircraft);
    let jet = Object::new(t, ObjectId(9), Team::USA);
    assert_eq!(
        jet.jet_attack_locomotor_token_for_test(),
        "SET_NORMAL",
        "no JetAI module must use C++ default NORMAL, not invented SUPERSONIC"
    );
    assert_eq!(jet.jet_attack_loco_persist_frames(), 0);
    assert_eq!(jet.jet_sneaky_offset_when_attacking(), 0.0);
    assert_eq!(jet.jet_attackers_miss_persist_frames(), 0);
    assert_eq!(jet.jet_return_to_base_idle_frames(), 0);
}
