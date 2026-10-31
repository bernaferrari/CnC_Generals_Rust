use generals_main::command_line::CommandLineArgs;
use generals_main::single_instance::create_startup_instance_guard;
use std::path::Path;

fn private_host_args(root: &Path) -> CommandLineArgs {
    CommandLineArgs::parse_from_args(vec![
        "generals".into(),
        "-runtime_host=headless".into(),
        "-runtime_host_private_profile=1".into(),
        format!("-userdata={}", root.display()),
    ])
    .unwrap()
}

#[test]
fn private_profiles_are_independent_but_the_same_profile_is_exclusive() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let a = create_startup_instance_guard(&private_host_args(first.path())).unwrap();
    let _b = create_startup_instance_guard(&private_host_args(second.path())).unwrap();
    assert!(create_startup_instance_guard(&private_host_args(&first.path().join("."))).is_err());
    drop(a);
    assert!(create_startup_instance_guard(&private_host_args(first.path())).is_ok());
}

#[test]
fn private_mode_requires_a_runtime_host_and_a_temporary_profile() {
    let root = tempfile::tempdir().unwrap();
    let no_host = CommandLineArgs::parse_from_args(vec![
        "generals".into(),
        "-runtime_host_private_profile=1".into(),
        format!("-userdata={}", root.path().display()),
    ])
    .unwrap();
    assert!(create_startup_instance_guard(&no_host).is_err());
    let no_data = CommandLineArgs::parse_from_args(vec![
        "generals".into(),
        "-runtime_host=headless".into(),
        "-runtime_host_private_profile=1".into(),
    ])
    .unwrap();
    assert!(create_startup_instance_guard(&no_data).is_err());
    let disabled = CommandLineArgs::parse_from_args(vec![
        "generals".into(),
        "-runtime_host=headless".into(),
        "-runtime_host_private_profile=0".into(),
        format!("-userdata={}", root.path().display()),
    ])
    .unwrap();
    assert!(create_startup_instance_guard(&disabled).is_err());
    assert!(create_startup_instance_guard(&private_host_args(Path::new("/"))).is_err());
}

#[cfg(unix)]
#[test]
fn private_profile_cannot_escape_the_temp_root_through_a_symlink() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let link = dir.path().join("outside");
    symlink("/", &link).unwrap();
    assert!(create_startup_instance_guard(&private_host_args(&link)).is_err());
}
