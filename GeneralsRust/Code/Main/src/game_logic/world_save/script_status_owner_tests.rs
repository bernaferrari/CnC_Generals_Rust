//! Actual Main script/map state, C++ Object.cpp1019/2015/3314/3505,
//! with a genuinely registered foreign Core owner reusing the same ID.
use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
use crate::save_load::snapshot::SnapshotBuilder;
use game_engine::common::{dict::Dict, well_known_keys};

fn world() -> (GameLogic, ObjectId) {
    let mut template = ThingTemplate::new("ScriptStatusOwner");
    template.add_kind_of(KindOf::Infantry);
    template.add_kind_of(KindOf::Selectable);
    world_from_template(template)
}

fn transport_world() -> (GameLogic, ObjectId) {
    let mut template = ThingTemplate::new("ScriptStatusOwner");
    template.add_kind_of(KindOf::Vehicle);
    template.add_kind_of(KindOf::Selectable);
    template.contain_module.kind = crate::game_logic::ContainModuleKind::Transport;
    template.contain_module.slots = Some(2);
    world_from_template(template)
}

fn world_from_template(template: ThingTemplate) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("ScriptStatusOwner", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    (world, id)
}

fn foreign_core(
    id: ObjectId,
) -> (
    gamelogic::object::object_factory::ObjectFactory,
    gamelogic::system::game_logic::GameLogic,
    std::sync::Arc<std::sync::RwLock<gamelogic::object::Object>>,
) {
    let mut factory = gamelogic::object::object_factory::ObjectFactory::new();
    let mut world = gamelogic::system::game_logic::GameLogic::new();
    let mut template = gamelogic::common::DefaultThingTemplate::new("ForeignScriptHero".into());
    template.add_kind_of(gamelogic::common::KindOf::Hero);
    let admitted = factory
        .prepare_object(
            &mut world,
            std::sync::Arc::new(template),
            gamelogic::common::Coord3D::ZERO,
            None,
            gamelogic::object::object_factory::ObjectCreationFlags::NO_AI,
        )
        .unwrap();
    assert_eq!(admitted, id.0);
    let object = factory
        .get_object(admitted)
        .unwrap()
        .get_base_object()
        .unwrap();
    object.write().unwrap().set_script_status(
        gamelogic::object::ObjectScriptStatusBit::ScriptTargetable,
        true,
    );
    assert_eq!(world.register_object(object.clone()).unwrap(), admitted);
    (factory, world, object)
}

fn assert_foreign_unchanged(object: &gamelogic::object::Object) {
    use gamelogic::object::ObjectScriptStatusBit;
    assert!(object.test_script_status_bit(ObjectScriptStatusBit::ScriptTargetable));
    assert!(!object.test_script_status_bit(ObjectScriptStatusBit::ScriptDisabled));
    assert!(!object.test_script_status_bit(ObjectScriptStatusBit::ScriptUnderpowered));
    assert!(!object.test_status(gamelogic::common::ObjectStatusTypes::Masked));
    assert!(object.is_kind_of(gamelogic::common::KindOf::Hero));
}

