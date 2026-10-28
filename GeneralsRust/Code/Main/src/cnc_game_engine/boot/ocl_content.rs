//! GameEngine.cpp441: Default OCL then application OCL before match workers.
use super::*;
impl CnCGameEngine {
    pub(super) async fn load_ocl_application_sources(
        assets: &StartupAssets,
    ) -> Result<Vec<String>, String> {
        let paths = [
            "Data/INI/Default/ObjectCreationList.ini",
            "Data/INI/ObjectCreationList.ini",
        ];
        Ok(Self::load_startup_ini_texts(&paths, Some(assets))
            .await?
            .into_iter()
            .map(|(_, text)| text)
            .collect())
    }
}
