//! Immutable receiving host projections paired with the typed content owner.
//! This uses the existing Object field and inheritance kernels, without a manager
//! instance, global pending override stream, or typed ModuleData construction.
use super::{IniParser, ObjectDefinition, WW3DAssetManager};
use game_engine::common::thing::thing_template::ObjectIniFields;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
pub(crate) struct ReceivingObjectDefinitions {
    base: HashMap<String, Arc<ObjectDefinition>>,
    current: HashMap<String, Arc<ObjectDefinition>>,
    base_order: Vec<String>,
    current_order: Vec<String>,
}
impl ReceivingObjectDefinitions {
    pub(crate) fn new(snapshot: impl IntoIterator<Item = (String, ObjectDefinition)>) -> Self {
        let mut base = HashMap::new();
        let mut base_order = Vec::new();
        for (name, row) in snapshot {
            let key = name.to_owned();
            if !base.contains_key(&key) {
                base_order.push(key.clone());
            }
            base.insert(key, Arc::new(row));
        }
        Self {
            current: base.clone(),
            current_order: base_order.clone(),
            base,
            base_order,
        }
    }
    pub(crate) fn get(&self, name: &str) -> Option<&Arc<ObjectDefinition>> {
        self.current.get(name)
    }
    pub(crate) fn rows(&self) -> impl Iterator<Item = &Arc<ObjectDefinition>> {
        self.current_order
            .iter()
            .filter_map(|key| self.current.get(key))
    }
    pub(crate) fn reset_map_overrides(&mut self) {
        self.current = self.base.clone();
        self.current_order = self.base_order.clone();
    }

    /// Ordered boot declarations share the actual incoming sources with the
    /// strict typed producer; no global resolved snapshot is recaptured.
    pub(crate) fn from_boot_sources(sources: &[(String, String)]) -> Result<Self, String> {
        let mut owner = Self::new(std::iter::empty());
        for (path, text) in sources {
            owner.apply_source(path, text, false)?;
        }
        owner.base = owner.current.clone();
        owner.base_order = owner.current_order.clone();
        Ok(owner)
    }

