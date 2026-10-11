//! CPP TransportContain.cpp:198-251,259-300: HELD/LOADED, then the
//! actual infantry weapon scan and immediate PlayerUpgrade weapon rebuild.
use super::*;
use crate::modules::ai_world::AiWorld;
use crate::weapon::{WeaponSetType, WeaponSlotType, WeaponTemplate, WeaponTemplateSet};

fn armed_transport_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeArmedTransport".into());
    base.add_kind_of(KindOf::Structure);
    let data = crate::object::contain::transport_contain::TransportContainModuleData {
        armed_riders_upgrade_weapon_set: true,
        slot_capacity: 2,
        base: crate::object::contain::open_contain::OpenContainModuleData {
            contain_max: 2,
            number_of_exit_paths: 0,
            allow_inside_kind_of: KindOf::Infantry.cpp_mask(),
            ..Default::default()
        },
        ..Default::default()
    };
    Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "TransportContain".into(),
            module_tag: "NativeArmedCargo".into(),
            data: Arc::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(data)),
            interface_mask: ModuleInterfaceType::CONTAIN,
        }],
    })
}

fn weapon_set(name: &str, damage: f32, range: f32, upgraded: bool) -> WeaponTemplateSet {
    let mut set = WeaponTemplateSet::new();
    if upgraded {
        set.conditions.set(WeaponSetType::PlayerUpgrade);
    }
    let mut weapon = WeaponTemplate::new(name.into());
    weapon.attack_range = range;
    weapon.primary_damage = damage;
    weapon.clip_size = 3;
    set.set_weapon_template(WeaponSlotType::Primary, Arc::new(weapon));
    set
}

fn install_weapon_sets(game: &mut NativeMatch, ranged_rider: bool, frame: u32) {
    let mut rider = game.source.write().unwrap();
    rider.weapon_set.add_weapon_template_set(weapon_set(
        "NativeRiderWeapon",
        20.0,
        if ranged_rider { 100.0 } else { 1.0 },
        false,
    ));
    let id = rider.get_id();
    rider
        .weapon_set
        .update_weapon_set_with_services(
            id,
            &Default::default(),
            Default::default(),
            frame,
            &game.bonuses,
        )
        .unwrap();
    drop(rider);
    let mut owner = game.target.write().unwrap();
    owner
        .weapon_set
        .add_weapon_template_set(weapon_set("NativeBaseWeapon", 1.0, 100.0, false));
    owner
        .weapon_set
        .add_weapon_template_set(weapon_set("NativeUpgradedWeapon", 70.0, 100.0, true));
    let id = owner.get_id();
    owner
        .weapon_set
        .update_weapon_set_with_services(
            id,
            &Default::default(),
            Default::default(),
            frame,
            &game.bonuses,
        )
        .unwrap();
}

fn board_cached_source(game: &mut NativeMatch, frame: u32) -> u32 {
    game.issue_enter(frame);
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let mut completed = frame;
    for now in frame..frame + 4 {
        game.operate(now, |world| {
            world.process_enter_path_queue(8).unwrap();
            if ai.lock().unwrap().get_current_state_id()
                == Some(crate::ai::states::AIStateType::Enter as u32)
            {
                ai.lock()
                    .unwrap()
                    .update_enter_phase_with_world(world)
                    .unwrap();
            }
        });
        completed = now;
        if game.source.read().unwrap().get_contained_by().is_some() {
            break;
        }
    }
    assert_eq!(
        game.source.read().unwrap().get_contained_by(),
        Some(game.target.read().unwrap().get_id())
    );
    completed
}

#[test]
fn armed_rider_rebuilds_transport_at_boarding_frame_without_touching_same_id_peer() {
    let mut game = NativeMatch::with_container(50.0, armed_transport_template());
    let mut peer = NativeMatch::with_container(190.0, armed_transport_template());
    install_weapon_sets(&mut game, true, 7);
    install_weapon_sets(&mut peer, true, 901);
    assert_eq!(
        game.target.read().unwrap().get_id(),
        peer.target.read().unwrap().get_id()
    );
    let peer_owner = peer.target.write().unwrap();
    let peer_rider = peer.source.write().unwrap();
    let now = board_cached_source(&mut game, 117);
    let owner = game.target.read().unwrap();
    let rider = game.source.read().unwrap();
    assert!(rider.is_disabled_by_type(crate::common::DisabledType::Held));
    assert!(owner.test_weapon_set_flag(WeaponSetType::PlayerUpgrade));
    let weapon = owner.get_current_weapon().unwrap().0;
    assert_eq!(weapon.get_template().get_name(), "NativeUpgradedWeapon");
    assert_eq!(weapon.when_last_reload_started, now);
    assert_eq!(weapon.get_remaining_ammo_at(now), 3);
    assert_eq!(
        owner
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contained_objects()
            .as_ref(),
        &[rider.get_id()]
    );
    assert!(!peer_owner.test_weapon_set_flag(WeaponSetType::PlayerUpgrade));
    assert_eq!(
        peer_owner
            .get_current_weapon()
            .unwrap()
            .0
            .get_template()
            .get_name(),
        "NativeBaseWeapon"
    );
    assert_eq!(
        peer_owner
            .get_current_weapon()
            .unwrap()
            .0
            .when_last_reload_started,
        901
    );
    assert_eq!(peer_rider.get_contained_by(), None);
}

