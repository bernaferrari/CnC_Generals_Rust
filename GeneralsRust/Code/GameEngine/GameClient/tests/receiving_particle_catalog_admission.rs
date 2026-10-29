use game_client_rust::effects::particle_manager::ParticleSystemManager;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::ini_particle_sys::IniParticleSys;
use game_engine::common::system::file_system::get_file_system;
use game_engine::common::system::local_file_system::LocalFileSystem;
use std::collections::HashMap;
use std::fs;

#[test]
fn receiving_map_admission_uses_each_manager_base_and_ordered_overlays() {
    const COMMON_ONLY: &str = "Yju10gCommonOnlyParticle";
    let root = tempfile::tempdir().unwrap();
    let particle_ini = root.path().join("Data/INI/ParticleSystem.ini");
    fs::create_dir_all(particle_ini.parent().unwrap()).unwrap();
    fs::write(
        &particle_ini,
        "ParticleSystem Yju10gReceivingBase\n  SystemLifetime = 41\nEnd\n",
    )
    .unwrap();

    {
        let file_system = get_file_system();
        let mut file_system = file_system.lock().unwrap();
        let local: &mut LocalFileSystem = file_system.ensure_backend(LocalFileSystem::new);
        local.add_search_path(root.path());
        file_system.clear_cache();
    }

    let common_template = IniParticleSys::parse_particle_system_block(
        AsciiString::from(COMMON_ONLY),
        HashMap::from([(String::from("SystemLifetime"), String::from("99"))]),
    )
    .unwrap();
    IniParticleSys::register_template(common_template).unwrap();

    let source_a_initial =
        String::from("ParticleSystem Yju10gReceivingOverlay\n  SystemLifetime = 11\nEnd\n");
    let source_a_final =
        String::from("ParticleSystem Yju10gReceivingOverlay\n  SystemLifetime = 17\nEnd\n");
    let source_b_initial =
        String::from("ParticleSystem Yju10gReceivingOverlay\n  SystemLifetime = 23\nEnd\n");
    let source_b_final =
        String::from("ParticleSystem Yju10gReceivingOverlay\n  SystemLifetime = 29\nEnd\n");
    let mut a = ParticleSystemManager::new();
    let mut b = ParticleSystemManager::new();
    a.admit_map_definition_overlays(&[source_a_initial, source_a_final])
        .unwrap();
    b.admit_map_definition_overlays(&[source_b_initial, source_b_final])
        .unwrap();
    // Normal Main FX spawn sites still call the idempotent effective-admission
    // helper. After map admission that must not re-import Common's peer catalog.
    a.admit_effective_definitions().unwrap();
    b.admit_effective_definitions().unwrap();

    assert_eq!(
        a.find_template("Yju10gReceivingBase")
            .unwrap()
            .info()
            .system_lifetime,
        41
    );
    assert_eq!(
        b.find_template("Yju10gReceivingBase")
            .unwrap()
            .info()
            .system_lifetime,
        41
    );
    assert_eq!(
        a.find_template("Yju10gReceivingOverlay")
            .unwrap()
            .info()
            .system_lifetime,
        17
    );
    assert_eq!(
        b.find_template("Yju10gReceivingOverlay")
            .unwrap()
            .info()
            .system_lifetime,
        29
    );
    assert!(a.find_template(COMMON_ONLY).is_none());
    assert!(b.find_template(COMMON_ONLY).is_none());

    let template_a = a.find_template("Yju10gReceivingOverlay").unwrap();
    let template_b = b.find_template("Yju10gReceivingOverlay").unwrap();
    let id_a = a.create_particle_system(&template_a, false).unwrap();
    let id_b = b.create_particle_system(&template_b, false).unwrap();
    assert_eq!(
        a.find_particle_system(id_a)
            .unwrap()
            .template()
            .info()
            .system_lifetime,
        17
    );
    assert_eq!(
        b.find_particle_system(id_b)
            .unwrap()
            .template()
            .info()
            .system_lifetime,
        29
    );

    let global = game_engine::common::ini::ini_particle_sys::get_particle_system_manager().unwrap();
    global
        .write()
        .remove_template(&AsciiString::from(COMMON_ONLY));
}
