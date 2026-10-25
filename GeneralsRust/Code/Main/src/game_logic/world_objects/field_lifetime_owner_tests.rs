//! C++ DeletionUpdate vs LifetimeUpdate, using receiving native field objects.
use super::super::*;
use crate::game_logic::host_inferno_cannon::INFERNO_FIRE_FIELD_TEMPLATE;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::native_lifecycle::NativeLifecycleState;
use crate::game_logic::special_power_strikes::{ANTHRAX_TOXIN_OBJECT_NAME, NUKE_RADIATION_OBJECT_NAME};

#[cfg(feature = "game_client")]
struct ExpiryWitness(std::sync::mpsc::Sender<gamelogic::helpers::HostFxObjectPose>);

#[cfg(feature = "game_client")]
impl game_client::fx_list::FXNugget for ExpiryWitness {
    fn do_fx_pos(
        &self,
        _: Option<&gamelogic::common::Coord3D>,
        _: Option<&gamelogic::common::Matrix3D>,
        _: f32,
        _: Option<&gamelogic::common::Coord3D>,
        _: f32,
    ) {
        panic!("InstantDeath expiry FX must observe the installed field object");
    }
    fn do_fx_obj_host(
        &self,
        primary: &gamelogic::helpers::HostFxObjectPose,
        secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
    ) {
        assert!(secondary.is_none(), "InstantDeath FX has no secondary");
        self.0.send(*primary).unwrap();
    }
}

#[cfg(feature = "game_client")]
fn install_expiry_witness() -> std::sync::mpsc::Receiver<gamelogic::helpers::HostFxObjectPose> {
    let (send, receive) = std::sync::mpsc::channel();
    let mut fx = game_client::fx_list::FXList::new();
    fx.add_fx_nugget(Box::new(ExpiryWitness(send)));
    game_client::fx_list::get_fx_list_store_mut().add_fx_list("OwnedFieldExpiryWitness".into(), fx);
    receive
}

#[derive(Clone, Copy)]
enum Field {
    Fire,
    Toxin,
    Radiation,
}

impl Field {
    fn name(self) -> &'static str {
        match self {
            Self::Fire => INFERNO_FIRE_FIELD_TEMPLATE,
            Self::Toxin => ANTHRAX_TOXIN_OBJECT_NAME,
            Self::Radiation => NUKE_RADIATION_OBJECT_NAME,
        }
    }
    fn update(self, world: &mut GameLogic) {
        match self {
            Self::Fire => world.update_inferno_fire_field_objects(),
            Self::Toxin => world.update_anthrax_toxin_field_objects(),
            Self::Radiation => world.update_nuke_radiation_field_objects(),
        }
    }
    fn deadline(self, object: &Object) -> Option<u32> {
        match self {
            Self::Fire => object.inferno_fire_field_expires_frame,
            Self::Toxin => object.anthrax_toxin_field_expires_frame,
            Self::Radiation => object.nuke_radiation_field_expires_frame,
        }
    }
}

fn spawned(field: Field, inactive: bool) -> (GameLogic, ObjectId, u32) {
    spawned_at(field, inactive, 11)
}

fn admit_field_definition(world: &mut GameLogic, field: Field, inactive: bool) {
    let body = if inactive {
        "InactiveBody"
    } else {
        "ActiveBody"
    };
    let health = if inactive {
        ""
    } else {
        "MaxHealth = 100\nInitialHealth = 100"
    };
    let (module, duration) = match field {
        Field::Fire => ("DeletionUpdate", 2500),
        Field::Toxin => ("LifetimeUpdate", 60000),
        Field::Radiation => ("LifetimeUpdate", 30000),
    };
    let name = field.name();
    let source = format!(
        "Object {name}\nKindOf = IMMOBILE INERT NO_COLLIDE\nBody = {body} BodyTag\n{health}\nEnd\nBehavior = {module} ExpiryTag\nMinLifetime = {duration}\nMaxLifetime = {duration}\nEnd\nBehavior = InstantDeathBehavior DeathWitness\nDeathTypes = ALL -LASERED -DETONATED\nFX = OwnedFieldExpiryWitness\nEnd\nEnd"
    );
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "field-lifetime-owner.ini".into(),
                source,
            )])
            .unwrap(),
        )
        .unwrap();
}

