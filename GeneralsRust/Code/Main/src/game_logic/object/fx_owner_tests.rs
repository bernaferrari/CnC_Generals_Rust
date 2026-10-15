//! Execute authored client nuggets at the real native onDie/UndeadBody boundary.
use super::*;
use crate::game_logic::{DamageHitContext, GameLogic, ObjectId, Player, Team, ThingTemplate};
use game_engine::common::ini::ini_fx_list::{CameraShakeType, FXList, FXNugget};
use gamelogic::common::{Coord3D, Matrix3D, ObjectShroudStatus};
use gamelogic::helpers::HostFxObjectPose;
use glam::Vec3;
use std::sync::mpsc::{Sender, channel};

struct Observe(u8, Sender<(u8, HostFxObjectPose, Option<HostFxObjectPose>)>);
impl game_client::fx_list::FXNugget for Observe {
    fn do_fx_pos(
        &self,
        _: Option<&Coord3D>,
        _: Option<&Matrix3D>,
        _: f32,
        _: Option<&Coord3D>,
        _: f32,
    ) {
        panic!("this authored effect must keep object form");
    }
    fn do_fx_obj_host(&self, primary: &HostFxObjectPose, secondary: Option<&HostFxObjectPose>) {
        self.1.send((self.0, *primary, secondary.copied())).unwrap();
    }
}
fn install(
    name: &str,
    shake: bool,
    marker: u8,
    events: Sender<(u8, HostFxObjectPose, Option<HostFxObjectPose>)>,
) {
    let mut definition = FXList::new(name.into());
    if shake {
        definition.add_nugget(FXNugget::ViewShake {
            shake_type: CameraShakeType::Normal,
        });
    }
    let mut list = game_client::fx_list::FXList::from_definition(&definition);
    list.add_fx_nugget(Box::new(Observe(marker, events)));
    game_client::fx_list::get_fx_list_store_mut().add_fx_list(name.into(), list);
}
fn world(id: ObjectId, target: Vec3) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::Neutral, "Neutral", false));
    world.add_player(Player::new(1, Team::USA, "Local", true));
    world.session_view.target = target;
    let mut template = ThingTemplate::new("NativeDieOwner");
    template.set_health(100.);
    let mut object = Object::new(template, id, Team::USA);
    object.owner_player_id = Some(1);
    object.set_position(Vec3::new(10., 4., 12.));
    world.objects.insert(id, object);
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .set_host_object_shroud_status(1, id.0, ObjectShroudStatus::Clear);
    world
}
fn expected_impulse() -> f32 {
    let rules = game_engine::common::global_data::read();
    if rules.shake_normal_intensity > rules.max_shake_intensity {
        3.
    } else {
        rules.shake_normal_intensity
    }
}

