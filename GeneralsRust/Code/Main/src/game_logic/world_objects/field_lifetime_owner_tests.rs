//! C++ DeletionUpdate vs LifetimeUpdate, using receiving native field objects.
use super::super::*;
use crate::game_logic::host_inferno_cannon::INFERNO_FIRE_FIELD_TEMPLATE;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::special_power_strikes::{
    ANTHRAX_TOXIN_OBJECT_NAME, NUKE_RADIATION_OBJECT_NAME,
};

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

fn spawned_at(field: Field, inactive: bool, frame: u32) -> (GameLogic, ObjectId, u32) {
    let mut parser = crate::assets::IniParser::new();
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
    parser.parse_ini_content(&format!(
        "Object {name}\nKindOf = IMMOBILE INERT NO_COLLIDE\nBody = {body} BodyTag\n{health}\nEnd\nBehavior = {module} ExpiryTag\nMinLifetime = {duration}\nMaxLifetime = {duration}\nEnd\nBehavior = InstantDeathBehavior DeathWitness\nDeathTypes = ALL -LASERED -DETONATED\nFX = OwnedFieldExpiryWitness\nEnd\nEnd"
    ), "field-lifetime-owner.ini").unwrap();
    let template = GameLogic::build_template_from_object_definition(
        name,
        parser.get_definition(name).unwrap(),
        None,
    );
    let mut world = GameLogic::new();
    world.frame = frame;
    world.templates.insert(name.into(), template);
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

fn normal_lifetime_die(field: Field) {
    for inactive in [true, false] {
        let (mut world, id, due) = spawned(field, inactive);
        world.host_object_mut(id).unwrap().status.death_type = HostDeathType::Lasered;
        world.frame = due - 1;
        field.update(&mut world);
        assert!(!world.has_pending_object_removals());
        world.frame = due;
        field.update(&mut world);
        let object = world.host_object(id).unwrap();
        assert!(object.status.on_die_started);
        assert_eq!(object.status.death_type, HostDeathType::Normal);
        assert_eq!(
            object.pending_death_fx.as_deref(),
            Some("OwnedFieldExpiryWitness")
        );
        assert_eq!(
            object.lifetime_update.as_ref().unwrap().expire_at_frame,
            due
        );
        assert!(!object.lifetime_update.as_ref().unwrap().active);
        assert!(field.deadline(object).is_none());
        world.host_object_mut(id).unwrap().pending_death_fx.take();
        field.update(&mut world);
        world.update_ai(&[id], 1.0 / 30.0);
        assert!(world.host_object(id).unwrap().pending_death_fx.is_none());
        world.process_destroy_list();
        assert!(world.host_object(id).is_none());
    }
}

#[test]
fn toxin_lifetime_uses_normal_body_kill_once() {
    normal_lifetime_die(Field::Toxin);
}
#[test]
fn radiation_lifetime_uses_normal_body_kill_once() {
    normal_lifetime_die(Field::Radiation);
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
    let lifetime = object.lifetime_update.as_ref().unwrap();
    assert_eq!(lifetime.expire_at_frame, due);
    assert!(!lifetime.active);
    assert!(field.deadline(object).is_none());

    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut fresh = GameLogic::new();
    builder.restore_from_snapshot(&decoded, &mut fresh).unwrap();
    builder.restore_from_snapshot(&decoded, &mut world).unwrap();
    for receiving in [&mut fresh, &mut world] {
        let object = receiving.host_object(id).unwrap();
        assert!(object.indestructible);
        assert_eq!(
            object.lifetime_update.as_ref().unwrap().expire_at_frame,
            due
        );
        assert!(!object.lifetime_update.as_ref().unwrap().active);
        receiving
            .host_object_mut(id)
            .unwrap()
            .set_indestructible(false);
        receiving.frame = due + 100;
        field.update(receiving);
        receiving.update_ai(&[id], 1.0 / 30.0);
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
        let lifetime = world
            .host_object(id)
            .unwrap()
            .lifetime_update
            .as_ref()
            .unwrap();
        assert!(lifetime.active);
        assert_eq!(lifetime.expire_at_frame, due);
        let builder = crate::save_load::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let mut receiving = GameLogic::new();
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
        assert_eq!(clone.lifetime_update.as_ref().unwrap().expire_at_frame, due);
        assert!(clone.lifetime_update.as_ref().unwrap().active);
        owner.frame = due - 1;
        field.update(&mut owner);
        assert!(owner.host_object(id).unwrap().is_alive());
        assert_eq!(
            owner
                .host_object(id)
                .unwrap()
                .lifetime_update
                .as_ref()
                .unwrap()
                .expire_at_frame,
            due
        );
        owner.frame = due;
        field.update(&mut owner);
        assert!(owner.host_object(id).unwrap().status.on_die_started);
    }
}
