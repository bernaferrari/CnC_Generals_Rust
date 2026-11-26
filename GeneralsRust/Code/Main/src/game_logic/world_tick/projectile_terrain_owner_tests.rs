//! Real Main projectile impact ownership, using C++'s bridge crossing branch.
// The detonation is observed through the client FXList the impact executes.
#![cfg(feature = "game_client")]
use crate::game_logic::weapon_bootstrap::{HostDumbProjectileFlight, HostProjectileFlight};
use crate::game_logic::{GameLogic, ObjectId, Weapon};
use glam::Vec3;
use std::sync::mpsc::{Receiver, Sender, channel};

/// ProjectileDetonationFX witness. Weapon::handleProjectileDetonation runs the
/// FXList at the detonation position (doFXPos) before Body damage, so the
/// end-phase impact queue is already drained when the update returns.
struct DetonationWitness(Sender<glam::Vec3>);

impl game_client::fx_list::FXNugget for DetonationWitness {
    fn do_fx_pos(
        &self,
        primary: Option<&glam::Vec3>,
        _: Option<&glam::Mat4>,
        _: f32,
        _: Option<&glam::Vec3>,
        _: f32,
    ) {
        self.0.send(*primary.expect("detonation position")).unwrap();
    }
    fn do_fx_obj_host(
        &self,
        _: &gamelogic::helpers::HostFxObjectPose,
        _: Option<&gamelogic::helpers::HostFxObjectPose>,
    ) {
        panic!("bridge crossing does not invent a direct victim");
    }
}

/// The local player sees the crossing, so its client FX gate admits it.
fn witness(world: &mut GameLogic) -> Receiver<glam::Vec3> {
    world.add_player(crate::game_logic::Player::new(
        1,
        crate::game_logic::Team::USA,
        "Viewer",
        true,
    ));
    assert!(world.select_local_player(Some(1)));
    {
        let mut shroud = world.world_services.shroud().lock().unwrap();
        shroud.init_shroud_grid_at_origin([-100.0, -100.0], 200.0, 200.0);
        shroud.do_shroud_reveal(&glam::Vec3::new(20.0, 20.0, 0.0), 40.0, 1 << 1);
    }
    let (send, receive) = channel();
    let mut fx = game_client::fx_list::FXList::new();
    fx.add_fx_nugget(Box::new(DetonationWitness(send)));
    let mut catalog = game_client::fx_list::FXListStore::new();
    catalog.add_fx_list("ProjectileOwnerImpact".into(), fx);
    world.admit_fx_catalog(catalog);
    receive
}

fn world(deck: f32) -> (GameLogic, ObjectId, Receiver<glam::Vec3>) {
    let mut world = GameLogic::new();
    world.override_world_size(200.0, 200.0);
    let detonations = witness(&mut world);
    let width = world.pathfinding_system.grid.width() as u32;
    let height = world.pathfinding_system.grid.height() as u32;
    assert!(world.restore_terrain_heights_from_grid(
        width,
        height,
        &vec![0.0; (width * height) as usize]
    ));
    ({
        let ground_owner = world.world_services.terrain();
        let ground_loan = ground_owner
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
            &ground_loan,
            world.map_loaded,
            world.terrain.as_ref(),
            world.pathfinding_height_samples.as_ref(),
            &world.pathfinding_system.grid,
        );
        world.pathfinding_system.grid.stamp_bridge_deck(
            Vec3::new(10.0, deck, 10.0),
            Vec3::new(10.0, deck, 30.0),
            Vec3::new(30.0, deck, 10.0),
            Vec3::new(30.0, deck, 30.0),
            false,
            &ground_source,
        )
    });
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
    (world, id, detonations)
}

fn impact(
    world: &mut GameLogic,
    id: ObjectId,
    deck: f32,
    detonations: &Receiver<glam::Vec3>,
) {
    // Match the real fixed-step owner scope retained for other Core services.
    // The projectile query itself must never consult its duplicate Core AI.
    let services = std::sync::Arc::clone(&world.world_services);
    let retired = gamelogic::system::engine_stores::with_world_services(&services, || {
        world.update_owned_projectile_impacts(1.0 / 30.0).unwrap()
    });
    assert_eq!(retired, vec![id]);
    assert_eq!(world.combat_system.projectile_count(), 0);
    assert!(
        world.combat_system.take_impact_fx().is_empty(),
        "detonation FX flush synchronously before Body damage"
    );
    let effects = detonations.try_iter().collect::<Vec<_>>();
    assert_eq!(effects.len(), 1, "exactly one detonation at the crossing");
    // Client FX coordinates are C++ Z-up: host (x, y, z) is (x, z, y).
    assert_eq!(
        (effects[0].x, effects[0].y, effects[0].z),
        (20.0, 20.0, deck + 2.0),
        "original two-unit bridge art fudge precedes detonation"
    );
}

#[test]
fn real_projectile_bridge_impact_uses_same_id_world_deck_with_foreign_core_ai_held() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "real_projectile_bridge_impact_uses_same_id_world_deck_with_foreign_core_ai_held",
        || {
            {
                let (mut first, id, first_fx) = world(35.0);
                let (mut second, second_id, second_fx) = world(55.0);
                let (mut reference, reference_id, reference_fx) = world(35.0);
                assert_eq!(id, second_id);
                assert_eq!(id, reference_id);
                impact(&mut reference, id, 35.0, &reference_fx);
                let foreign = gamelogic::system::engine_stores::new_for_world();
                let held = foreign.ai().write().unwrap();
                gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                    impact(&mut second, id, 55.0, &second_fx);
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
                    impact(&mut first, id, 35.0, &first_fx);
                });
                drop(held);
            };
        },
    );
}