#[test]
fn native_die_first_and_additional_modules_are_inline_owned_once_and_keep_killer() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_die_first_and_additional_modules_are_inline_owned_once_and_keep_killer",
        || {
            let (sent, received) = channel();
            install("FX_NativeFirstDie", true, 1, sent.clone());
            install("FX_NativeAdditionalDie", false, 2, sent);
            let id = ObjectId(9301);
            let mut owner = world(id, Vec3::new(10., 0., 12.));
            let range = game_engine::common::global_data::read().max_shake_range;
            let mut peer = world(id, Vec3::new(10. + range * 2., 0., 12.));
            for world in [&mut owner, &mut peer] {
                let mut killer = Object::new(
                    ThingTemplate::new("NativeKiller"),
                    ObjectId(9302),
                    Team::China,
                );
                killer.set_position(Vec3::new(25., 6., 50.));
                world.objects.insert(killer.id, killer);
                let object = world.objects.get_mut(&id).unwrap();
                object.last_damage_source = Some(ObjectId(9302));
                let mut die = crate::game_logic::host_fx_list_die::HostFxListDieData::with_fx(
                    "FX_NativeFirstDie",
                );
                die.more.push(
                    crate::game_logic::host_fx_list_die::HostFxListDieData::with_fx(
                        "FX_NativeAdditionalDie",
                    ),
                );
                object.fx_list_die = Some(die);
            }
            peer.fire_owned_fx_list_die(id);
            assert_eq!(peer.session_view.impulse.intensity, 0.);
            let peer_events: Vec<_> = received.try_iter().collect();
            assert_eq!(peer_events.iter().map(|e| e.0).collect::<Vec<_>>(), [1, 2]);
            owner.fire_owned_fx_list_die(id);
            assert_eq!(
                owner.session_view.impulse.intensity,
                expected_impulse(),
                "first native module cannot wait for the AI drain"
            );
            assert_eq!(
                owner.session_view.impulse.offset,
                Vec3::ZERO,
                "enqueue does not step the view"
            );
            let events: Vec<_> = received.try_iter().collect();
            assert_eq!(events.iter().map(|e| e.0).collect::<Vec<_>>(), [1, 2]);
            for (_, primary, secondary) in events {
                assert_eq!(primary.id, id.0);
                assert_eq!(primary.position, Coord3D::new(10., 12., 4.));
                let secondary = secondary.unwrap();
                assert_eq!(secondary.id, 9302);
                assert_eq!(secondary.position, Coord3D::new(25., 50., 6.));
            }
            assert!(owner.objects[&id].pending_death_fx.is_none());
            owner.fire_owned_fx_list_die(id);
            assert!(
                received.try_recv().is_err(),
                "authored DieMux once gate owns replay prevention"
            );
            let encoded = serde_json::to_vec(&owner.objects[&id]).unwrap();
            let mut restored = world(id, Vec3::ZERO);
            restored
                .session_view
                .admit(owner.session_view.capture())
                .unwrap();
            restored
                .objects
                .insert(id, serde_json::from_slice(&encoded).unwrap());
            restored.fire_owned_fx_list_die(id);
            assert!(
                received.try_recv().is_err(),
                "restored fired modules cannot replay"
            );
            assert_eq!(
                owner.session_view.advance_impulse(),
                restored.session_view.advance_impulse()
            );
        },
    );
}

#[test]
fn native_second_life_start_uses_receiving_view_for_normal_and_indestructible_body() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_second_life_start_uses_receiving_view_for_normal_and_indestructible_body",
        || {
            let (sent, received) = channel();
            install(
                crate::game_logic::host_battle_bus::BATTLE_BUS_FX_START_UNDEATH,
                true,
                7,
                sent,
            );
            for indestructible in [false, true] {
                let id = ObjectId(9401);
                let mut owner = world(id, Vec3::new(10., 0., 12.));
                let range = game_engine::common::global_data::read().max_shake_range;
                let mut peer = world(id, Vec3::new(10. + range * 2., 0., 12.));
                for world in [&mut owner, &mut peer] {
                    let object = world.objects.get_mut(&id).unwrap();
                    object.install_battle_bus_transport();
                    object.indestructible = indestructible;
                }
                for world in [&mut peer, &mut owner] {
                    let damage = world.objects[&id].health.current + 100.;
                    assert!(
                        world
                            .apply_owned_damage(
                                id,
                                damage,
                                None,
                                crate::game_logic::combat::DamageType::Explosive,
                                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                                None,
                                &DamageHitContext::default()
                            )
                            .is_some()
                    );
                    assert!(
                        world.objects[&id]
                            .battle_bus_body
                            .as_ref()
                            .unwrap()
                            .is_in_first_death
                    );
                }
                assert_eq!(peer.session_view.impulse.intensity, 0.);
                assert_eq!(owner.session_view.impulse.intensity, expected_impulse());
                let events: Vec<_> = received.try_iter().collect();
                assert_eq!(events.len(), 2);
                assert!(
                    events
                        .iter()
                        .all(|event| event.0 == 7 && event.1.id == id.0)
                );
            }
        },
    );
}
