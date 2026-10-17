//! GameEngine.cpp441: Default OCL then application OCL before match workers.
use super::*;
impl CnCGameEngine {
    pub(super) async fn load_ocl_application_sources() -> Result<Vec<String>, String> {
        let mut sources = Vec::new();
        for path in [
            "Data/INI/Default/ObjectCreationList.ini",
            "Data/INI/ObjectCreationList.ini",
        ] {
            if let Some(text) = Self::read_startup_ini_from_disk(path) {
                sources.push(text);
                continue;
            }
            if let Some(manager) = crate::assets::manager::get_asset_manager() {
                let mut manager = manager
                    .lock()
                    .map_err(|_| "OCL asset manager poisoned".to_string())?;
                match manager.extract_file(path).await {
                    Ok(bytes) => sources.push(
                        String::from_utf8(bytes).map_err(|error| format!("{path}: {error}"))?,
                    ),
                    Err(error) => log::debug!("OCL source unavailable {path}: {error}"),
                }
            }
        }
        Ok(sources)
    }
}
