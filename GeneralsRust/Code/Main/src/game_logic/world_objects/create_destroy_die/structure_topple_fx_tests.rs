//! Actual Main onDie/update, authored INI, and real GameClient FX nuggets.
//! CPP StructureToppleUpdate.cpp194–296 / FXList.cpp794–805.
use super::*;
use gamelogic::common::{Coord3D, Matrix3D, ObjectShroudStatus};
use gamelogic::helpers::HostFxObjectPose;
use std::sync::mpsc::{self, Receiver, Sender};

#[path = "structure_topple_filter_tests.rs"]
mod structure_topple_filter_tests;

#[derive(Debug)]
struct Effect {
    name: &'static str,
    primary: Option<HostFxObjectPose>,
    position: Option<Coord3D>,
}
struct Nugget {
    name: &'static str,
    events: Sender<Effect>,
}
impl game_client::fx_list::FXNugget for Nugget {
    fn do_fx_pos(
        &self,
        position: Option<&Coord3D>,
        _: Option<&Matrix3D>,
        _: f32,
        _: Option<&Coord3D>,
        _: f32,
    ) {
        self.events
            .send(Effect {
                name: self.name,
                primary: None,
                position: position.copied(),
            })
            .unwrap();
    }
    fn do_fx_obj_host(&self, primary: &HostFxObjectPose, _: Option<&HostFxObjectPose>) {
        // No catalog lock may span a callback. This is the real client nugget.
        drop(game_client::fx_list::get_fx_list_store_mut());
        self.events
            .send(Effect {
                name: self.name,
                primary: Some(*primary),
                position: None,
            })
            .unwrap();
    }
}
fn install_fx() -> Receiver<Effect> {
    let (events, received) = mpsc::channel();
    for name in [
        "FX_OwnToppleStart",
        "FX_OwnToppleDelay",
        "FX_OwnToppleAngle",
        "FX_OwnToppleLater",
        "FX_OwnToppleDone",
        "FX_OwnToppleCrush",
    ] {
        let mut fx = game_client::fx_list::FXList::new();
        fx.add_fx_nugget(Box::new(Nugget {
            name,
            events: events.clone(),
        }));
        game_client::fx_list::get_fx_list_store_mut().add_fx_list(name.into(), fx);
    }
    game_client::fx_list::register_fx_list_manager_bridge();
    received
}
fn world(local: u32, x: f32) -> (GameLogic, ObjectId) {
    world_with_crushing(local, x, None)
}
fn world_with_crushing(local: u32, x: f32, weapon: Option<&str>) -> (GameLogic, ObjectId) {
    world_with_filters(local, x, weapon, "")
}
fn world_with_filters(
    local: u32,
    x: f32,
    weapon: Option<&str>,
    filters: &str,
) -> (GameLogic, ObjectId) {
    let crushing = weapon.map_or(String::new(), |weapon| {
        format!("    CrushingWeaponName = {weapon}\n    CrushingFX = FX_OwnToppleCrush\n")
    });
    let source = format!(
        r#"
Object AuthoredOwnTopple
  KindOf = STRUCTURE SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = StructureToppleUpdate ModuleTag_Fall
    StructuralIntegrity = 0.1
    StructuralDecay = 0
    ToppleStartFX = FX_OwnToppleStart
    ToppleDelayFX = FX_OwnToppleDelay
    ToppleDoneFX = FX_OwnToppleDone
    AngleFX = 0.058 FX_OwnToppleAngle
    AngleFX = 0.2 FX_OwnToppleLater
    MinToppleBurstDelay = 34
    MaxToppleBurstDelay = 34
{crushing}{filters}
  End
End
Object OwnedCrushStructure
  KindOf = STRUCTURE
  Body = ActiveBody CrushBody
    MaxHealth = 100
    InitialHealth = 100
  End
End
Object FilteredCrushVictim
  KindOf = STRUCTURE
  Body = ActiveBody FilteredBody
    MaxHealth = 100
    InitialHealth = 100
  End
End
"#
    );
    let content =
        crate::assets::ReceivingObjectContent::from_sources([("owned-topple.ini".into(), source)])
            .unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    let definition = world.templates["AuthoredOwnTopple"]
        .structure_topple_definition
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap();
    assert_eq!(definition.integrity, 0.1);
    assert_eq!(definition.decay, 0.0);
    assert_eq!(
        definition.fx.angle_fx.len(),
        2,
        "repeated original AngleFX keeps source order"
    );
    assert_eq!(
        definition.fx.min_burst_delay, 2,
        "duration is original ceil(ms * 30/1000)"
    );
    if let Some(weapon) = weapon {
        assert_eq!(definition.fx.weapon, weapon);
        assert_eq!(definition.fx.fx, "FX_OwnToppleCrush");
    }
    world.add_player(Player::new(local, Team::USA, "Local", true));
    world.add_player(Player::new(8, Team::China, "Enemy", false));
    world.terrain = Some(crate::game_logic::TerrainData::flat(
        glam::Vec3::ZERO,
        glam::Vec3::new(256.0, 0.0, 256.0),
    ));
    {
        let mut shroud = world.world_services.shroud().lock().unwrap();
        shroud.init_shroud_grid(256.0, 256.0);
        shroud.reveal_map_for_player_permanently(local).unwrap();
    }
    let id = world
        .create_object_for_player("AuthoredOwnTopple", 8, glam::Vec3::new(x, 4.0, 12.0))
        .unwrap();
    // ActiveBody.cpp:110–111/154–157: MaxHealth alone leaves an authored
    // Body at zero HP. These phase tests require an admitted live toppler.
    let object = world.host_object(id).unwrap();
    assert_eq!(object.body_initial_health(), 100.0);
    assert_eq!(object.health.maximum, 100.0);
    assert_eq!(object.health.current, 100.0);
    assert!(object.is_alive());
    (world, id)
}
fn shroud(world: &GameLogic, player: u32, id: ObjectId, status: ObjectShroudStatus) {
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .set_host_object_shroud_status(player, id.0, status);
}
fn kill(world: &mut GameLogic, id: ObjectId) {
    assert!(
        world
            .apply_owned_kill(
                id,
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal
            )
            .unwrap()
            .destroyed
    );
    let object = world.host_object(id).unwrap();
    assert_eq!(object.health.current, 0.0);
    assert!(object.status.on_die_started);
    assert!(object.structure_topple_data.is_some());
}
fn foreign_core(
    id: ObjectId,
) -> (
    gamelogic::object::object_factory::ObjectFactory,
    gamelogic::GameLogic,
) {
    let mut factory = gamelogic::object::object_factory::ObjectFactory::new();
    let mut world = gamelogic::GameLogic::new();
    let template = gamelogic::common::DefaultThingTemplate::new("ForeignTopplePose".into());
    let actual = factory
        .prepare_object(
            &mut world,
            std::sync::Arc::new(template),
            Coord3D::new(999.0, 888.0, 777.0),
            None,
            gamelogic::object::object_factory::ObjectCreationFlags::NO_AI,
        )
        .unwrap();
    assert_eq!(actual, id.0);
    let object = factory
        .get_object(actual)
        .unwrap()
        .get_base_object()
        .unwrap();
    world.register_object(object).unwrap();
    (factory, world)
}