fn spawned_at(field: Field, inactive: bool, frame: u32) -> (GameLogic, ObjectId, u32) {
    let mut world = GameLogic::new();
    world.frame = frame;
    admit_field_definition(&mut world, field, inactive);
    let id = match field {
        Field::Fire => world
            .spawn_inferno_fire_field_object(1, Vec3::ZERO, false, Team::USA)
            .unwrap(),
        Field::Toxin => {
            world.special_power_strikes.spawn_toxin_field(
                ObjectId(900),
                Team::USA,
                Vec3::ZERO,
                world.frame,
                0,
            );
            world.spawn_anthrax_toxin_field_objects_for_new_fields();
            world.special_power_strikes.toxin_fields()[0]
                .object_id
                .unwrap()
        }
        Field::Radiation => {
            world.special_power_strikes.spawn_radiation_field(
                ObjectId(900),
                Team::USA,
                Vec3::ZERO,
                world.frame,
                0,
            );
            world.spawn_nuke_radiation_field_objects_for_new_fields();
            world.special_power_strikes.radiation_fields()[0]
                .object_id
                .unwrap()
        }
    };
    let deadline = field.deadline(world.host_object(id).unwrap()).unwrap();
    (world, id, deadline)
}

#[test]
fn fire_deletion_skips_body_and_die_for_both_body_classes() {
    for inactive in [true, false] {
        let (mut world, id, due) = spawned(Field::Fire, inactive);
        world.host_object_mut(id).unwrap().set_indestructible(true);
        let before_hp = world.host_object(id).unwrap().health.current;
        world.frame = due - 1;
        Field::Fire.update(&mut world);
        assert!(!world.has_pending_object_removals());
        world.frame = due;
        Field::Fire.update(&mut world);
        let object = world.host_object(id).unwrap();
        assert!(object.status.destroyed);
        assert!(!object.status.on_die_started);
        assert_eq!(object.health.current, before_hp);
        assert!(object.pending_death_fx.is_none());
        assert!(!object.inactive_body_die_called);
        Field::Fire.update(&mut world);
        world.process_destroy_list();
        assert!(world.host_object(id).is_none());
    }
}

fn canonical_lifetime(object: &Object) -> (u32, bool) {
    assert!(
        object.lifetime_update.is_none(),
        "native owner has no competing legacy timer"
    );
    object
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states
        .iter()
        .find_map(|state| match state {
            NativeLifecycleState::Lifetime { die_frame, active } => Some((*die_frame, *active)),
            _ => None,
        })
        .expect("authored LifetimeUpdate")
}

fn normal_lifetime_die(field: Field) {
    for inactive in [true, false] {
        let (mut world, id, due) = spawned(field, inactive);
        #[cfg(feature = "game_client")]
        let events = {
            let events = install_expiry_witness();
            world.add_player(Player::new(1, Team::USA, "FieldObserver", true));
            world
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .set_host_object_shroud_status(
                    1,
                    id.0,
                    gamelogic::common::ObjectShroudStatus::Clear,
                );
            events
        };
        world.host_object_mut(id).unwrap().status.death_type = HostDeathType::Lasered;
        world.frame = due - 1;
        field.update(&mut world);
        assert!(!world.has_pending_object_removals());
        #[cfg(feature = "game_client")]
        assert!(
            events.try_iter().next().is_none(),
            "expiry FX must wait for its deadline"
        );
        world.frame = due;
        field.update(&mut world);
        let object = world.host_object(id).unwrap();
        assert!(object.status.on_die_started);
        assert_eq!(object.status.death_type, HostDeathType::Normal);
        assert!(
            object.pending_death_fx.is_none(),
            "authored expiry FX is delivered synchronously, not replayed later"
        );
        let witness_slot = object
            .thing()
            .template
            .native_die_declarations
            .as_ref()
            .unwrap()
            .iter()
            .find(|slot| slot.module_tag.as_deref() == Some("DeathWitness"))
            .unwrap()
            .source_index;
        assert!(
            object.native_die_runtime.completed.contains(&witness_slot),
            "the authored InstantDeath slot completed"
        );
        #[cfg(feature = "game_client")]
        {
            let delivered: Vec<_> = events.try_iter().collect();
            assert_eq!(delivered.len(), 1, "one actual authored field expiry FX");
            assert_eq!(delivered[0].id, id.0);
            assert_eq!(delivered[0].position, gamelogic::common::Coord3D::ZERO);
        }
        assert_eq!(canonical_lifetime(object).0, due);
        assert!(!canonical_lifetime(object).1);
        assert!(field.deadline(object).is_none());
        field.update(&mut world);
        world
            .update_ai(&[id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
        assert!(world.host_object(id).unwrap().pending_death_fx.is_none());
        #[cfg(feature = "game_client")]
        assert!(
            events.try_iter().next().is_none(),
            "sleeping lifetime must not replay authored FX"
        );
        world.process_destroy_list();
        assert!(world.host_object(id).is_none());
    }
}

#[test]
fn toxin_lifetime_uses_normal_body_kill_once() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "toxin_lifetime_uses_normal_body_kill_once",
        || normal_lifetime_die(Field::Toxin),
    );
}
#[test]
fn radiation_lifetime_uses_normal_body_kill_once() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "radiation_lifetime_uses_normal_body_kill_once",
        || normal_lifetime_die(Field::Radiation),
    );
}

