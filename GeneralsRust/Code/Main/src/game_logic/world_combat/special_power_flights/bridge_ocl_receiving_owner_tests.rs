//! Bridge OCL receiving-world tests.

use super::*;

fn owner_with_bridge(child: &str) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
            .admit_object_creation_list_sources([format!(
                "ObjectCreationList OCL_GenericBridgeDie\nCreateObject\nObjectNames = {child}\nCount = 1\nEnd\nEnd\n"
            )])
            .expect("bridge die OCL admitted to this world");
    let mut bridge_template = ThingTemplate::new("BridgeOclOwnerFixture");
    bridge_template
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .set_health(100.0);
    world
        .templates
        .insert("BridgeOclOwnerFixture".into(), bridge_template);
    world
        .templates
        .insert(child.into(), ThingTemplate::new(child));
    let bridge = world
        .create_object(
            "BridgeOclOwnerFixture",
            Team::USA,
            Vec3::new(30.0, 0.0, 40.0),
        )
        .expect("bridge owner object");
    (world, bridge)
}

#[test]
fn bridge_die_ocl_uses_the_receiving_world_for_same_id_bridges() {
    let (mut left, left_bridge) = owner_with_bridge("LeftBridgeDieWitness");
    let (mut right, right_bridge) = owner_with_bridge("RightBridgeDieWitness");
    assert_eq!(left_bridge, right_bridge);

    left.play_bridge_die_ocl_at_with_effects(
        left_bridge,
        &mut crate::game_logic::ScriptPresentationEffects::default(),
    );
    right.play_bridge_die_ocl_at_with_effects(
        right_bridge,
        &mut crate::game_logic::ScriptPresentationEffects::default(),
    );

    assert!(
        left.objects
            .registered_values()
            .any(|object| object.template_name == "LeftBridgeDieWitness")
    );
    assert!(
        !left
            .objects
            .registered_values()
            .any(|object| object.template_name == "RightBridgeDieWitness")
    );
    assert!(
        right
            .objects
            .registered_values()
            .any(|object| object.template_name == "RightBridgeDieWitness")
    );
    assert!(
        !right
            .objects
            .registered_values()
            .any(|object| object.template_name == "LeftBridgeDieWitness")
    );
    assert!(left.objects.get(&left_bridge).is_some());
    assert!(right.objects.get(&right_bridge).is_some());
}
