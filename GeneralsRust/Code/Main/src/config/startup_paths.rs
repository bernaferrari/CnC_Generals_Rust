//! Immutable application paths admitted before preference IO or game construction.
use super::GlobalData;
use crate::command_line::CommandLineArgs;
use anyhow::{Result, ensure};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StartupUserData {
    root: PathBuf,
}

impl StartupUserData {
    pub(crate) fn admit(texts: &[String], command_line: &CommandLineArgs) -> Result<Self> {
        let explicit = match command_line.options.get("userdata") {
            None => None,
            Some(value) => {
                let value = value
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("-userdata requires a path"))?;
                ensure!(
                    !value.trim().is_empty(),
                    "-userdata requires a nonempty path"
                );
                Some(Path::new(value))
            }
        };
        let (root, _) = GlobalData::startup_options_from_texts(texts, explicit)?;
        ensure!(
            root.is_absolute(),
            "application user data root must be absolute"
        );
        Ok(Self { root })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn options_path(&self) -> PathBuf {
        self.root.join("Options.ini")
    }
    pub(crate) fn save_directory(&self) -> PathBuf {
        self.root.join("Save")
    }

    /// Admit the root before subsystem callbacks and restore the same application
    /// path even when initialization fails. Preserve the caller's failure policy.
    pub(crate) fn initialize_with_admitted_paths(
        &self,
        initialize: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        self.publish_compatibility_paths();
        let result = initialize();
        self.publish_compatibility_paths();
        result
    }

    /// Original UserPreferences uses the application's admitted GlobalData path.
    /// These existing singleton fields are application compatibility adapters;
    /// they do not own or select mutable simulation state.
    pub(crate) fn publish_compatibility_paths(&self) {
        game_engine::common::ini::ini_game_data::ensure_global_data()
            .write()
            .set_path_user_data(self.root.to_string_lossy().into_owned());
        game_engine::common::global_data::write()
            .set_user_data_dir(self.root.to_string_lossy().into_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(value: &str) -> CommandLineArgs {
        CommandLineArgs::parse_from_args(vec!["generals".into(), format!("-userdata={value}")])
            .unwrap()
    }

    #[test]
    fn actual_cli_admission_keeps_two_preference_owners_independent() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join("Options.ini"), "ScrollFactor = 21\n").unwrap();
        std::fs::write(b.path().join("Options.ini"), "ScrollFactor = 77\n").unwrap();
        let sources = ["[Paths]\nUserDataPath = ForeignRoot\n".into()];
        let a = StartupUserData::admit(&sources, &args(a.path().to_str().unwrap())).unwrap();
        let b = StartupUserData::admit(&sources, &args(b.path().to_str().unwrap())).unwrap();
        let mut preferences_a = game_engine::common::user_preferences::UserPreferences::new();
        let mut preferences_b = game_engine::common::user_preferences::UserPreferences::new();
        assert!(preferences_a.load(a.options_path().to_str().unwrap()));
        assert!(preferences_b.load(b.options_path().to_str().unwrap()));
        assert_eq!(preferences_a.get_int_or("ScrollFactor", 0), 21);
        assert_eq!(preferences_b.get_int_or("ScrollFactor", 0), 77);
        assert_ne!(a.save_directory(), b.save_directory());
        assert_eq!(a.root().join("Options.ini"), a.options_path());
    }

    #[test]
    fn native_separate_value_cli_and_equal_cli_admit_same_root_before_io() {
        let private = tempfile::tempdir().unwrap();
        let separate = CommandLineArgs::parse_from_args(vec![
            "generals".into(),
            "-userdata".into(),
            private.path().to_str().unwrap().into(),
        ])
        .unwrap();
        let equal = args(private.path().to_str().unwrap());
        assert_eq!(
            StartupUserData::admit(&[], &separate).unwrap(),
            StartupUserData::admit(&[], &equal).unwrap()
        );
    }

    #[test]
    fn failed_subsystem_initialization_keeps_actual_common_preferences_at_admitted_root() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "failed_subsystem_initialization_keeps_actual_common_preferences_at_admitted_root",
            || {
                let private = tempfile::tempdir().unwrap();
                std::fs::write(private.path().join("Options.ini"), "ScrollFactor = 43\n").unwrap();
                let paths =
                    StartupUserData::admit(&[], &args(private.path().to_str().unwrap())).unwrap();
                let result = paths.initialize_with_admitted_paths(|| {
                    let mut preferences =
                        game_engine::common::user_preferences::UserPreferences::new();
                    assert!(preferences.load("Options.ini"));
                    assert_eq!(
                        preferences.configured_path(),
                        Some(paths.options_path().as_path())
                    );
                    assert_eq!(preferences.get_int_or("ScrollFactor", 0), 43);
                    // A failing compatibility initializer may leave its runtime view
                    // changed. The application ingress must retain its accepted owner.
                    game_engine::common::global_data::write().set_user_data_dir("foreign".into());
                    Err(anyhow::anyhow!("injected subsystem initialization failure"))
                });
                assert!(result.is_err());
                assert_eq!(
                    game_engine::common::global_data::read().get_user_data_dir(),
                    paths.root().to_str().unwrap()
                );
                let mut preferences = game_engine::common::user_preferences::UserPreferences::new();
                assert!(preferences.load("Options.ini"));
                assert_eq!(
                    preferences.configured_path(),
                    Some(paths.options_path().as_path())
                );
                assert_eq!(preferences.get_int_or("ScrollFactor", 0), 43);
            },
        );
    }

    #[test]
    fn actual_mixed_case_double_dash_and_relative_userdata_use_the_same_resolver() {
        let cwd = std::env::current_dir().unwrap();
        for option in ["--userdata=RelativeOwner", "-UsErDaTa=RelativeOwner"] {
            let args =
                CommandLineArgs::parse_from_args(vec!["generals".into(), option.into()]).unwrap();
            assert_eq!(
                StartupUserData::admit(&[], &args).unwrap().root(),
                cwd.join("RelativeOwner")
            );
        }
        let empty = CommandLineArgs::parse_from_args(vec!["generals".into(), "--userdata=".into()])
            .unwrap();
        assert!(StartupUserData::admit(&[], &empty).is_err());
    }

    #[test]
    fn missing_explicit_preference_file_does_not_select_another_root() {
        let private = tempfile::tempdir().unwrap();
        let paths = StartupUserData::admit(&[], &args(private.path().to_str().unwrap())).unwrap();
        let mut preferences = game_engine::common::user_preferences::UserPreferences::new();
        assert!(!preferences.load(paths.options_path().to_str().unwrap()));
        assert_eq!(
            preferences.configured_path(),
            Some(paths.options_path().as_path())
        );
        assert_eq!(paths.root(), private.path());
        assert!(!paths.options_path().exists());
    }

    #[test]
    fn native_cli_rejects_missing_empty_userdata_and_preserves_ini_without_override() {
        for raw in [
            vec!["generals".into(), "-userdata".into()],
            vec!["generals".into(), "-userdata=".into()],
        ] {
            match CommandLineArgs::parse_from_args(raw) {
                Ok(args) => assert!(StartupUserData::admit(&[], &args).is_err()),
                Err(_) => {}
            }
        }
        let args = CommandLineArgs::parse_from_args(vec!["generals".into()]).unwrap();
        let paths =
            StartupUserData::admit(&["[Paths]\nUserDataPath = AuthoredData\n".into()], &args)
                .unwrap();
        assert_eq!(
            paths.root(),
            std::env::current_dir().unwrap().join("AuthoredData")
        );
    }
}