#[test]
fn structure_topple_real_on_die_and_first_update_use_owned_visibility_pose_and_cpp_nudge() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "structure_topple_real_on_die_and_first_update_use_owned_visibility_pose_and_cpp_nudge",
        || {
            let events = install_fx();
            let (mut left, id) = world(0, 10.0);
            let (mut right, other) = world(7, 100.0);
            assert_eq!(id, other);
            let (_factory, _foreign) = foreign_core(id);
            shroud(&left, 0, id, ObjectShroudStatus::Clear);
            shroud(&right, 7, other, ObjectShroudStatus::Shrouded);
            kill(&mut left, id);
            kill(&mut right, other);
            let start: Vec<_> = events.try_iter().collect();
            assert_eq!(
                start.len(),
                2,
                "Start is original doFXPos, independent of object shroud"
            );
            assert!(
                start
                    .iter()
                    .all(|effect| effect.name == "FX_OwnToppleStart" && effect.primary.is_none())
            );
            assert_eq!(start[0].position.unwrap(), Coord3D::new(10.0, 12.0, 4.0));
            assert_eq!(start[1].position.unwrap(), Coord3D::new(100.0, 12.0, 4.0));
            let old = left.host_object(id).unwrap().get_transform_matrix();
            assert!(!left.update_owned_structure_topple(id).unwrap());
            assert!(!right.update_owned_structure_topple(other).unwrap());
            let observed: Vec<_> = events.try_iter().collect();
            assert_eq!(
                observed.len(),
                1,
                "foreign owner visibility does not suppress or leak Angle FX"
            );
            assert_eq!(observed[0].name, "FX_OwnToppleAngle");
            let pose = observed[0].primary.unwrap();
            assert_eq!(pose.id, id.0);
            assert_eq!(pose.player_index, 8);
            assert_eq!(pose.position, Coord3D::new(10.0, 12.0, 4.0));
            assert_eq!(
                pose.transform,
                crate::game_logic::host_fx_list_dispatch::host_to_leftover_mat4(old)
            );
            let object = left.host_object(id).unwrap();
            let data = object.structure_topple_data.as_ref().unwrap();
            let velocity = 0.02_f32 * 0.001_f32.sin() * (1.0_f32 - 0.1_f32);
            assert_eq!(
                data.topple_velocity.to_bits(),
                velocity.to_bits(),
                "no invented velocity kick or floor"
            );
            assert_eq!(
                data.accumulated_angle.to_bits(),
                (0.001_f32 + velocity).to_bits()
            );
            // Literal CPP in-place row operations, not production rotation
            // factors reused as an expected value. Translation is untouched.
            let cpp = crate::game_logic::host_fx_list_dispatch::host_to_leftover_mat4(old);
            let mut rows = [
                [cpp.x_axis.x, cpp.y_axis.x, cpp.z_axis.x],
                [cpp.x_axis.y, cpp.y_axis.y, cpp.z_axis.y],
                [cpp.x_axis.z, cpp.y_axis.z, cpp.z_axis.z],
            ];
            let theta = -velocity * data.dir_y;
            let (c, s) = (theta.cos(), theta.sin());
            for column in 0..3 {
                let (tmp1, tmp2) = (rows[1][column], rows[2][column]);
                rows[1][column] = c * tmp1 - s * tmp2;
                rows[2][column] = s * tmp1 + c * tmp2;
            }
            let theta = velocity * data.dir_x;
            let (c, s) = (theta.cos(), theta.sin());
            for column in 0..3 {
                let (tmp1, tmp2) = (rows[0][column], rows[2][column]);
                rows[0][column] = c * tmp1 + s * tmp2;
                rows[2][column] = -s * tmp1 + c * tmp2;
            }
            let cpp_expected = glam::Mat4::from_cols(
                glam::Vec4::new(rows[0][0], rows[1][0], rows[2][0], 0.0),
                glam::Vec4::new(rows[0][1], rows[1][1], rows[2][1], 0.0),
                glam::Vec4::new(rows[0][2], rows[1][2], rows[2][2], 0.0),
                cpp.w_axis,
            );
            assert_eq!(
                object.get_transform_matrix(),
                crate::game_logic::host_fx_list_dispatch::host_to_leftover_mat4(cpp_expected)
            );
            assert_eq!(
                object.health.current, 0.0,
                "topple does not revive the dead body"
            );
        },
    );
}

