//! CPP PhysicsUpdate: returned falling hit -> current effective death/SPLATTED;
//! clear FREEFALL -> resting Object::kill (max health + m_kill).
use crate::game_logic::{GameLogic, HostArmorSet, ObjectId, Player, Team};
use game_engine::common::ini::{ini_damage_fx as damage, ini_fx_list as fx};
use gamelogic::common::{Coord3D, Matrix3D, ObjectShroudStatus};
use gamelogic::helpers::HostFxObjectPose;
use glam::Vec3;
use std::sync::mpsc::{Receiver, Sender, channel};

const DIE: &str = "FX_LandingPhaseDie";
const ARMOR: &str = "FX_LandingPhaseArmor";
const RULES: &str = "DamageFX_LandingPhase";
const ZERO_ARMOR: &str = "Armor_LandingPhaseUnresistableZero";

struct Observe(&'static str, Sender<(&'static str, Option<u32>)>);
impl game_client::fx_list::FXNugget for Observe {
    fn do_fx_pos(
        &self,
        _: Option<&Coord3D>,
        _: Option<&Matrix3D>,
        _: f32,
        _: Option<&Coord3D>,
        _: f32,
    ) {
        panic!("landing Die/Armor must retain receiving object-form dispatch");
    }
    fn do_fx_obj_host(&self, _: &HostFxObjectPose, secondary: Option<&HostFxObjectPose>) {
        self.1
            .send((self.0, secondary.map(|pose| pose.id)))
            .unwrap();
    }
}
fn install() -> Receiver<(&'static str, Option<u32>)> {
    let (send, receive) = channel();
    for name in [DIE, ARMOR] {
        let mut definition = fx::FXList::new(name.into());
        definition.add_nugget(fx::FXNugget::ViewShake {
            shake_type: fx::CameraShakeType::Normal,
        });
        let mut list = game_client::fx_list::FXList::from_definition(&definition);
        list.add_fx_nugget(Box::new(Observe(name, send.clone())));
        game_client::fx_list::get_fx_list_store_mut().add_fx_list(name.into(), list);
    }
    let mut rules = damage::DamageFX::new();
    for kind in [
        damage::DamageType::Falling,
        damage::DamageType::Unresistable,
    ] {
        rules.set_major_minor_fx(kind, Some(ARMOR.into()), Some(ARMOR.into()), 1.);
    }
    damage::init_global_damage_fx_store();
    damage::get_damage_fx_store_mut()
        .unwrap()
        .add_damage_fx(RULES.into(), rules);
    receive
}
fn world() -> (GameLogic, ObjectId) {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&format!("Object LandingPhaseVictim\nKindOf = VEHICLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = PhysicsBehavior Physics\nKillWhenRestingOnGround = Yes\nEnd\nBehavior = FXListDie FX\nDeathFX = {DIE}\nEnd\nBehavior = KeepObjectDie Keep\nEnd\nEnd"), "landing-phase.ini").unwrap();
    let mut template = GameLogic::build_template_from_object_definition(
        "LandingPhaseVictim",
        parser.get_definition("LandingPhaseVictim").unwrap(),
        None,
    );
    let mut armor = gamelogic::object::armor::ArmorTemplate::new();
    armor.set_default(1.);
    armor.set_coefficient(gamelogic::damage::DamageType::Unresistable, 0.);
    let mut definitions = gamelogic::object::armor::ArmorDefinitions::new();
    definitions.register_template(ZERO_ARMOR, armor);
    template.armor_definitions = Some(std::sync::Arc::new(definitions));
    template.armor_sets.push(HostArmorSet {
        conditions: 0,
        armor: Some(ZERO_ARMOR.into()),
        damage_fx: Some(RULES.into()),
    });
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::Neutral, "Neutral", false));
    world.add_player(Player::new(1, Team::USA, "Local", true));
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("LandingPhaseVictim", 1, Vec3::ZERO)
        .unwrap();
    let mut shroud = world.world_services.shroud().lock().unwrap();
    shroud.init_shroud_grid(128., 128.);
    shroud.reveal_map_for_player_permanently(1).unwrap();
    shroud.set_host_object_shroud_status(1, id.0, ObjectShroudStatus::Clear);
    drop(shroud);
    (world, id)
}
fn resting(world: &mut GameLogic, id: ObjectId) {
    let object = world.objects.get_mut(&id).unwrap();
    object.health.current = 40.;
    assert!(
        object.kill_when_resting_on_ground,
        "receiving authored PhysicsBehavior, not snapshot-only fixture mutation"
    );
    object.is_in_freefall = true;
    object.set_status_disabled_freefall(true);
    object.movement.velocity = Vec3::ZERO;
    object.model_condition_bits |=
        1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL;
}
fn assert_dead_owner(world: &GameLogic, id: ObjectId) {
    let object = &world.objects[&id];
    assert_eq!(object.health.current, 0.);
    assert!(
        object.status.on_die_started,
        "actual installed onDie, not standalone peel"
    );
    for tag in ["FX", "Keep"] {
        let declaration = object
            .thing()
            .template
            .native_die_declarations
            .as_ref()
            .unwrap()
            .iter()
            .find(|declaration| declaration.module_tag.as_deref() == Some(tag))
            .expect("authored Die slot");
        assert!(
            object
                .native_die_runtime
                .completed
                .contains(&declaration.source_index),
            "authored {tag} slot must complete"
        );
    }
    assert_eq!(
        world.objects.len(),
        1,
        "KeepObjectDie leaves identity installed"
    );
    assert_eq!(world.objects_to_destroy.len(), 0);
    assert!(!object.status.disabled_freefall);
}

