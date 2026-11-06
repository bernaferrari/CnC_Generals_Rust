//! hq-2qedt.40 / hq-fndvwq: a creation callback that replaces the producer
//! (same ObjectId, new birth) during SpawnUnit must leave the newborn alive but
//! unlinked: no producer_id, no inherited factory orientation.
use super::*;
use crate::game_logic::{GameLogic, Player, ThingTemplate};

fn world() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "Owner", true));
    for name in ["ReplaceSelfFactory", "ReplaceSelfUnit"] {
        logic
            .templates
            .insert(name.to_string(), ThingTemplate::new(name));
    }
    let producer = logic
        .create_object_for_player("ReplaceSelfFactory", 0, Vec3::ZERO)
        .expect("create producer");
    {
        let (p, partition) = logic.host_object_and_partition_mut(producer).unwrap();
        p.set_orientation_with_partition_owner(1.25, partition);
    }
    (logic, producer)
}

fn complete(logic: &mut GameLogic, producer: ObjectId) -> ObjectId {
    logic
        .host_apply_unit_production_completions_with_effects(
            vec![(
                Team::USA,
                "ReplaceSelfUnit".to_string(),
                Vec3::new(25.0, 0.0, 40.0),
                None,
                producer,
            )],
            &mut Default::default(),
        )
        .expect("production completion");
    logic
        .host_objects()
        .registered_values()
        .find(|o| o.template_name == "ReplaceSelfUnit")
        .map(|o| o.id)
        .expect("newborn survives")
}

#[test]
fn replace_self_creation_callback_invalidates_producer_birth_receipt() {
    // Control: untouched producer links the newborn and passes orientation.
    let (mut control, producer) = world();
    spawn_callback_hook::clear();
    let unit = complete(&mut control, producer);
    let u = control.host_object(unit).unwrap();
    assert_eq!(u.producer_id, Some(producer));
    assert!((u.get_orientation() - 1.25).abs() < 1e-4);

    // Replace-self: callback swaps the producer for a new birth at same ID.
    let (mut replaced, producer) = world();
    spawn_callback_hook::install(|logic, producer, _newborn| {
        let mut replacement = logic.objects.remove(&producer).unwrap();
        replacement.visual_draw_identity_token = std::sync::Arc::new(());
        logic.objects.insert(producer, replacement);
    });
    let unit = complete(&mut replaced, producer);
    spawn_callback_hook::clear();
    let u = replaced.host_object(unit).unwrap();
    assert_eq!(
        u.producer_id, None,
        "newborn must not attach to a replacement factory at the same id"
    );
    assert!(
        (u.get_orientation() - 1.25).abs() > 1e-4,
        "retired factory orientation must not be applied"
    );
}