#[test]
fn structure_topple_saved_future_preserves_actual_phase_fx_and_incremental_matrix() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "structure_topple_saved_future_preserves_actual_phase_fx_and_incremental_matrix",
        || {
            let events = install_fx();
            let (mut source, id) = world(0, 10.0);
            shroud(&source, 0, id, ObjectShroudStatus::Clear);
            kill(&mut source, id);
            for frame in 0..6 {
                source.frame = frame;
                source.update_owned_structure_topple(id).unwrap();
            }
            let _: Vec<_> = events.try_iter().collect();
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let (mut receiving, other) = world(0, 10.0);
            assert_eq!(id, other);
            builder
                .restore_from_snapshot(&snapshot, &mut receiving)
                .unwrap();
            shroud(&receiving, 0, id, ObjectShroudStatus::Clear);
            assert_eq!(
                source.host_object(id).unwrap().get_transform_matrix(),
                receiving.host_object(id).unwrap().get_transform_matrix()
            );
            let frozen =
                crate::presentation_frame::PresentationFrame::build_from_logic(&mut receiving, 0);
            assert_eq!(
                frozen.structure_topple_transforms.get(&id),
                Some(&receiving.host_object(id).unwrap().get_transform_matrix()),
                "renderer reads the completed owner matrix, not another lean reconstruction"
            );
            for frame in 6..24 {
                source.frame = frame;
                receiving.frame = frame;
                assert_eq!(
                    source.update_owned_structure_topple(id).unwrap(),
                    receiving.update_owned_structure_topple(id).unwrap()
                );
                let effects: Vec<_> = events.try_iter().collect();
                assert_eq!(effects.len() % 2, 0);
                let half = effects.len() / 2;
                for (continuous, restored) in effects[..half].iter().zip(&effects[half..]) {
                    assert_eq!(continuous.name, restored.name);
                    assert_eq!(continuous.position, restored.position);
                    assert_eq!(
                        continuous.primary.map(|pose| pose.transform),
                        restored.primary.map(|pose| pose.transform)
                    );
                }
                let source_object = source.host_object(id).unwrap();
                let restored_object = receiving.host_object(id).unwrap();
                assert_eq!(
                    source_object.get_transform_matrix(),
                    restored_object.get_transform_matrix()
                );
                let a = source_object.structure_topple_data.as_ref().unwrap();
                let b = restored_object.structure_topple_data.as_ref().unwrap();
                assert_eq!(a.accumulated_angle.to_bits(), b.accumulated_angle.to_bits());
                assert_eq!(a.next_burst_frame, b.next_burst_frame);
            }
        },
    );
}