    /// The caller validates these same sources through the strict typed producer
    /// first. This second projection creates no typed data or runtime keys.
    pub(crate) fn map_override_draft(&self, sources: &[String]) -> Result<Self, String> {
        let mut draft = self.clone();
        draft.reset_map_overrides();
        for (index, source) in sources.iter().enumerate() {
            draft.apply_source(&format!("<receiving companion {index}>"), source, true)?;
        }
        Ok(draft)
    }
    fn apply_source(
        &mut self,
        path: &str,
        source: &str,
        create_overrides: bool,
    ) -> Result<(), String> {
        let lines = source.lines().collect::<Vec<_>>();
        let mut index = 0;
        while index < lines.len() {
            let header = lines[index].split(';').next().unwrap_or("").trim();
            let tokens = header.split_whitespace().collect::<Vec<_>>();
            let Some(kind) = tokens.first() else {
                index += 1;
                continue;
            };
            if !["Object", "ObjectReskin", "ChildObject"]
                .iter()
                .any(|word| kind.eq_ignore_ascii_case(word))
                || tokens.get(1) == Some(&"=")
            {
                index += 1;
                continue;
            }
            let name = tokens.get(1).ok_or("Missing receiving Object name")?;
            let parent = tokens.get(2).copied();
            let (properties, next) = ObjectIniFields::capture_body(&lines, index + 1);
            if next <= index || next > lines.len() {
                return Err("Invalid receiving Object extent".into());
            }
            let key = (*name).to_owned();
            let definition = if kind.eq_ignore_ascii_case("Object") {
                if let Some(previous) = self.get(name) {
                    if create_overrides {
                        let mut next = (**previous).clone();
                        next.apply_create_override_properties(&properties);
                        next
                    } else {
                        let child = Self::parse_current_declaration(
                            &lines[index..next],
                            name,
                            path,
                            false,
                        )?;
                        WW3DAssetManager::merge_definition_inheritance((**previous).clone(), child)
                    }
                } else {
                    let child = Self::parse_current_declaration(
                        &lines[index..next],
                        name,
                        path,
                        create_overrides,
                    )?;
                    if let Some(default) = self.get("DefaultThingTemplate") {
                        WW3DAssetManager::merge_default_thing_template((**default).clone(), child)
                    } else {
                        child
                    }
                }
            } else {
                let parent = parent.ok_or("Receiving inherited Object has no parent")?;
                let parent = self
                    .get(parent)
                    .ok_or_else(|| format!("Missing receiving parent '{parent}'"))?;
                let child = Self::parse_current_declaration(
                    &lines[index..next],
                    name,
                    path,
                    create_overrides,
                )?;
                WW3DAssetManager::merge_definition_inheritance((**parent).clone(), child)
            };
            if !self.current.contains_key(&key) {
                self.current_order.push(key.clone());
            }
            self.current.insert(key, Arc::new(definition));
            index = next;
        }
        Ok(())
    }
    fn parse_current_declaration(
        lines: &[&str],
        name: &str,
        path: &str,
        create_overrides: bool,
    ) -> Result<ObjectDefinition, String> {
        let mut parser = IniParser::new();
        parser
            .parse_ini_content_with_overrides(&lines.join("\n"), path, create_overrides)
            .map_err(|error| error.to_string())?;
        parser
            .get_definition(name)
            .cloned()
            .ok_or_else(|| format!("Missing parsed receiving Object '{name}'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> ReceivingObjectDefinitions {
        let mut parser = IniParser::new();
        parser
            .parse_ini_content("Object Parent\nScale = 2\nEnd", "owned-base.ini")
            .unwrap();
        ReceivingObjectDefinitions::new(
            parser
                .get_all_definitions()
                .iter()
                .map(|(name, row)| (name.clone(), row.clone())),
        )
    }
    #[test]
    fn receiving_map_projection_preserves_duplicate_source_order_inheritance_and_peer() {
        let owner = base();
        let prior = Arc::clone(owner.get("Parent").unwrap());
        let map = "Object Parent\nScale = 3\nEnd\nObject Parent\nScale = 4\nEnd\nChildObject MapOnly Parent\nEnd".to_owned();
        let mut next = owner.map_override_draft(&[map]).unwrap();
        assert_eq!(next.get("Parent").unwrap().scale, 4.0);
        assert_eq!(next.get("MapOnly").unwrap().scale, 4.0);
        assert_eq!(owner.get("Parent").unwrap().scale, 2.0);
        assert!(Arc::ptr_eq(owner.get("Parent").unwrap(), &prior));
        next.reset_map_overrides();
        assert!(Arc::ptr_eq(next.get("Parent").unwrap(), &prior));
        assert!(next.get("MapOnly").is_none());
    }
    #[test]
    fn rejected_receiving_projection_tail_preserves_accepted_base_and_map() {
        let base = base();
        let owner = base
            .map_override_draft(&["Object Parent\nScale = 7\nEnd".into()])
            .unwrap();
        let before = Arc::clone(owner.get("Parent").unwrap());
        let error = owner
            .map_override_draft(&[
                "Object Parent\nScale = 9\nEnd\nChildObject Broken MissingParent\nEnd".into(),
            ])
            .err()
            .unwrap();
        assert!(error.contains("Missing receiving parent"), "{error}");
        assert!(Arc::ptr_eq(owner.get("Parent").unwrap(), &before));
        assert_eq!(owner.get("Parent").unwrap().scale, 7.0);
        assert!(owner.get("Broken").is_none());
        assert_eq!(
            owner
                .map_override_draft(&[])
                .unwrap()
                .get("Parent")
                .unwrap()
                .scale,
            2.0
        );
    }
    #[test]
    fn receiving_definition_and_map_override_keep_case_distinct_source_identities() {
        use game_engine::common::{
            ini::INILoadType,
            name_key_generator::NameKeyNamespace,
            thing::{
                module_factory::ModuleFactory, receiving_module_content::ReceivingModuleContent,
            },
        };
        let source = "Object Foo\nScale = 2\nEnd\nObject foo\nScale = 3\nEnd";
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(source, "exact-case-base.ini")
            .unwrap();
        let owner = ReceivingObjectDefinitions::new(
            parser
                .get_all_definitions()
                .iter()
                .map(|(name, row)| (name.clone(), row.clone())),
        );
        let mut typed =
            ReceivingModuleContent::new(ModuleFactory::new_receiving(NameKeyNamespace::new()));
        assert_eq!(
            typed.load_ini_text(source, INILoadType::Overwrite).unwrap(),
            2
        );
        let upper = typed.admitted_template("Foo").unwrap();
        let lower = typed.admitted_template("foo").unwrap();
        assert!(!Arc::ptr_eq(&upper, &lower));
        assert_eq!(owner.get("Foo").unwrap().scale, 2.0);
        assert_eq!(owner.get("foo").unwrap().scale, 3.0);
        assert!(owner.get("FOO").is_none());
        assert!(typed.admitted_template("FOO").is_none());
        let override_source = "Object foo\nScale = 7\nEnd".to_owned();
        let next = owner
            .map_override_draft(&[override_source.clone()])
            .unwrap();
        typed
            .load_ini_text(&override_source, INILoadType::CreateOverrides)
            .unwrap();
        assert_eq!(next.get("Foo").unwrap().scale, 2.0);
        assert_eq!(next.get("foo").unwrap().scale, 7.0);
        assert!(Arc::ptr_eq(
            &upper,
            &typed.admitted_template("Foo").unwrap()
        ));
        assert!(!Arc::ptr_eq(
            &lower,
            &typed.admitted_template("foo").unwrap()
        ));
        assert_eq!(owner.get("foo").unwrap().scale, 3.0);
    }
}
