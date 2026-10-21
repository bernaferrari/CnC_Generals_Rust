//! Armor.cpp117-132 clears each named block; ArmorStore::reset is a no-op.
//! GameLogic.cpp2404-2415 dispatches Map.ini followed by Solo.ini.
use super::*;
use crate::game_logic::{GameLogic, HostArmorSet, KindOf, Team, ThingTemplate};
use glam::Vec3;
fn catalog(percent: u32) -> Arc<ArmorDefinitions> {
    Arc::new(
        ArmorDefinitions::load_from_str(&format!(
            "Armor SharedArmor\n Armor = Default {percent}%\nEnd\n"
        ))
        .unwrap(),
    )
}
fn coefficient(world: &GameLogic) -> f32 {
    world
        .current_armor_definitions()
        .unwrap()
        .find_template("SharedArmor")
        .unwrap()
        .adjust_damage(gamelogic::damage::DamageType::Explosion, 100.0)
}
fn map_at(directory: &std::path::Path) -> std::path::PathBuf {
    let mut data = game_engine::common::system::DataChunkOutput::new();
    data.open_data_chunk("HeightMapData", 3);
    for value in [8, 8, 0, 64] {
        data.write_int(value);
    }
    for _ in 0..64 {
        data.write_byte(40);
    }
    data.close_data_chunk();
    let path = directory.join("OwnedArmor.map");
    std::fs::write(&path, data.into_ckmp_bytes()).unwrap();
    path
}
#[test]
fn actual_armor_map_solo_reset_and_factory_damage_use_receiving_catalog() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_armor_map_solo_reset_and_factory_damage_use_receiving_catalog",
        || {
            let directory = tempfile::tempdir().unwrap();
            let path = map_at(directory.path());
            std::fs::write(
                directory.path().join("Map.ini"),
                "Armor SharedArmor\n Armor = Explosion 5%\n Armor = Default 50%\nEnd\n",
            )
            .unwrap();
            std::fs::write(
                directory.path().join("Solo.ini"),
                "Armor SharedArmor\n Armor = Default 75%\n Armor = Explosion 25%\nEnd\n",
            )
            .unwrap();
            let peer_directory = tempfile::tempdir().unwrap();
            let peer_path = map_at(peer_directory.path());
            let mut mapless = GameLogic::new();
            assert!(mapless.load_map(peer_path.to_str().unwrap()));
            assert!(
                mapless.current_armor_definitions().is_none(),
                "explicit mapless profile survives a map with no Armor blocks"
            );
            let mut a = GameLogic::new();
            let mut b = GameLogic::new();
            assert!(
                a.current_armor_definitions().is_none(),
                "constructors never load/publish armor"
            );
            a.admit_armor_definitions(catalog(100));
            b.admit_armor_definitions(catalog(200));
            let foreign = gamelogic::object::armor::TheArmorStore::write();
            let before = foreign.len();
            assert!(a.load_map(path.to_str().unwrap()));
            assert_eq!(
                coefficient(&a),
                25.0,
                "Solo is last, and row order survives DEFAULT"
            );
            assert_eq!(coefficient(&b), 200.0);
            assert!(b.load_map(peer_path.to_str().unwrap()));
            assert_eq!(coefficient(&b), 200.0);
            let mut template = ThingTemplate::new("OwnedArmorVehicle");
            template.add_kind_of(KindOf::Vehicle);
            template.set_health(1000.0);
            template.armor_sets.push(HostArmorSet {
                conditions: 0,
                armor: Some("SharedArmor".into()),
                damage_fx: None,
            });
            a.templates.insert(template.name.clone(), template.clone());
            b.templates.insert(template.name.clone(), template);
            let aid = a
                .create_object("OwnedArmorVehicle", Team::USA, Vec3::ZERO)
                .unwrap();
            let bid = b
                .create_object("OwnedArmorVehicle", Team::USA, Vec3::ZERO)
                .unwrap();
            assert_eq!(aid, bid);
            let mut events = crate::game_logic::HostHealthEvents::default();
            a.objects.get_mut(&aid).unwrap().take_damage_from_typed(
                100.0,
                None,
                crate::game_logic::combat::DamageType::Explosive,
                &mut events,
            );
            assert_eq!(a.objects[&aid].health.current, 975.0);
            assert_eq!(b.objects[&bid].health.current, 1000.0);
            b.objects.get_mut(&bid).unwrap().take_damage_from_typed(
                100.0,
                None,
                crate::game_logic::combat::DamageType::Explosive,
                &mut events,
            );
            assert_eq!(b.objects[&bid].health.current, 800.0);
            b.reset();
            assert_eq!(coefficient(&a), 25.0);
            a.reset();
            assert_eq!(
                coefficient(&a),
                25.0,
                "original ArmorStore reset retains definitions"
            );
            assert!(a.load_map(peer_path.to_str().unwrap()));
            assert_eq!(
                coefficient(&a),
                25.0,
                "next map without Armor retains current rows"
            );
            std::fs::write(
                directory.path().join("Solo.ini"),
                "Armor SharedArmor\n Armor = NOT_A_DAMAGE_TYPE 25%\nEnd\n",
            )
            .unwrap();
            assert!(!a.load_map(path.to_str().unwrap()));
            assert_eq!(
                coefficient(&a),
                25.0,
                "rejected content cannot publish Map partial rows"
            );
            assert_eq!(
                foreign.len(),
                before,
                "real load/create/damage/reset never reads or writes foreign ArmorStore"
            );
        },
    );
}
