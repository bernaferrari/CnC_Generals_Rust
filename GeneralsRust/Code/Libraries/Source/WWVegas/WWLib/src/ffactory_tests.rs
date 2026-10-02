use super::*;

#[test]
fn default_accessors_refer_to_the_same_cpp_factory() {
    // ffactory.cpp:15-17 publishes both interfaces of _DefaultFileFactory.
    let simple: Arc<dyn FileFactoryClass> = the_simple_file_factory();
    assert!(Arc::ptr_eq(&the_file_factory(), &simple));
}

#[test]
fn auto_ptr_preserves_the_factory_resolved_path() {
    // ffactory.cpp:26-34 retains Get_File's result without calling Set_Name.
    let factory = Arc::new(SimpleFileFactory::new());
    factory.set_sub_directory("assets\\");
    factory.set_strip_path(true);
    let file = FileAutoPtr::new(factory, "models\\tank.w3d");
    assert_eq!(file.get().file_name(), Some("assets\\tank.w3d"));
}

#[test]
fn constructors_and_configuration_are_instance_local() {
    let first = SimpleFileFactory::new();
    first.set_sub_directory("first\\");
    first.set_strip_path(true);
    let second = SimpleFileFactory::new();
    assert_eq!(
        first.get_file("models\\tank.w3d").file_name(),
        Some("first\\tank.w3d")
    );
    assert_eq!(second.get_sub_directory(), "");
    assert!(!second.get_strip_path());
    second.set_sub_directory("second\\");
    assert_eq!(
        second.get_file("models\\tank.w3d").file_name(),
        Some("second\\models\\tank.w3d")
    );
    assert_eq!(first.get_sub_directory(), "first\\");
}

#[test]
fn prepending_and_appending_keep_cpp_search_order() {
    let factory = SimpleFileFactory::new();
    factory.set_sub_directory("middle\\");
    factory.prepend_sub_directory("first");
    factory.append_sub_directory("last");
    factory.prepend_sub_directory("");
    factory.append_sub_directory("");
    assert_eq!(factory.get_sub_directory(), "first\\;middle\\;last\\");
}

#[test]
fn search_uses_first_existing_file_and_last_path_for_new_files() {
    // C++ Get_File's strtok loop stops at the first successful Open; on a
    // miss, it retains the last nonempty search directory for new files.
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let last = dir.path().join("last");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&last).unwrap();
    std::fs::write(first.join("tank.w3d"), b"first").unwrap();
    std::fs::write(last.join("tank.w3d"), b"last").unwrap();
    std::fs::write(last.join("only.w3d"), b"only").unwrap();
    let factory = SimpleFileFactory::new();
    factory.set_sub_directory(&format!(
        "{}{};;{}{};",
        first.display(),
        std::path::MAIN_SEPARATOR,
        last.display(),
        std::path::MAIN_SEPARATOR
    ));
    for (name, expected) in [
        ("tank.w3d", first.join("tank.w3d")),
        ("only.w3d", last.join("only.w3d")),
        ("new.w3d", last.join("new.w3d")),
    ] {
        let mut file = factory.get_file(name);
        assert_eq!(file.file_name(), expected.to_str());
        if name != "new.w3d" {
            assert!(file.open_read());
            let mut bytes = [0u8; 5];
            let n = file.read(&mut bytes);
            assert_eq!(
                &bytes[..n],
                if name == "tank.w3d" {
                    b"first".as_slice()
                } else {
                    b"only".as_slice()
                }
            );
        }
    }
}

#[test]
fn absolute_windows_paths_bypass_search_unless_stripped() {
    let factory = SimpleFileFactory::new();
    factory.set_sub_directory("assets\\");
    for path in ["C:\\models\\tank.w3d", "\\\\server\\models\\tank.w3d"] {
        assert_eq!(factory.get_file(path).file_name(), Some(path));
        factory.set_strip_path(true);
        assert_eq!(factory.get_file(path).file_name(), Some("assets\\tank.w3d"));
        factory.set_strip_path(false);
    }
}
