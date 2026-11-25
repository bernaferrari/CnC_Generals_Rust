//! Public save-boundary failure witness.
use super::*;
#[cfg(feature = "game_client")]
mod mission_particle_capture {
    use super::*;
    use crate::save_load::snapshot::ClientDrawableWorldSnapshot;
    use game_client::core::game_client::GameClient;
    use game_client::effects::ParticleSystemTemplate;
    use game_client::system::SubsystemInterface;
    use std::sync::Arc;

    #[test]
    fn particle_capture_mission_ignores_unsaved_native_system() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "particle_capture_mission_ignores_unsaved_native_system",
            || {
                let directory = tempfile::tempdir().unwrap();
                let mut saves = SaveFileManager::with_save_directory(directory.path());
                let mut save_info = fixture_save_info();
                save_info.save_type = SaveFileType::Mission;
                let world = GameLogic::new();
                let mut client = GameClient::new().expect("CPU-only GameClient");
                client.particle_systems_mut().reset().unwrap();
                let template = Arc::new(ParticleSystemTemplate::new("x".repeat(256)));
                client
                    .particle_systems_mut()
                    .register_template(template.clone());
                client
                    .particle_systems_mut()
                    .create_particle_system(&template, true)
                    .unwrap();
                let result = saves.save_game_with_client_state(
                    "mission_particle_capture",
                    &world,
                    ClientDrawableWorldSnapshot::default(),
                    &mut client,
                    &save_info,
                );
                client.particle_systems_mut().reset().unwrap();
                result.expect("mission save must not encode the excluded particle block");
                let bytes = std::fs::read(saves.get_save_path("mission_particle_capture")).unwrap();
                let archive = decode_versioned_archive(&bytes);
                assert_eq!(archive.save_info.save_type, SaveFileType::Mission);
                assert!(archive.snapshot.particle_system_archive.is_none());
                assert!(archive.campaign_state.is_some());
            },
        );
    }
}