#[test]
fn main_script_and_map_flags_ignore_registered_foreign_core_id_and_survive_save() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "main_script_and_map_flags_ignore_registered_foreign_core_id_and_survive_save",
        || {
            let (mut main, id) = world();
            let (mut peer, peer_id) = world();
            assert_eq!(id, peer_id);
            let (_factory, _core, foreign) = foreign_core(id);
            assert_foreign_unchanged(&foreign.read().unwrap());
            assert!(!main.host_object(id).unwrap().is_script_targetable());
            assert!(
                !main.unit_is_hero(id),
                "the foreign Core hero is not this Main unit"
            );

            let mut properties = Dict::new();
            properties.set_bool(well_known_keys::key_object_targetable(), false);
            properties.set_bool(well_known_keys::key_object_enabled(), false);
            properties.set_bool(well_known_keys::key_object_powered(), false);
            properties.set_bool(well_known_keys::key_object_unsellable(), true);
            main.apply_update_obj_values_from_map_properties(id, &properties);
            let object = main.host_object_mut(id).unwrap();
            assert!(!object.is_script_targetable());
            assert!(object.is_script_disabled());
            assert!(object.is_script_underpowered());
            assert!(object.is_script_unsellable());
            object.select();
            assert!(object.selected && object.status.selected);
            object.set_status_masked(true);
            assert!(object.status.masked);
            assert!(!object.selected && !object.status.selected);
            object.set_script_targetable(true);
            let mut properties_without_script_flags = Dict::new();
            properties_without_script_flags
                .set_bool(well_known_keys::key_object_unsellable(), true);
            main.apply_update_obj_values_from_map_properties(id, &properties_without_script_flags);
            let unchanged = main.host_object(id).unwrap();
            assert!(unchanged.is_script_targetable());
            assert!(unchanged.is_script_disabled());
            assert!(unchanged.is_script_underpowered());
            assert!(unchanged.status.masked);
            assert!(!unchanged.selected && !unchanged.status.selected);
            assert_foreign_unchanged(&foreign.read().unwrap());
            assert!(!peer.host_object(id).unwrap().is_script_disabled());
            assert!(!peer.host_object(id).unwrap().is_script_underpowered());
            assert!(!peer.host_object(id).unwrap().status.masked);

            let snapshotter = SnapshotBuilder::new();
            let saved = snapshotter.create_world_snapshot(&main).unwrap();
            let peer_saved = snapshotter.create_world_snapshot(&peer).unwrap();
            let (mut receiving, _) = world();
            snapshotter
                .restore_from_snapshot(&saved, &mut receiving)
                .unwrap();
            let restored = receiving.host_object(id).unwrap();
            assert!(restored.is_script_targetable());
            assert!(restored.is_script_disabled());
            assert!(restored.is_script_underpowered());
            assert!(restored.is_script_unsellable());
            assert!(restored.status.masked);
            assert!(!restored.selected && !restored.status.selected);
            let mut targetable = Dict::new();
            targetable.set_bool(well_known_keys::key_object_targetable(), false);
            receiving.apply_update_obj_values_from_map_properties(id, &targetable);
            assert!(!receiving.host_object(id).unwrap().is_script_targetable());
            let restored = receiving.host_object_mut(id).unwrap();
            restored.set_script_disabled(false);
            restored.set_script_underpowered(false);
            restored.set_status_masked(false);
            assert!(!restored.is_disabled());
            assert!(!restored.status.masked);
            assert!(!restored.selected && !restored.status.selected);
            snapshotter
                .restore_from_snapshot(&peer_saved, &mut peer)
                .unwrap();
            assert!(!peer.host_object(id).unwrap().is_script_targetable());
            assert!(!peer.host_object(id).unwrap().is_disabled());
            assert!(!peer.host_object(id).unwrap().status.masked);
            assert!(main.host_object(id).unwrap().is_script_targetable());
            assert!(main.host_object(id).unwrap().status.masked);
            assert_foreign_unchanged(&foreign.read().unwrap());
        },
    );
}

#[test]
fn contained_hero_query_uses_driving_main_roster_and_survives_save() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "contained_hero_query_uses_driving_main_roster_and_survives_save",
        || {
            let (mut main, carrier_id) = transport_world();
            let (peer, peer_id) = transport_world();
            assert_eq!(carrier_id, peer_id);
            let (_factory, _core, foreign) = foreign_core(carrier_id);
            let mut hero = ThingTemplate::new("OwnedPassengerHero");
            hero.add_kind_of(KindOf::Infantry);
            hero.add_kind_of(KindOf::Hero);
            main.templates.insert(hero.name.clone(), hero);
            let hero_id = main
                .create_object("OwnedPassengerHero", Team::USA, glam::Vec3::ZERO)
                .unwrap();
            // The same public Main roster used by normal containment owns
            // these identities. Hero classification does not replay entry.
            let carrier = main.host_object_mut(carrier_id).unwrap();
            assert_eq!(carrier.max_transport, 2);
            assert!(carrier.add_occupant(hero_id));
            main.set_fixture_object_contained_by(hero_id, Some(carrier_id));
            assert!(main.host_object(hero_id).unwrap().is_hero());
            assert!(!main.host_object(carrier_id).unwrap().is_hero());
            assert!(main.unit_is_hero(carrier_id));
            assert!(!peer.unit_is_hero(carrier_id));
            assert_foreign_unchanged(&foreign.read().unwrap());
            let snapshotter = SnapshotBuilder::new();
            let saved = snapshotter.create_world_snapshot(&main).unwrap();
            let mut receiving = GameLogic::new();
            receiving.templates = main.templates.clone();
            snapshotter
                .restore_from_snapshot(&saved, &mut receiving)
                .unwrap();
            assert!(receiving.unit_is_hero(carrier_id));
            {
                let (object, ownership_events) = receiving
                    .host_object_and_ownership_events_mut(carrier_id)
                    .unwrap();
                object.remove_occupant(hero_id, ownership_events)
            };
            assert!(!receiving.unit_is_hero(carrier_id));
            assert!(main.unit_is_hero(carrier_id));
            assert!(!peer.unit_is_hero(carrier_id));
            assert_foreign_unchanged(&foreign.read().unwrap());
        },
    );
}
