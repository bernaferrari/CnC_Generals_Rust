//! Actual receiving projectile callback and authored ViewShake boundary.
use super::*;
use crate::game_logic::weapon_bootstrap::{
    HostDumbProjectileFlight, HostMissileFlight, HostProjectileFlight,
};
use gamelogic::common::types::ObjectShroudStatus;

fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::Neutral, "Neutral", false));
    world.add_player(Player::new(1, Team::USA, "Local", true));
    world
}
fn missile(id: ObjectId, delay: u32) -> Projectile {
    let mut p = Projectile::new(
        id,
        Vec3::new(10., 4., 12.),
        Vec3::new(10000., 4., 12.),
        1.,
        DamageType::Unresistable,
        ObjectId(800),
        None,
    );
    p.speed = 30.;
    p.velocity = Vec3::new(30., 0., 0.);
    p.source_owner_player_id = Some(1);
    p.flight = Some(HostProjectileFlight::Missile(HostMissileFlight {
        ignition_delay_frames: delay,
        ignition_fx: "FX_ReceivingProjectile".into(),
        ..Default::default()
    }));
    // Production binds the authored flight after selecting its module. This
    // enters C++ LAUNCH; Projectile::new's unbound runtime is already armed.
    p.bind_authored_flight(p.position, p.target_position, p.speed, None);
    p
}
#[cfg(feature = "game_client")]
fn install_shake() {
    use game_engine::common::ini::ini_fx_list::{CameraShakeType, FXList, FXNugget};
    let mut definition = FXList::new("FX_ReceivingProjectile".into());
    definition.add_nugget(FXNugget::ViewShake {
        shake_type: CameraShakeType::Normal,
    });
    game_client::fx_list::get_fx_list_store_mut().add_fx_list(
        definition.name.to_string(),
        game_client::fx_list::FXList::from_definition(&definition),
    );
}
fn reveal_object(world: &GameLogic, id: ObjectId) {
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .set_host_object_shroud_status(1, id.0, ObjectShroudStatus::Clear);
}

#[test]
#[cfg(feature = "game_client")]
fn native_ignition_is_delayed_once_and_uses_receiving_view_after_restore() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_ignition_is_delayed_once_and_uses_receiving_view_after_restore",
        || {
            install_shake();
            let rules = game_engine::common::global_data::read().clone();
            assert!(rules.max_shake_range > 0. && rules.shake_normal_intensity > 0.);
            let id = ObjectId(9001);
            let mut owner = world();
            let mut peer = world();
            owner.session_view.target = Vec3::new(10., 0., 12.);
            peer.session_view.target = Vec3::new(10. + rules.max_shake_range * 2., 0., 12.);
            reveal_object(&owner, id);
            reveal_object(&peer, id);
            owner.combat_system.projectiles.insert(id, missile(id, 2));
            peer.combat_system.projectiles.insert(id, missile(id, 2));
            game_client::display::view::with_tactical_view(|view| {
                view.set_position(&game_client::display::view::Point3::new(
                    -100000., -100000., 0.,
                ));
                view.reset_camera_shake();
            });
            owner.update_owned_projectile_impacts(1. / 30.);
            assert_eq!(owner.session_view.impulse.intensity, 0.);
            assert!(
                !owner.combat_system.projectiles[&id]
                    .flight_runtime
                    .missile_armed
            );
            let encoded = serde_json::to_vec(&owner.combat_system.projectiles[&id]).unwrap();
            let mut restored = world();
            restored
                .session_view
                .admit(owner.session_view.capture())
                .unwrap();
            reveal_object(&restored, id);
            restored
                .combat_system
                .projectiles
                .insert(id, serde_json::from_slice(&encoded).unwrap());
            peer.update_owned_projectile_impacts(2. / 30.);
            assert_eq!(
                peer.session_view.impulse.intensity, 0.,
                "same ID peer uses its own far target"
            );
            owner.update_owned_projectile_impacts(1. / 30.);
            restored.update_owned_projectile_impacts(1. / 30.);
            let expected = if rules.shake_normal_intensity > rules.max_shake_intensity {
                3.
            } else {
                rules.shake_normal_intensity
            };
            assert_eq!(
                owner.session_view.impulse.intensity, expected,
                "OLD ambient view dispatch leaves this zero"
            );
            assert_eq!(restored.session_view.impulse.intensity, expected);
            assert_eq!(owner.session_view.impulse.offset, Vec3::ZERO);
            assert!(
                owner.combat_system.projectiles[&id]
                    .flight_runtime
                    .missile_armed
            );
            assert!(
                restored.combat_system.projectiles[&id]
                    .flight_runtime
                    .missile_armed
            );
            // Pre-ignition projectile restore above proves delay/arming/intensity,
            // not equality of independently drawn presentation RNG angles.
            // At a new POST-ignition checkpoint, restore the exact owner impulse
            // and projectile; future offsets must continue that saved value.
            let saved_projectile =
                serde_json::to_vec(&owner.combat_system.projectiles[&id]).unwrap();
            restored
                .combat_system
                .projectiles
                .insert(id, serde_json::from_slice(&saved_projectile).unwrap());
            restored
                .session_view
                .admit(owner.session_view.capture())
                .unwrap();
            for _ in 0..4 {
                let before = owner.session_view.impulse.intensity;
                owner.update_owned_projectile_impacts(1. / 30.);
                restored.update_owned_projectile_impacts(1. / 30.);
                assert_eq!(
                    owner.session_view.impulse.intensity, before,
                    "armed missile must not replay ignition"
                );
                assert_eq!(
                    owner.session_view.advance_impulse(),
                    restored.session_view.advance_impulse()
                );
                // Draw decay makes an accidental second ignition observable.
                let decayed = owner.session_view.impulse.intensity;
                owner.update_owned_projectile_impacts(0.);
                restored.update_owned_projectile_impacts(0.);
                assert_eq!(owner.session_view.impulse.intensity, decayed);
            }
            game_client::display::view::with_tactical_view(|view| {
                assert_eq!(view.camera_shake_intensity(), 0.)
            });
        },
    );
}

