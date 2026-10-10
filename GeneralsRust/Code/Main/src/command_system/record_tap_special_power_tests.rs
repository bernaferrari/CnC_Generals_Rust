use super::*;
use crate::game_logic::game_logic::pose_owner_tests::isolated_at;
use game_engine::common::name_key_generator::NameKeyGenerator;

#[test]
fn registered_special_power_decodes_without_prior_recording() {
    isolated_at(
        module_path!(),
        "registered_special_power_decodes_without_prior_recording",
        || {
            NameKeyGenerator::init();
            let id = intern_name("SP::SpySatellite");
            let message = GameMessage::with_player(GameMessageType::DoSpecialPower(id, 0, 0), 2);
            let command = game_message_to_host_command(&message).expect("registered replay power");
            assert_eq!(command.player_id, 2);
            assert!(matches!(
                command.command_type,
                CommandType::DoSpecialPower {
                    power_type: SpecialPowerType::SpySatellite,
                    target: PowerTarget::None
                }
            ));
            for name in [
                "SpySatellite",
                "SP::UnknownPower",
                "SP::",
                "SP::SpySatelliteSuffix",
            ] {
                assert_eq!(
                    resolve_special(intern_name(name)),
                    SpecialPowerType::Invalid
                );
            }
            assert_eq!(resolve_special(u32::MAX), SpecialPowerType::Invalid);
        },
    );
}

#[test]
fn special_power_ids_follow_name_registry_reset() {
    isolated_at(
        module_path!(),
        "special_power_ids_follow_name_registry_reset",
        || {
            NameKeyGenerator::init();
            let old = intern_special(&SpecialPowerType::RadarScan);
            NameKeyGenerator::init();
            let ordinary = intern_name("UnitTemplate");
            assert_eq!(ordinary, old, "the reset reused the previous numeric ID");
            let current = intern_special(&SpecialPowerType::RadarScan);
            assert_ne!(
                current, ordinary,
                "a reset must invalidate the cached numeric ID"
            );
            assert_eq!(resolve_special(old), SpecialPowerType::Invalid);
            let message = GameMessage::with_player(
                GameMessageType::DoSpecialPowerAtObject(current, 17, 0, 0),
                2,
            );
            let restored =
                game_message_to_host_command(&message).expect("current power must decode");
            assert!(matches!(
                restored.command_type,
                CommandType::DoSpecialPower {
                    power_type: SpecialPowerType::RadarScan,
                    target: PowerTarget::Object(ObjectId(17))
                }
            ));
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn special_power_resolution_stays_with_its_name_namespace() {
    isolated_at(
        module_path!(),
        "special_power_resolution_stays_with_its_name_namespace",
        || {
            let (ready, receive_ready) = std::sync::mpsc::channel();
            let (resume, receive_resume) = std::sync::mpsc::channel();
            let first = std::thread::spawn(move || {
                NameKeyGenerator::init();
                let id = intern_special(&SpecialPowerType::RadarScan);
                ready.send(id).unwrap();
                receive_resume.recv().unwrap();
                (id, resolve_special(id))
            });
            let first_id = receive_ready.recv().unwrap();
            let second = std::thread::spawn(|| {
                NameKeyGenerator::init();
                let id = intern_special(&SpecialPowerType::SpyDrone);
                (id, resolve_special(id))
            })
            .join()
            .unwrap();
            resume.send(()).unwrap();
            let first = first.join().unwrap();
            assert_eq!(
                first_id, second.0,
                "independent registries reuse numeric IDs"
            );
            assert_eq!(second.1, SpecialPowerType::SpyDrone);
            assert_eq!(
                first.1,
                SpecialPowerType::RadarScan,
                "another namespace must not overwrite this power"
            );
        },
    );
}