#[test]
fn contact_only_rider_does_not_grant_upgrade_and_replacement_is_rechecked() {
    let mut contact = NativeMatch::with_container(50.0, armed_transport_template());
    install_weapon_sets(&mut contact, false, 3);
    board_cached_source(&mut contact, 127);
    assert!(
        !contact
            .target
            .read()
            .unwrap()
            .test_weapon_set_flag(WeaponSetType::PlayerUpgrade)
    );

    let mut game = NativeMatch::with_container(50.0, armed_transport_template());
    install_weapon_sets(&mut game, true, 4);
    board_cached_source(&mut game, 137);
    assert!(
        game.target
            .read()
            .unwrap()
            .test_weapon_set_flag(WeaponSetType::PlayerUpgrade)
    );
    // Admit a distinct upgrade condition and execute its real flag/rebuild.
    // Equal-condition additions retain the first sparse-match winner; merely
    // appending another default definition does not replace an installed weapon.
    // Replace the actual installed rider weapon with a contact-only set. The
    // next real boarding callback must scan all current riders, not a cached
    // "armed at admission" bit or a peer registry entry.
    {
        let mut rider = game.source.write().unwrap();
        let mut replacement = weapon_set("ReplacedContactWeapon", 20.0, 1.0, false);
        replacement.conditions.set(WeaponSetType::PlayerUpgrade);
        rider.weapon_set.add_weapon_template_set(replacement);
        let bonus_flags = rider.get_weapon_bonus_condition();
        rider
            .set_weapon_set_flag_with_services(
                WeaponSetType::PlayerUpgrade,
                true,
                bonus_flags,
                147,
                &game.bonuses,
            )
            .unwrap();
        let installed = rider.get_current_weapon().unwrap().0;
        assert_eq!(installed.get_template().get_name(), "ReplacedContactWeapon");
        assert!(installed.is_contact_weapon());
        assert!(installed.is_damage_weapon());
    }
    let team = game
        .players
        .get_player(0)
        .unwrap()
        .read()
        .unwrap()
        .get_default_team();
    let second = game
        .factory
        .prepare_object(
            infantry_template(),
            Coord3D::new(50.0, 50.0, 0.0),
            team,
            ObjectCreationFlags::empty(),
        )
        .unwrap();
    let goal = game.target.read().unwrap().get_id();
    let first_id = game.source.read().unwrap().get_id();
    assert_ne!(second, first_id);
    assert_ne!(second, goal);
    let second_object = game
        .factory
        .get_object(second)
        .unwrap()
        .get_base_object()
        .unwrap();
    {
        let second = second_object.read().unwrap();
        assert_eq!(second.get_contained_by(), None);
        assert!(
            !second.has_any_weapon(),
            "the second actual rider is unarmed"
        );
    }
    game.operate(147, |world| world.enter(second, goal).unwrap());
    assert_eq!(second_object.read().unwrap().get_contained_by(), Some(goal));
    let owner = game.target.read().unwrap();
    assert_eq!(
        owner
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contained_objects()
            .as_ref(),
        &[first_id, second]
    );
    assert!(!owner.test_weapon_set_flag(WeaponSetType::PlayerUpgrade));
    assert_eq!(
        owner
            .get_current_weapon()
            .unwrap()
            .0
            .get_template()
            .get_name(),
        "NativeBaseWeapon"
    );
    assert_eq!(
        owner
            .get_current_weapon()
            .unwrap()
            .0
            .when_last_reload_started,
        147
    );
    assert_eq!(
        owner
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contained_objects()
            .as_ref(),
        &[game.source.read().unwrap().get_id(), second]
    );
}