#[test]
fn ignition_callback_observes_unarmed_event_pose_before_exhaust_and_attack() {
    let mut p = missile(ObjectId(9001), 2);
    let mut calls = Vec::new();
    let mut fx = |name: &str, object: ProjectileFxObject<'_>| {
        let ProjectileFxObject::Projectile(p) = object else {
            panic!("ignition needs missile object");
        };
        calls.push((name.to_owned(), p.position));
        assert!(!p.flight_runtime.missile_armed);
        assert_eq!(p.leftover_exhaust_id, 0);
    };
    p.update_with_terrain_and_fx(1. / 30., true, None, &mut fx);
    p.update_with_terrain_and_fx(1. / 30., true, None, &mut fx);
    p.update_with_terrain_and_fx(1. / 30., true, None, &mut fx);
    assert_eq!(
        calls,
        vec![("FX_ReceivingProjectile".into(), Vec3::new(10., 4., 12.))]
    );
    assert!(p.flight_runtime.missile_armed);
}

#[test]
#[cfg(feature = "game_client")]
fn native_garrison_hit_uses_building_pose_and_suppresses_normal_detonation() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_garrison_hit_uses_building_pose_and_suppresses_normal_detonation",
        || {
            install_shake();
            for eligible in [false, true] {
                let mut owner = world();
                let building = ObjectId(2);
                let occupant = ObjectId(3);
                let projectile = ObjectId(9001);
                let pos = Vec3::new(1000., 0., 0.);
                let mut rider = Object::new_simple(
                    occupant,
                    crate::game_logic::ObjectType::Infantry,
                    "Rider".into(),
                );
                rider.set_position(pos);
                owner.objects.insert(occupant, rider);
                let mut bunker = Object::new_simple(
                    building,
                    crate::game_logic::ObjectType::Building,
                    "Bunker".into(),
                );
                bunker.set_position(pos);
                bunker.template_mut().contain_module.kind =
                    crate::game_logic::ContainModuleKind::Garrison;
                bunker.template_mut().garrison_contain_max = Some(5);
                let mut data = crate::game_logic::buildings::BuildingData::new(
                    crate::game_logic::buildings::BuildingType::Bunker,
                );
                data.garrisoned_units = vec![occupant];
                bunker.building_data = Some(data);
                owner.objects.insert(building, bunker);
                reveal_object(&owner, building);
                owner.session_view.target = pos;
                let mut p = Projectile::new(
                    projectile,
                    Vec3::ZERO,
                    pos,
                    1.,
                    DamageType::Unresistable,
                    ObjectId(800),
                    Some(building),
                );
                p.speed = 0.;
                p.flight = Some(HostProjectileFlight::Dumb(HostDumbProjectileFlight {
                    garrison_hit_kill_count: if eligible { 1 } else { 0 },
                    garrison_hit_kill_fx: "FX_ReceivingProjectile".into(),
                    ..Default::default()
                }));
                p.detonation_fx_name = "FX_NormalDetonation".into();
                owner.combat_system.projectiles.insert(projectile, p);
                let retired = owner.update_owned_projectile_impacts(1. / 30.);
                assert_eq!(retired, vec![projectile]);
                if eligible {
                    assert!(
                        owner.session_view.impulse.intensity > 0.,
                        "OLD dispatch mutates ambient view instead"
                    );
                    assert!(!owner.objects[&occupant].is_alive());
                    assert!(owner.objects[&building].contained_units().is_empty());
                    assert!(owner.combat_system.take_impact_fx().is_empty());
                } else {
                    assert_eq!(owner.session_view.impulse.intensity, 0.);
                    assert!(owner.objects[&occupant].is_alive());
                    assert!(!owner.combat_system.take_impact_fx().is_empty());
                }
            }
        },
    );
}
