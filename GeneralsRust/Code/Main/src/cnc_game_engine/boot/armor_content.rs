//! GameEngine.cpp445 loads Armor.ini once. An unused compatibility candidate
//! must never be read/decoded after the authoritative source was selected.
use super::*;
use std::future::Future;

const ARMOR_PATHS: [&str; 2] = ["Data/INI/Armor.ini", "Data/INI/Default/Armor.ini"];

/// A domain-local selection kernel; IO remains at application admission.
/// Only absence advances to the compatibility candidate. The original INI
/// dispatcher accepts an empty file; malformed present content still rejects.
async fn select_armor_source<F, Fut>(
    mut read: F,
) -> Result<Arc<gamelogic::object::armor::ArmorDefinitions>, String>
where
    F: FnMut(&'static str) -> Fut,
    Fut: Future<Output = Result<Option<String>, String>>,
{
    for path in ARMOR_PATHS {
        if let Some(text) = read(path).await? {
            let catalog = match gamelogic::object::armor::ArmorDefinitions::load_from_str(&text) {
                Ok(catalog) => catalog,
                // INI.cpp327-389 has no minimum block count. A present empty
                // source is authoritative, never a reason to open another file.
                Err(gamelogic::object::armor::ArmorLoadError::Empty) => {
                    gamelogic::object::armor::ArmorDefinitions::new()
                }
                Err(error) => return Err(format!("{path}: {error}")),
            };
            return Ok(Arc::new(catalog));
        }
    }
    Ok(Arc::new(gamelogic::object::armor::ArmorDefinitions::new()))
}
impl CnCGameEngine {
    pub(super) async fn load_application_armor_definitions()
    -> Result<Arc<gamelogic::object::armor::ArmorDefinitions>, String> {
        select_armor_source(|path| async move {
            // The existing loose/archive resolver is unchanged. A singleton
            // request ensures its eager collection cannot touch the next path.
            let texts = Self::load_startup_ini_texts(&[path]).await?;
            Ok(texts.into_iter().next().map(|(_, text)| text))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn armor_startup_selects_primary_without_decoding_unused_invalid_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let primary = directory.path().join(ARMOR_PATHS[0]);
        let fallback = directory.path().join(ARMOR_PATHS[1]);
        std::fs::create_dir_all(primary.parent().unwrap()).unwrap();
        std::fs::create_dir_all(fallback.parent().unwrap()).unwrap();
        std::fs::write(&primary, "Armor Selected\n Armor = DEFAULT 25%\nEnd\n").unwrap();
        std::fs::write(&fallback, [0xff, 0xfe, 0xfd]).unwrap();
        fn load(
            directory: &std::path::Path,
            requested: &mut Vec<&'static str>,
        ) -> Result<Arc<gamelogic::object::armor::ArmorDefinitions>, String> {
            requested.clear();
            futures::executor::block_on(select_armor_source(|path| {
                requested.push(path);
                let result = match std::fs::read(directory.join(path)) {
                    Ok(bytes) => String::from_utf8(bytes)
                        .map(Some)
                        .map_err(|error| format!("{path}: {error}")),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(error) => Err(format!("{path}: {error}")),
                };
                std::future::ready(result)
            }))
        }
        let mut requested = Vec::new();
        let selected = load(directory.path(), &mut requested).unwrap();
        assert_eq!(
            selected
                .find_template("Selected")
                .unwrap()
                .adjust_damage(gamelogic::damage::DamageType::Explosion, 100.0),
            25.0
        );
        assert_eq!(
            requested,
            vec![ARMOR_PATHS[0]],
            "an unused invalid UTF8 file is not requested"
        );
        assert!(String::from_utf8(std::fs::read(&fallback).unwrap()).is_err());

        // Missing primary retains the explicitly declared compatibility path.
        std::fs::remove_file(&primary).unwrap();
        std::fs::write(&fallback, "Armor Selected\n Armor = DEFAULT 50%\nEnd\n").unwrap();
        let selected = load(directory.path(), &mut requested).unwrap();
        assert_eq!(
            selected
                .find_template("Selected")
                .unwrap()
                .adjust_damage(gamelogic::damage::DamageType::Explosion, 100.0),
            50.0
        );
        assert_eq!(requested, vec![ARMOR_PATHS[0], ARMOR_PATHS[1]]);

        // Empty is a valid authoritative source; malformed is an error.
        // Neither case opens the compatibility file.
        std::fs::write(&primary, "").unwrap();
        assert!(load(directory.path(), &mut requested).unwrap().is_empty());
        assert_eq!(requested, vec![ARMOR_PATHS[0]]);
        std::fs::write(
            &primary,
            "Armor Selected\n Armor = NOT_A_DAMAGE_TYPE 25%\nEnd\n",
        )
        .unwrap();
        assert!(load(directory.path(), &mut requested).is_err());
        assert_eq!(requested, vec![ARMOR_PATHS[0]]);
    }
}
