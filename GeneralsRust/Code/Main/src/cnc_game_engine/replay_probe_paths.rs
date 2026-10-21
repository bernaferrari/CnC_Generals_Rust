//! Fail-closed writable-path admission for the native developer probe only.
//! No Engine, Recorder, Tokio runtime or published asset manager is constructed here.
use super::CnCGameEngine;
use anyhow::{Context, ensure};
use std::path::{Component, Path};

fn private_destination(root: &Path, candidate: &Path) -> anyhow::Result<()> {
    ensure!(
        candidate.is_absolute(),
        "probe destination must be absolute: {}",
        candidate.display()
    );
    ensure!(
        !candidate
            .components()
            .any(|part| matches!(part, Component::ParentDir)),
        "probe destination contains parent traversal: {}",
        candidate.display()
    );
    ensure!(
        candidate.starts_with(root),
        "probe refuses external writable destination: {}",
        candidate.display()
    );
    // Reject every preexisting symlink component, including a dangling leaf.
    let mut prefix = std::path::PathBuf::new();
    for part in candidate.components() {
        prefix.push(part.as_os_str());
        if let Ok(metadata) = std::fs::symlink_metadata(&prefix) {
            ensure!(
                !metadata.file_type().is_symlink(),
                "probe writable path contains symlink: {}",
                prefix.display()
            );
        }
    }
    Ok(())
}

fn admitted_options(
    root: &Path,
    texts: &[String],
    explicit: Option<&Path>,
) -> anyhow::Result<std::path::PathBuf> {
    if let Some(explicit) = explicit {
        let (directory, _) =
            crate::config::GlobalData::startup_options_from_texts(texts, Some(explicit))?;
        private_destination(root, &directory.join("Options.ini"))?;
        return Ok(directory);
    }
    for count in 1..=texts.len() {
        let (directory, _) =
            crate::config::GlobalData::camera_startup_options_from_texts(&texts[..count])?;
        private_destination(root, &directory.join("Options.ini"))?;
    }
    let (directory, _) = crate::config::GlobalData::camera_startup_options_from_texts(texts)?;
    private_destination(root, &directory.join("Options.ini"))?;
    Ok(directory)
}

pub fn prepare_replay_probe_private_paths(root: &Path, source: &Path) -> anyhow::Result<()> {
    ensure!(
        root.is_absolute() && root.canonicalize()? == root,
        "private root must be canonical"
    );
    ensure!(
        std::env::current_dir()? == root.join("cwd"),
        "probe requires private cwd before startup"
    );
    let user_data = root.join("userdata");
    private_destination(root, &user_data)?;
    let mut archive = crate::assets::archive::ArchiveFileSystem::new();
    pollster::block_on(archive.init()).context("read-only probe asset admission")?;
    let paths = [
        "Data/INI/Default/GameData.ini",
        "Data/INI/GameData.ini",
        "Data/INI/GameDataDebug.ini",
    ];
    let mut sources = Vec::new();
    for path in paths {
        // Preserve the Engine's original loose-file precedence, then licensed archives.
        let disk = CnCGameEngine::startup_ini_disk_roots()
            .into_iter()
            .find_map(|relative| {
                CnCGameEngine::read_startup_ini_candidate(
                    &source.join(relative).join(path),
                    relative != ".",
                )
            });
        let text = match disk {
            Some(text) => Some(text),
            None => match archive.open_file_sync(path) {
                Ok(bytes) => Some(String::from_utf8(bytes).context(path)?),
                Err(_) => None,
            },
        };
        if let Some(text) = text {
            sources.push((path, text));
        }
    }
    // The actual CLI root is admitted after ordered INI overlays, before all
    // preference IO. Intermediate authored roots never select a file.
    let texts = sources
        .iter()
        .map(|(_, text)| text.clone())
        .collect::<Vec<_>>();
    let options = admitted_options(root, &texts, Some(&user_data))?;
    // Freeze the exact inspected bytes ahead of the Engine's disk-first reads.
    // No path/config rewriting changes the native camera scenario.
    for (path, text) in sources {
        let destination = root.join("cwd").join(path);
        std::fs::create_dir_all(destination.parent().unwrap())?;
        std::fs::write(destination, text)?;
    }
    {
        let data = game_engine::common::ini::ini_game_data::ensure_global_data();
        let mut data = data.write();
        data.set_path_user_data(user_data.to_string_lossy().into_owned());
        data.save_stats = false;
        data.base_stats_dir = user_data.join("Stats").to_string_lossy().into_owned();
    }
    game_engine::common::global_data::write()
        .set_user_data_dir(user_data.to_string_lossy().into_owned());
    // Common GameData currently has no parsed setters for these three path/stat
    // fields. Explicit installation prevents its executable/Replays fallback.
    let outputs = [
        options.join("Options.ini"),
        user_data.join("Options.ini"),
        user_data.join("Save"),
        user_data.join("Replays"),
        user_data.join("Stats"),
    ];
    for path in &outputs {
        private_destination(root, path)?;
    }
    let receipt = outputs
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(root.join("resolved-writable-paths.txt"), receipt)?;
    eprintln!("probe Main camera Options output: {}", outputs[0].display());
    eprintln!("probe Common/LOD Options output: {}", outputs[1].display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writable_paths_reject_external_and_parent_traversal() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join("probe-admission-unit");
        assert!(private_destination(&root, &root.join("userdata/Options.ini")).is_ok());
        assert!(private_destination(&root, &root.join("../Options.ini")).is_err());
        assert!(private_destination(&root, &root.with_file_name("foreign/Options.ini")).is_err());
        assert!(private_destination(&root, Path::new("Options.ini")).is_err());
    }
    #[test]
    fn authored_external_options_is_rejected_even_when_later_override_is_private() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let safe = format!(
            "[Paths]\nUserDataPath = {}\n",
            root.join("userdata").display()
        );
        assert_eq!(
            admitted_options(&root, &[safe.clone()], None).unwrap(),
            root.join("userdata")
        );
        let external = format!(
            "[Paths]\nUserDataPath = {}\n",
            root.parent().unwrap().join("foreign").display()
        );
        assert!(admitted_options(&root, &[external, safe], None).is_err());
    }
    #[test]
    fn authored_relative_options_resolves_in_current_private_directory() {
        // The production decoder resolves relative Paths against the driving
        // process CWD before private admission. No process CWD/env mutation.
        let cwd = std::env::current_dir().unwrap();
        let source = "[Paths]\nUserDataPath = UserData\n".to_owned();
        assert_eq!(
            admitted_options(&cwd, &[source], None).unwrap(),
            cwd.join("UserData")
        );
        let traversal = "[Paths]\nUserDataPath = ../Foreign\n".to_owned();
        assert!(admitted_options(&cwd, &[traversal], None).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn writable_paths_reject_symlink_escape_before_any_write() {
        let root = tempfile::tempdir().unwrap();
        let foreign = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(foreign.path(), root.path().join("userdata")).unwrap();
        assert!(
            private_destination(root.path(), &root.path().join("userdata/Options.ini")).is_err()
        );
        assert!(!foreign.path().join("Options.ini").exists());
    }
}