#[test]
fn structure_topple_done_fx_observes_pre_rotation_pose_and_finishes_once_this_frame() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "structure_topple_done_fx_observes_pre_rotation_pose_and_finishes_once_this_frame",
        || {
            let events = install_fx();
            let (mut owner, id) = world(0, 10.0);
            shroud(&owner, 0, id, ObjectShroudStatus::Clear);
            kill(&mut owner, id);
            let _: Vec<_> = events.try_iter().collect();
            let mut done_frame = None;
            for frame in 0..600 {
                owner.frame = frame;
                let before = owner.host_object(id).unwrap().get_transform_matrix();
                let complete = owner.update_owned_structure_topple(id).unwrap();
                let observed: Vec<_> = events.try_iter().collect();
                let done: Vec<_> = observed
                    .iter()
                    .filter(|effect| effect.name == "FX_OwnToppleDone")
                    .collect();
                if !done.is_empty() {
                    assert_eq!(done.len(), 1);
                    assert!(complete, "CPP WaitingForDone completes in the same update");
                    assert_eq!(
                        done[0].primary.unwrap().transform,
                        crate::game_logic::host_fx_list_dispatch::host_to_leftover_mat4(before)
                    );
                    let object = owner.host_object(id).unwrap();
                    let data = object.structure_topple_data.as_ref().unwrap();
                    assert_eq!(data.done_frame, frame);
                    assert_eq!(
                        data.state,
                        crate::game_logic::host_structure_topple::HostStructureToppleState::Done
                    );
                    assert_eq!(object.health.current, 0.0);
                    let rubble =
                        1u128 << crate::game_logic::host_enum_table_residual::rubble_model_bit();
                    let post = 1u128
                        << crate::game_logic::host_enum_table_residual::post_collapse_model_bit();
                    assert_eq!(object.model_condition_bits & rubble, 0);
                    assert_ne!(object.model_condition_bits & post, 0);
                    done_frame = Some(frame);
                    break;
                }
                assert!(!complete);
            }
            assert!(done_frame.is_some());
            owner.frame = done_frame.unwrap() + 1;
            assert!(!owner.update_owned_structure_topple(id).unwrap());
            assert!(
                events.try_iter().next().is_none(),
                "sleep-forever never repeats Done FX"
            );
        },
    );
}

#[test]
fn retained_definition_does_not_change_raw_thing_positional_wire() {
    let (owner, id) = world(0, 10.0);
    let thing = owner.host_object(id).unwrap().thing().clone();
    assert!(thing.template.structure_topple_definition.is_some());
    let encoded = bincode_legacy::serialize(&thing).unwrap();
    let mut without_definition = thing.clone();
    without_definition.template.structure_topple_definition = None;
    assert_eq!(
        encoded,
        bincode_legacy::serialize(&without_definition).unwrap(),
        "immutable admission data never appends fields to positional Thing serde"
    );
    let decoded: crate::game_logic::Thing = bincode_legacy::deserialize(&encoded).unwrap();
    assert!(decoded.template.authored_structure_topple);
    assert!(decoded.template.structure_topple_definition.is_none());
    assert_eq!(decoded.get_transform_matrix(), thing.get_transform_matrix());
}

