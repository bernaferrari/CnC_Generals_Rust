//! CPP GameState.cpp:800–825,848–900: portable map identities, not geometry.
//! Roots are explicit application inputs; mapping has no filesystem side effects.
use std::path::{Path, PathBuf};

fn normalized(path: &str) -> String {
    path.replace('/', "\\")
}

fn under<'a>(path: &'a str, directory: &str) -> Option<&'a str> {
    let directory = directory.trim_end_matches('\\');
    if directory.is_empty() || path.len() <= directory.len() {
        return None;
    }
    let (prefix, rest) = path.split_at_checked(directory.len())?;
    if prefix.eq_ignore_ascii_case(directory) {
        rest.strip_prefix('\\')
    } else {
        None
    }
}

fn leaf(path: &str) -> &str {
    path.rsplit('\\').next().unwrap_or(path)
}

fn leaf_and_directory(path: &str) -> &str {
    path.rsplitn(3, '\\')
        .nth(2)
        .map_or(path, |prefix| &path[prefix.len() + 1..])
}

pub(super) fn to_portable(path: &str, save_directory: &Path, user_data_directory: &str) -> String {
    let input = normalized(path);
    let save = normalized(&save_directory.to_string_lossy());
    let user_maps = normalized(
        &Path::new(user_data_directory)
            .join("Maps")
            .to_string_lossy(),
    );
    let portable = if under(&input, &save).is_some() || under(&input, "Save").is_some() {
        Some(format!("Save\\{}", leaf(&input)))
    } else if under(&input, "Maps").is_some() {
        Some(format!("Maps\\{}", leaf_and_directory(&input)))
    } else if under(&input, "UserData\\Maps").is_some()
        || (!user_data_directory.is_empty() && under(&input, &user_maps).is_some())
    {
        Some(format!("UserData\\Maps\\{}", leaf_and_directory(&input)))
    } else {
        None
    };
    // Windows' unknown-path branch lowercased the input. An arbitrary native
    // path is an admitted platform extension: retain its case and separators
    // so its genuine companion directory remains usable on Linux.
    portable.map_or_else(|| path.to_string(), |value| value.to_ascii_lowercase())
}

pub(super) fn from_portable(
    path: &str,
    save_directory: &Path,
    user_data_directory: &str,
) -> String {
    let input = normalized(path);
    if under(&input, "Save").is_some() {
        return save_directory
            .join(leaf(&input))
            .to_string_lossy()
            .into_owned();
    }
    if under(&input, "Maps").is_some() {
        // Official maps retain their virtual identity; the application's
        // mounted-content / retail resolver selects the actual file.
        return format!("Maps\\{}", leaf_and_directory(&input));
    }
    if under(&input, "UserData\\Maps").is_some() {
        let suffix = leaf_and_directory(&input).replace('\\', "/");
        // MapUtil.cpp:336-340 concatenates the configured user-data root
        // and Maps; an empty configuration supplies no synthetic directory.
        let root = PathBuf::from(user_data_directory);
        return root
            .join("Maps")
            .join(suffix)
            .to_string_lossy()
            .into_owned();
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpp_portable_names_keep_save_official_and_user_roots_distinct() {
        let save = Path::new("/Native/User/Save");
        let user = "/Native/User";
        assert_eq!(
            to_portable("/Native/User/Save/Embedded.map", save, user),
            r"save\embedded.map"
        );
        assert_eq!(
            to_portable(r"Maps\Alpine\Alpine.map", save, user),
            r"maps\alpine\alpine.map"
        );
        assert_eq!(
            to_portable("/Native/User/Maps/Custom/Custom.map", save, user),
            r"userdata\maps\custom\custom.map"
        );
        assert_eq!(
            PathBuf::from(from_portable(r"SAVE\embedded.map", save, user)),
            save.join("embedded.map")
        );
        assert_eq!(
            from_portable(r"maps\alpine\alpine.map", save, user),
            r"Maps\alpine\alpine.map"
        );
        assert_eq!(
            PathBuf::from(from_portable(
                r"UserData\Maps\custom\custom.map",
                save,
                user
            )),
            Path::new(user)
                .join("Maps")
                .join("custom")
                .join("custom.map")
        );
        assert_eq!(
            PathBuf::from(from_portable(r"UserData\Maps\custom\custom.map", save, "")),
            Path::new("Maps").join("custom").join("custom.map")
        );
    }

    #[test]
    fn arbitrary_native_pristine_paths_roundtrip_without_case_or_directory_loss() {
        let save = Path::new("/Native/User/Save");
        for input in [
            "/tmp/Authored/OwnedSavedMap.map",
            "/Native/User/SaveOther/Other.map",
            "/Native/User/MapsOther/Other.map",
        ] {
            let portable = to_portable(input, save, "/Native/User");
            assert_eq!(portable, input);
            assert_eq!(from_portable(&portable, save, "/Native/User"), input);
        }
    }
}
