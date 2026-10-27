//! One immutable current Terrain catalog; C++ TerrainTypeCollection::reset is a no-op.
pub(crate) use game_engine::common::ini::ini_terrain::TerrainTypes;
use game_engine::common::ini::ini_road::TerrainRoadCollection;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub(crate) struct TerrainRoadDefinitions {
    pub(crate) base: TerrainRoadCollection,
    pub(crate) effective: TerrainRoadCollection,
}

/// Read the ordered application Road/Bridge layers into a detached draft.
pub(crate) fn parse_application_road_sources(
    sources: &[(&str, String)],
) -> Result<TerrainRoadCollection, String> {
    let mut definitions = TerrainRoadCollection::new();
    for (path, text) in sources {
        let mut ini = game_engine::common::ini::INI::new();
        ini.set_terrain_roads_target(definitions);
        ini.with_inline_source(text, |ini| ini.parse_current_file())
            .map_err(|error| format!("{path}: {error}"))?;
        definitions = ini
            .take_terrain_roads_target()
            .ok_or("owned Road definition target missing")?;
    }
    Ok(definitions)
}

/// Same Common INI block dispatcher as application startup, with an explicit
/// catalog. No global TerrainTypes lookup/publication or filesystem side effect.
pub(crate) fn parse_application_terrain_sources(
    sources: &[(&str, String)],
) -> Result<Arc<TerrainTypes>, String> {
    let mut definitions = TerrainTypes::new();
    for (path, text) in sources {
        let mut ini = game_engine::common::ini::INI::new();
        ini.set_terrain_types_target(definitions);
        let result = ini.with_inline_source(text, |ini| ini.parse_current_file());
        definitions = ini
            .take_terrain_types_target()
            .ok_or("owned Terrain definition target missing")?;
        result.map_err(|error| format!("{path}: {error}"))?;
    }
    Ok(Arc::new(definitions))
}
impl super::GameLogic {
    /// Admit the immutable application baseline for this session.
    pub fn admit_terrain_road_definitions(
        &mut self,
        definitions: TerrainRoadCollection,
    ) -> Result<(), String> {
        if !self.objects.registered_is_empty() {
            return Err("Road definition admission requires an empty receiving world".into());
        }
        self.terrain_road_definitions = TerrainRoadDefinitions {
            base: definitions.clone(),
            effective: definitions,
        };
        Ok(())
    }

    pub(crate) fn terrain_road_baseline(&self) -> &TerrainRoadCollection {
        &self.terrain_road_definitions.base
    }

    pub(crate) fn effective_terrain_roads(&self) -> &TerrainRoadCollection {
        &self.terrain_road_definitions.effective
    }

    pub(crate) fn admit_terrain_definitions(
        &mut self,
        catalog: Arc<TerrainTypes>,
    ) -> Result<(), String> {
        if !self.objects.registered_is_empty() {
            return Err("Terrain definition admission requires an empty receiving world".into());
        }
        self.terrain_definitions = catalog;
        Ok(())
    }
    pub(crate) fn effective_terrain_definitions(&self) -> &Arc<TerrainTypes> {
        &self.terrain_definitions
    }
}

#[cfg(test)]
#[path = "terrain_definition_owner_tests.rs"]
mod tests;
