use super::{ObjectCreationList, parse_object_creation_lists_from_ini};
use std::collections::HashMap;
use std::sync::Arc;

pub type ObjectCreationListDefinitions = HashMap<String, ObjectCreationList>;

/// Immutable application definitions shared with new matches and the
/// effective set for this match after map or solo INI overrides.
#[derive(Debug, Default)]
pub struct ObjectCreationLists {
    application: Arc<ObjectCreationListDefinitions>,
    effective: ObjectCreationListDefinitions,
}

impl ObjectCreationLists {
    pub fn admit_application_sources(&mut self, sources: &[String]) -> Result<(), String> {
        let mut definitions = ObjectCreationListDefinitions::new();
        Self::apply_sources(&mut definitions, sources)?;
        self.seed_application_definitions(Arc::new(definitions));
        Ok(())
    }

    pub fn seed_application_definitions(
        &mut self,
        application: Arc<ObjectCreationListDefinitions>,
    ) {
        self.effective = (*application).clone();
        self.application = application;
    }

    pub fn application_definitions(&self) -> Arc<ObjectCreationListDefinitions> {
        Arc::clone(&self.application)
    }

    pub fn reset_to_application(&mut self) {
        self.effective.clone_from(&self.application);
    }

    pub fn apply_map_overrides(&mut self, sources: &[String]) -> Result<(), String> {
        let mut candidate = self.effective.clone();
        Self::apply_sources(&mut candidate, sources)?;
        self.effective = candidate;
        Ok(())
    }

    fn apply_sources(
        definitions: &mut ObjectCreationListDefinitions,
        sources: &[String],
    ) -> Result<(), String> {
        for source in sources {
            definitions.extend(parse_object_creation_lists_from_ini(source)?);
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&ObjectCreationList> {
        self.effective.get(name.trim())
    }

    pub fn admit_definition(&mut self, name: &str, list: ObjectCreationList) -> Result<(), String> {
        use std::collections::hash_map::Entry;

        let name = name.trim();
        if name.is_empty() {
            return Err("weapon OCL definition name is blank".into());
        }
        match self.effective.entry(name.to_owned()) {
            Entry::Occupied(_) => Err(format!("weapon OCL definition already admitted: {name}")),
            Entry::Vacant(entry) => {
                Arc::make_mut(&mut self.application).insert(name.to_owned(), list.clone());
                entry.insert(list);
                Ok(())
            }
        }
    }
}