fn rejected_kill_stays_asleep(field: Field) {
    let (mut world, id, due) = spawned(field, false);
    world.host_object_mut(id).unwrap().set_indestructible(true);
    world.frame = due;
    field.update(&mut world);
    let object = world.host_object(id).unwrap();
    assert!(object.is_alive());
    assert_eq!(object.health.current, 100.0);
    assert!(!object.status.on_die_started);
    assert!(object.pending_death_fx.is_none());
    assert!(!world.has_pending_object_removals());
    let (deadline, active) = canonical_lifetime(object);
    assert_eq!(deadline, due);
    assert!(!active);
    assert!(field.deadline(object).is_none());

    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut fresh = GameLogic::new();
    // This fixture supplied custom immutable field authoring before creation.
    // A receiving factory must admit those same definitions before strict load.
    admit_field_definition(&mut fresh, field, false);
    builder.restore_from_snapshot(&decoded, &mut fresh).unwrap();
    builder.restore_from_snapshot(&decoded, &mut world).unwrap();
    for receiving in [&mut fresh, &mut world] {
        let object = receiving.host_object(id).unwrap();
        assert!(object.indestructible);
        assert_eq!(canonical_lifetime(object).0, due);
        assert!(!canonical_lifetime(object).1);
        receiving
            .host_object_mut(id)
            .unwrap()
            .set_indestructible(false);
        receiving.frame = due + 100;
        field.update(receiving);
        receiving
            .update_ai(&[id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
        assert!(receiving.host_object(id).unwrap().is_alive());
        assert_eq!(receiving.host_object(id).unwrap().health.current, 100.0);
        assert!(!receiving.has_pending_object_removals());
    }
}

#[test]
fn toxin_rejected_lifetime_stays_asleep_through_save_and_ai() {
    rejected_kill_stays_asleep(Field::Toxin);
}
#[test]
fn radiation_rejected_lifetime_stays_asleep_through_save_and_ai() {
    rejected_kill_stays_asleep(Field::Radiation);
}

#[test]
fn field_creation_binds_one_receiving_lifetime_and_preserves_deadline_on_load() {
    for field in [Field::Toxin, Field::Radiation] {
        let (world, id, due) = spawned(field, false);
        let (deadline, active) = canonical_lifetime(world.host_object(id).unwrap());
        assert!(active);
        assert_eq!(deadline, due);
        let builder = crate::save_load::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let mut receiving = GameLogic::new();
        admit_field_definition(&mut receiving, field, false);
        builder
            .restore_from_snapshot(&snapshot, &mut receiving)
            .unwrap();
        // A legacy serialized projection must not become another live timer.
        let object = receiving.host_object_mut(id).unwrap();
        match field {
            Field::Toxin => object.anthrax_toxin_field_expires_frame = Some(due - 5),
            Field::Radiation => object.nuke_radiation_field_expires_frame = Some(due - 5),
            Field::Fire => unreachable!(),
        }
        receiving.frame = due - 1;
        field.update(&mut receiving);
        assert!(receiving.host_object(id).unwrap().is_alive());
        receiving.frame = due;
        field.update(&mut receiving);
        assert!(receiving.host_object(id).unwrap().status.on_die_started);
    }
}

#[test]
fn same_id_field_deadlines_survive_peer_reset_clone_and_replacement() {
    for field in [Field::Toxin, Field::Radiation] {
        let (mut owner, id, due) = spawned_at(field, false, 11);
        let (mut peer, peer_id, peer_due) = spawned_at(field, false, 711);
        assert_eq!(id, peer_id);
        assert_ne!(due, peer_due);
        let clone = owner.host_object(id).unwrap().clone();
        peer.reset();
        drop(peer);
        let (mut replacement, replacement_id, replacement_due) = spawned_at(field, false, 21);
        assert_eq!(id, replacement_id);
        replacement.frame = replacement_due;
        field.update(&mut replacement);
        assert!(replacement.host_object(id).unwrap().status.on_die_started);
        assert_eq!(canonical_lifetime(&clone).0, due);
        assert!(canonical_lifetime(&clone).1);
        owner.frame = due - 1;
        field.update(&mut owner);
        assert!(owner.host_object(id).unwrap().is_alive());
        assert_eq!(canonical_lifetime(owner.host_object(id).unwrap()).0, due);
        owner.frame = due;
        field.update(&mut owner);
        assert!(owner.host_object(id).unwrap().status.on_die_started);
    }
}
