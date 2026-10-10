//! Real Main projectile impact ownership, using C++'s bridge crossing branch.
use crate::game_logic::{GameLogic, ObjectId, Weapon};
use crate::game_logic::weapon_bootstrap::{HostDumbProjectileFlight, HostProjectileFlight};
use glam::Vec3;

fn world(deck: f32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.override_world_size(200.0, 200.0);
    let width = world.pathfinding_system.grid.width() as u32;
    let height = world.pathfinding_system.grid.height() as u32;
    assert!(world.restore_terrain_heights_from_grid(
        width,
        height,
        &vec![0.0; (width * height) as usize]
    ));
    world.pathfinding_system.grid.stamp_bridge_deck(
        Vec3::new(10.0, deck, 10.0),
        Vec3::new(10.0, deck, 30.0),
        Vec3::new(30.0, deck, 10.0),
        Vec3::new(30.0, deck, 30.0),
        false,
    );
    let start = Vec3::new(20.0, deck + 1.0, 20.0);
    let layer = world
        .pathfinding_system
        .grid
        .highest_layer_at_or_below(start, 0.0, false);
    assert_eq!(layer, 2, "fixture admits a real bridge flight layer");
    let id = world.combat_system.fire_projectile(
        start,
        Vec3::new(100.0, 0.0, 100.0),
        &Weapon::default(),
        ObjectId(7),
        None,
        30.0,
    );
    let projectile = world.combat_system.projectile_mut(id).unwrap();
    projectile.flight = Some(HostProjectileFlight::Dumb(
        HostDumbProjectileFlight::default(),
    ));
    // Persisted flight samples enter the same real DumbProjectile update branch
    // as Bezier launch admission. The next sample is just below the deck.
    projectile.flight_runtime.path = vec![
        Vec3::new(20.0, deck - 0.25, 20.0),
        Vec3::new(100.0, 0.0, 100.0),
    ];
    projectile.flight_runtime.layer = layer;
    projectile.detonation_fx_name = "ProjectileOwnerImpact".into();
    (world, id)
}

fn impact(world: &mut GameLogic, id: ObjectId, deck: f32) {
    // Match the real fixed-step owner scope retained for other Core services.
    // The projectile query itself must never consult its duplicate Core AI.
    let services = std::sync::Arc::clone(&world.world_services);
    let retired = gamelogic::system::engine_stores::with_world_services(&services, || {
        world.update_owned_projectile_impacts(1.0 / 30.0)
    });
    assert_eq!(retired, vec![id]);
    assert_eq!(world.combat_system.projectile_count(), 0);
    let effects = world.combat_system.take_impact_fx();
    assert_eq!(effects.len(), 1, "exactly one detonation at the crossing");
    assert_eq!(
        effects[0].position,
        Vec3::new(20.0, deck + 2.0, 20.0),
        "original two-unit bridge art fudge precedes detonation"
    );
    assert_eq!(effects[0].detonation_fx_name, "ProjectileOwnerImpact");
    assert_eq!(
        effects[0].target_id, None,
        "bridge crossing does not invent a direct victim"
    );
}

#[test]
fn real_projectile_bridge_impact_uses_same_id_world_deck_with_foreign_core_ai_held() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "real_projectile_bridge_impact_uses_same_id_world_deck_with_foreign_core_ai_held",
        || {
            crate::gameworld_shadow::with_gameworld_authority(
                crate::game_logic::game_logic::GameWorldAuthority::DEFAULT_OFF,
                || {
                    let (mut first, id) = world(35.0);
                    let (mut second, second_id) = world(55.0);
                    let (mut reference, reference_id) = world(35.0);
                    assert_eq!(id, second_id);
                    assert_eq!(id, reference_id);
                    impact(&mut reference, id, 35.0);
                    let foreign = gamelogic::system::engine_stores::new_for_world();
                    let held = foreign.ai().write().unwrap();
                    gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                        impact(&mut second, id, 55.0);
                        assert_eq!(
                            first.combat_system.projectile_count(),
                            1,
                            "other world's impact cannot retire this same-ID flight"
                        );
                        second.reset();
                        let unrelated = GameLogic::new();
                        drop(unrelated);
                        assert!(std::sync::Arc::ptr_eq(
                            &gamelogic::system::engine_stores::active(),
                            &foreign
                        ));
                        impact(&mut first, id, 35.0);
                    });
                    drop(held);
                },
            );
        },
    );
}
