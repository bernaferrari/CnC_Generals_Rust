//! Public save-boundary failure witness.
use super::*;
#[cfg(feature = "game_client")]
mod particle_capture_failure_control {
    use super::*;
    use crate::game_logic::combat_particles::with_client_particle_manager;
    use game_client::effects::ParticleSystemTemplate;
    use game_client::system::SubsystemInterface;
    use std::sync::Arc;

    #[test]
    fn particle_capture_failure_preserves_existing_archive_and_creates_no_temp() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "particle_capture_failure_preserves_existing_archive_and_creates_no_temp",
            || {
                // C++ GameState.cpp:1354-1375 rethrows failed snapshot blocks;
                // XferSave.cpp's ASCII length is one byte, not a lossy null companion.
                with_client_particle_manager(|manager| manager.reset().unwrap()).unwrap();
                let save_info = fixture_save_info();
                let prior_archive = SaveFileManager::write_common_sav_chunks(
                    &WorldSnapshot::default(),
                    &save_info,
                    CPP_GAME_SINGLE_PLAYER,
                )
                .unwrap();
                let directory = tempfile::tempdir().unwrap();
                let mut saves = SaveFileManager::with_save_directory(directory.path());
                let filename = "particle_capture_failure";
                let destination = saves.get_save_path(filename);
                std::fs::write(&destination, &prior_archive).unwrap();
                let temp_directory = directory.path().join("temp");
                let temp_file = temp_directory.join(format!("{filename}_temp.tmp"));
                assert!(!temp_directory.exists());
                let world = GameLogic::new();

                let capture_result = with_client_particle_manager(|manager| {
                    let mut template = ParticleSystemTemplate::new("x".repeat(256));
                    template.info_mut().system_lifetime = 1000;
                    let template = Arc::new(template);
                    manager.register_template(template.clone());
                    manager.create_particle_system(&template, true).unwrap();
                    manager.capture_xfer_bytes()
                })
                .unwrap();
                let save_result = saves.save_game(filename, &world, &save_info);
                // Reset before checking OLD or NEW results: a failing OLD assertion must
                // not leave the deliberately unrepresentable system in the live manager.
                with_client_particle_manager(|manager| manager.reset().unwrap()).unwrap();

                let capture_error = capture_result.expect_err("256-byte names cannot be encoded");
                let save_error =
                    save_result.expect_err("a real particle Xfer failure must abort save");
                assert!(
                    matches!(save_error, SaveLoadError::Serialization(ref error) if error == &capture_error)
                );
                assert_eq!(std::fs::read(&destination).unwrap(), prior_archive);
                assert!(!temp_file.exists());
                assert!(!temp_directory.exists());
            },
        );
    }
}

#[cfg(feature = "game_client")]
mod mission_particle_capture {
    use super::*;
    use crate::game_logic::combat_particles::with_client_particle_manager;
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
                with_client_particle_manager(|manager| {
                    manager.reset().unwrap();
                    let template = Arc::new(ParticleSystemTemplate::new("x".repeat(256)));
                    manager.register_template(template.clone());
                    manager.create_particle_system(&template, true).unwrap();
                })
                .unwrap();
                let result = saves.save_game("mission_particle_capture", &world, &save_info);
                with_client_particle_manager(|manager| manager.reset().unwrap()).unwrap();
                result.expect("mission save must not encode the excluded particle block");
                let archive =
                    std::fs::read(saves.get_save_path("mission_particle_capture")).unwrap();
                let blocks = walk_named_chunks(&archive).unwrap();
                let names: Vec<_> = blocks.iter().map(|(name, _)| name.as_str()).collect();
                assert_eq!(names, [CHUNK_GAME_STATE, CHUNK_CAMPAIGN]);
            },
        );
    }
}
