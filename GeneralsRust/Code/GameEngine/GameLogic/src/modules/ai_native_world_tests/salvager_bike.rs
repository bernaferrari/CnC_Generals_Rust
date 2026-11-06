//! CPP TransportContain.cpp:290-299 / Weapon.cpp:3127-3132: a Hero+Salvager
//! rider boarding a CLIFF_JUMPER hands its secondary Weapon's next-shot frame,
//! last-reload frame AND status to the bike (transferNextShotStatsFrom).
use super::*;
use crate::weapon::{WeaponSlotType, WeaponStatus, WeaponTemplate, WeaponTemplateSet};

fn bike_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeSalvagerBike".into());
    base.add_kind_of(KindOf::Structure);
    base.add_kind_of(KindOf::CliffJumper);
    let data = crate::object::contain::transport_contain::TransportContainModuleData {
        slot_capacity: 1,
        base: crate::object::contain::open_contain::OpenContainModuleData {
            contain_max: 1,
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
            module_tag: "NativeBikeCargo".into(),
            data: Arc::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(data)),
            interface_mask: ModuleInterfaceType::CONTAIN,
        }],
    })
}

fn hero_salvager_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeSalvagerHero".into());
    base.add_kind_of(KindOf::Infantry);
    base.add_kind_of(KindOf::Hero);
    base.add_kind_of(KindOf::Salvager);
    let mut data = crate::object::update::AIUpdateModuleData::default();
    data.set_locomotor_set_entries(
        crate::common::LocomotorSetType::Normal,
        vec!["NativeEnterGround".into()],
    );
    let _ = infantry_template(); // registers NativeEnterGround
    Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "AIUpdateInterface".into(),
            module_tag: "NativeSalvagerAI".into(),
            data: Arc::new(data),
            interface_mask: ModuleInterfaceType::UPDATE,
        }],
    })
}

fn secondary_set(name: &str) -> WeaponTemplateSet {
    let mut set = WeaponTemplateSet::new();
    let mut weapon = WeaponTemplate::new(name.into());
    weapon.attack_range = 100.0;
    weapon.primary_damage = 10.0;
    weapon.clip_size = 2;
    set.set_weapon_template(WeaponSlotType::Secondary, Arc::new(weapon));
    set
}

fn arm(obj: &Arc<RwLock<Object>>, name: &str, frame: u32, game: &NativeMatch) {
    let mut o = obj.write().unwrap();
    o.weapon_set.add_weapon_template_set(secondary_set(name));
    let id = o.get_id();
    o.weapon_set
        .update_weapon_set_with_services(
            id,
            &Default::default(),
            Default::default(),
            frame,
            &game.bonuses,
        )
        .unwrap();
}

#[test]
fn salvager_boarding_bike_transfers_reloading_status_to_bike_weapon() {
    // Same thread-local binding as transport_remove's bind_test_world.
    if game_engine::common::common_services::try_common_services().is_none() {
        game_engine::common::common_services::install_common_services(Arc::new(
            game_engine::common::common_services::CommonServices::new(),
        ));
    }
    if crate::system::engine_stores::try_active_services().is_none() {
        crate::system::engine_stores::install_active(crate::system::engine_stores::new_for_world());
    }
    let mut game =
        NativeMatch::with_source_template(50.0, bike_template(), false, hero_salvager_template());
    arm(&game.source, "NativeSalvagerRiderGun", 7, &game);
    arm(&game.target, "NativeSalvagerBikeGun", 7, &game);
    {
        let mut rider = game.source.write().unwrap();
        let w = rider
            .get_weapon_in_slot_mut(WeaponSlotType::Secondary.into())
            .expect("rider secondary");
        w.ammo_in_clip = 0;
        w.set_possible_next_shot_frame(5000);
        w.set_status(WeaponStatus::ReloadingClip);
    }
    {
        // Bike's own secondary is loaded and between shots: only the rider's
        // transferred status can make it report RELOADING_CLIP.
        let mut bike = game.target.write().unwrap();
        let w = bike
            .get_weapon_in_slot_mut(WeaponSlotType::Secondary.into())
            .expect("bike secondary");
        w.ammo_in_clip = 2;
        w.set_status(WeaponStatus::BetweenFiringShots);
    }
    let now = board_cached(&mut game, 117);
    let bike = game.target.read().unwrap();
    let w = bike
        .get_weapon_in_slot(WeaponSlotType::Secondary.into())
        .expect("bike secondary");
    assert_eq!(
        w.when_we_can_fire_again, 5000,
        "next-shot frame transferred"
    );
    assert_eq!(
        w.get_status_at(now),
        WeaponStatus::ReloadingClip,
        "C++ m_status = weapon.getStatus(): bike inherits the rider's reload"
    );
}

fn board_cached(game: &mut NativeMatch, frame: u32) -> u32 {
    super::armed_transport_tests::board_cached_source(game, frame)
}