fn install_crushing_weapon(name: &str, damage: f32) {
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut weapon = gamelogic::weapon::WeaponTemplate::new(name.into());
        weapon.primary_damage = damage;
        weapon.primary_damage_radius = 20.0;
        weapon.damage_type = gamelogic::damage::DamageType::Explosion;
        weapon.death_type = gamelogic::damage::DeathType::Exploded;
        store.add_weapon_template(weapon);
    })
    .unwrap();
}
fn crushing_point(
    position: glam::Vec3,
) -> crate::game_logic::host_structure_topple::StructureToppleCrushSample {
    crate::game_logic::host_structure_topple::StructureToppleCrushSample {
        x: position.x,
        z: position.z,
        // These old component observations do not replace the actual weapon.
        damage: 99999.0,
        radius: 99999.0,
    }
}

#[test]
fn crushing_fires_each_authored_weapon_point_and_includes_structures() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "crushing_fires_each_authored_weapon_point_and_includes_structures",
        || {
            const WEAPON: &str = "OwnedRepeatedToppleWeapon";
            let effects = install_fx();
            install_crushing_weapon(WEAPON, 10.0);
            let (mut owner, source) = world_with_crushing(0, 10.0, Some(WEAPON));
            let (mut peer, peer_source) = world_with_crushing(7, 100.0, Some(WEAPON));
            assert_eq!(source, peer_source);
            let (_factory, _foreign) = foreign_core(source);
            kill(&mut owner, source);
            kill(&mut peer, peer_source);
            effects.try_iter().for_each(drop);
            let position = glam::Vec3::new(20.0, 0.0, 12.0);
            let victim = owner
                .create_object_for_player("OwnedCrushStructure", 0, position)
                .unwrap();
            let peer_victim = peer
                .create_object_for_player("OwnedCrushStructure", 7, position)
                .unwrap();
            assert_eq!(victim, peer_victim);
            for (world, id) in [(&owner, victim), (&peer, peer_victim)] {
                let object = world.host_object(id).unwrap();
                assert!(object.is_kind_of(KindOf::Structure));
                assert_eq!(object.body_initial_health(), 100.0);
                assert_eq!(object.health.current, 100.0);
                assert_eq!(
                    object.thing().template.geometry_info,
                    crate::game_logic::HostGeometryInfo::default()
                );
            }
            owner
                .apply_structure_topple_crush_samples(
                    source,
                    vec![crushing_point(position), crushing_point(position)],
                )
                .unwrap();
            assert_eq!(
                owner.host_object(victim).unwrap().health.current,
                80.0,
                "CPP creates and fires one weapon per point, not max damage per victim"
            );
            assert_eq!(
                peer.host_object(peer_victim).unwrap().health.current,
                100.0,
                "same-ID peer is outside the driving weapon owner"
            );
            assert_eq!(
                effects
                    .try_iter()
                    .filter(|effect| effect.name == "FX_OwnToppleCrush")
                    .count(),
                2
            );
        },
    );
}

#[test]
fn crushing_completes_actual_victim_on_die_before_point_fx_and_next_shot() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "crushing_completes_actual_victim_on_die_before_point_fx_and_next_shot",
        || {
            const WEAPON: &str = "OwnedLethalToppleWeapon";
            let effects = install_fx();
            install_crushing_weapon(WEAPON, 100.0);
            let (mut owner, source) = world_with_crushing(0, 10.0, Some(WEAPON));
            kill(&mut owner, source);
            effects.try_iter().for_each(drop);
            let position = glam::Vec3::new(20.0, 0.0, 12.0);
            let victim = owner
                .create_object_for_player("AuthoredOwnTopple", 0, position)
                .unwrap();
            owner
                .apply_structure_topple_crush_samples(
                    source,
                    vec![crushing_point(position), crushing_point(position)],
                )
                .unwrap();
            let observed: Vec<_> = effects.try_iter().map(|effect| effect.name).collect();
            assert_eq!(
                observed,
                [
                    "FX_OwnToppleStart",
                    "FX_OwnToppleCrush",
                    "FX_OwnToppleCrush"
                ],
                "real victim StructureTopple onDie/Start precedes the first point FX; next shot sees its dead Body"
            );
            let victim = owner.host_object(victim).unwrap();
            assert_eq!(victim.health.current, 0.0);
            assert!(victim.status.on_die_started);
            assert!(
                victim
                    .structure_topple_data
                    .as_ref()
                    .unwrap()
                    .start_fx_played
            );
        },
    );
}