#[test]
fn bulk_stamp_kill_bit_runs_installed_die_before_armor_and_survives_encoded_continuation() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "bulk_stamp_kill_bit_runs_installed_die_before_armor_and_survives_encoded_continuation",
        || {
            let events = install();
            let (mut owner, id) = world();
            let (peer, peer_id) = world();
            assert_eq!(id, peer_id);
            resting(&mut owner, id);
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let saved = builder.create_world_snapshot(&owner).unwrap();
            let bytes = bincode_legacy::serialize(&saved).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            let (mut restored, restored_id) = world();
            assert_eq!(id, restored_id);
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            for candidate in [&mut owner, &mut restored] {
                candidate.stamp_airborne_targets_from_locomotor(&[id]);
                assert_dead_owner(candidate, id);
                assert!(candidate.session_view.impulse.intensity > 0.);
                assert_eq!(
                    events.try_iter().collect::<Vec<_>>(),
                    vec![(DIE, None), (ARMOR, None)]
                );
            }
            // Armor coefficient zero proves Object::kill's m_kill path; merely
            // sending current HP as ordinary UNRESISTABLE damage leaves 40 HP.
            assert_eq!(peer.objects[&id].health.current, 100.);
            assert!(!peer.objects[&id].status.on_die_started);
            assert_eq!(peer.session_view.impulse.intensity, 0.);
            let post = builder.create_world_snapshot(&owner).unwrap();
            let post_bytes = bincode_legacy::serialize(&post).unwrap();
            let post =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&post_bytes).unwrap();
            builder.restore_from_snapshot(&post, &mut restored).unwrap();
            restored.stamp_airborne_targets_from_locomotor(&[id]);
            assert!(
                events.try_recv().is_err(),
                "completed saved onDie cannot replay"
            );
            assert_dead_owner(&restored, id);
        },
    );
}

#[test]
fn actual_shock_stun_fall_completes_installed_die_before_self_source_armor() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_shock_stun_fall_completes_installed_die_before_self_source_armor",
        || {
            let events = install();
            let (mut owner, id) = world();
            let (peer, peer_id) = world();
            assert_eq!(id, peer_id);
            let object = owner.objects.get_mut(&id).unwrap();
            object.set_position(Vec3::new(0., 1., 0.));
            object.shock_stun_frames = 20;
            object.shock_allow_bounce = false;
            object.shock_was_airborne = true;
            object.was_airborne_last_frame = true;
            object.movement.velocity = Vec3::new(0., -10., 0.);
            object.physics_mass = 1.;
            object.min_fall_speed_for_damage = 0.;
            object.fall_height_damage_factor = 100.;
            owner.tick_shock_stun_all();
            assert_dead_owner(&owner, id);
            assert!(owner.session_view.impulse.intensity > 0.);
            assert_eq!(
                events.try_iter().collect::<Vec<_>>(),
                vec![(DIE, Some(id.0)), (ARMOR, Some(id.0))]
            );
            assert_ne!(
                owner.objects[&id].model_condition_bits
                    & (1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_SPLATTED),
                0
            );
            assert_eq!(peer.objects[&id].health.current, 100.);
            assert!(!peer.objects[&id].status.on_die_started);
            assert_eq!(peer.session_view.impulse.intensity, 0.);
        },
    );
}

#[test]
fn stamp_clears_freefall_before_resting_request_and_rereads_effective_death_for_splatted() {
    use crate::game_logic::object::{ContactStep, StampContact};
    let (mut owner, id) = world();
    resting(&mut owner, id);
    let object = owner.objects.get_mut(&id).unwrap();
    let ContactStep::Damage { request, .. } =
        object.step_stamp_contact(StampContact::Start(0.), false)
    else {
        panic!("resting kill must yield at actual Body boundary");
    };
    assert!(request.kill);
    assert!(request.source.is_none());
    assert_eq!(
        request.kind,
        crate::game_logic::combat::DamageType::Unresistable
    );
    assert!(!object.status.disabled_freefall, "CPP875 precedes kill891");
    assert!(!object.is_in_freefall);
    assert_eq!(
        object.model_condition_bits
            & (1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL),
        0
    );

    // A rejected/dead falling attempt can return nonlethal while current Body
    // is still effectively dead. CPP865 observes the Body after that return.
    object.kill_when_resting_on_ground = false;
    object.health.current = 0.;
    object.status.effectively_dead = true;
    object.status.destroyed = false;
    object.was_airborne_last_frame = true;
    object.movement.velocity = Vec3::new(0., -10., 0.);
    object.min_fall_speed_for_damage = 0.;
    object.fall_height_damage_factor = 100.;
    object.set_status_disabled_freefall(true);
    owner.stamp_airborne_targets_from_locomotor(&[id]);
    assert_ne!(
        owner.objects[&id].model_condition_bits
            & (1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_SPLATTED),
        0
    );
    assert!(
        !owner.objects[&id].status.on_die_started,
        "already-dead Body cannot invoke onDie again"
    );
}