#[test]
fn unsupported_crushing_template_rejects_before_owned_phase_cursor_or_fx() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "unsupported_crushing_template_rejects_before_owned_phase_cursor_or_fx",
        || {
            const WEAPON: &str = "OwnedUnsupportedToppleWeapon";
            let effects = install_fx();
            install_crushing_weapon(WEAPON, 100.0);
            gamelogic::weapon::with_weapon_store_mut(|store| {
                let mut weapon = store.find_weapon_template(WEAPON).unwrap().as_ref().clone();
                weapon.projectile_name = "ActualUnsupportedProjectile".into();
                store.add_weapon_template(weapon);
            })
            .unwrap();
            let (mut owner, source) = world_with_crushing(0, 10.0, Some(WEAPON));
            kill(&mut owner, source);
            effects.try_iter().for_each(drop);
            let before = bincode_legacy::serialize(
                owner
                    .host_object(source)
                    .unwrap()
                    .structure_topple_data
                    .as_ref()
                    .unwrap(),
            )
            .unwrap();
            let pose = owner.host_object(source).unwrap().get_transform_matrix();
            let error = owner.update_owned_structure_topple(source).unwrap_err();
            assert!(error.contains("projectile/stream birth"), "{error}");
            assert_eq!(
                bincode_legacy::serialize(
                    owner
                        .host_object(source)
                        .unwrap()
                        .structure_topple_data
                        .as_ref()
                        .unwrap()
                )
                .unwrap(),
                before
            );
            assert_eq!(
                owner.host_object(source).unwrap().get_transform_matrix(),
                pose
            );
            assert!(effects.try_iter().next().is_none());
        },
    );
}

#[test]
fn absent_crushing_source_never_uses_foreign_core_identity() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "absent_crushing_source_never_uses_foreign_core_identity",
        || {
            const WEAPON: &str = "OwnedAbsentToppleWeapon";
            let effects = install_fx();
            install_crushing_weapon(WEAPON, 100.0);
            let (mut owner, source) = world_with_crushing(0, 10.0, Some(WEAPON));
            let (_factory, _foreign) = foreign_core(source);
            owner.objects.remove(&source);
            let position = glam::Vec3::new(20.0, 0.0, 12.0);
            let victim = owner
                .create_object_for_player("AuthoredOwnTopple", 0, position)
                .unwrap();
            owner
                .apply_structure_topple_crush_samples(source, vec![crushing_point(position)])
                .unwrap();
            assert_eq!(owner.host_object(victim).unwrap().health.current, 100.0);
            assert!(effects.try_iter().next().is_none());
        },
    );
}

#[test]
fn crushing_uses_actual_ground_below_a_receiving_world_bridge_deck() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "crushing_uses_actual_ground_below_a_receiving_world_bridge_deck",
        || {
            const WEAPON: &str = "OwnedGroundToppleWeapon";
            let effects = install_fx();
            install_crushing_weapon(WEAPON, 10.0);
            let (mut owner, source) = world_with_crushing(0, 10.0, Some(WEAPON));
            let mut map = gamelogic::system::map_loader::MapData::new();
            map.width = 30;
            map.height = 30;
            map.heightmap = vec![0; 900];
            map.bridges
                .push(gamelogic::system::map_loader::BridgeData::new(
                    Coord3D::new(0.0, 12.0, 20.0),
                    Coord3D::new(40.0, 12.0, 20.0),
                    20.0,
                    "OwnedToppleBridge".into(),
                ));
            let terrain = owner.world_services.terrain().clone();
            owner
                .admit_map_bridge_geometry(&mut terrain.write().unwrap(), map)
                .unwrap();
            let position = glam::Vec3::new(20.0, 0.0, 12.0);
            assert_eq!(
                owner.terrain_height_at(position),
                Some(20.0),
                "real layer query observes the deck"
            );
            assert_eq!(owner.terrain_ground_height_at(position), Some(0.0));
            kill(&mut owner, source);
            effects.try_iter().for_each(drop);
            owner
                .apply_structure_topple_crush_samples(source, vec![crushing_point(position)])
                .unwrap();
            let observed: Vec<_> = effects
                .try_iter()
                .filter(|effect| effect.name == "FX_OwnToppleCrush")
                .collect();
            assert_eq!(observed.len(), 1);
            assert_eq!(
                observed[0].position.unwrap(),
                Coord3D::new(20.0, 12.0, 0.0),
                "CPP doDamageLine invokes getGroundHeight, not bridge-layer height"
            );
        },
    );
}
