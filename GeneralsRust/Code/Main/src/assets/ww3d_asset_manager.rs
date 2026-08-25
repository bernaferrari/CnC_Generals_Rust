////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// WW3D Asset Manager - Port of C++ WW3DAssetManager
// Handles texture and model loading from INI-defined object templates
// Reference: /GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/assetmgr.cpp

use crate::assets::archive::ArchiveFileSystem;
use crate::assets::ini_parser::{
    AuthoredConditionModelSelection, AuthoredDrawModel, IniParser, ObjectDefinition,
};
use anyhow::Result;
use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};

/// WW3D Asset Manager - Manages object definitions and their associated assets
/// Equivalent to C++ WW3DAssetManager::Get_Texture() and object template system
pub struct WW3DAssetManager {
    /// Object definitions loaded from INI files
    object_definitions: HashMap<String, ObjectDefinition>,
    /// Lowercase -> canonical name lookup for case-insensitive matches
    normalized_name_lookup: HashMap<String, String>,

    /// INI parser instance
    ini_parser: IniParser,

    /// Texture cache: object_name -> texture_filename
    texture_cache: HashMap<String, String>,

    /// Model cache: object_name -> model_filename
    model_cache: HashMap<String, String>,

    /// Whether INI files have been loaded
    initialized: bool,
}

impl WW3DAssetManager {
    /// Create a new WW3D Asset Manager
    pub fn new() -> Self {
        Self {
            object_definitions: HashMap::new(),
            normalized_name_lookup: HashMap::new(),
            ini_parser: IniParser::new(),
            texture_cache: HashMap::new(),
            model_cache: HashMap::new(),
            initialized: false,
        }
    }

    /// Initialize by loading all object INI files from INIZH.big
    /// Matches C++ behavior of loading object templates at startup
    pub async fn initialize(&mut self, archive_system: &mut ArchiveFileSystem) -> Result<()> {
        if self.initialized {
            warn!("WW3DAssetManager already initialized");
            return Ok(());
        }

        info!("Initializing WW3DAssetManager - Loading INI object definitions from INIZH.big");

        // Discover available object INIs directly from mounted archives.
        // This avoids stale hardcoded names and keeps parity with mods/expansions.
        let object_ini_files = Self::discover_object_ini_files(archive_system);

        let mut total_objects_loaded = 0;
        let mut files_processed = 0;
        let total_files = object_ini_files.len();

        for (idx, ini_file) in object_ini_files.iter().enumerate() {
            debug!(
                "📄 Loading INI file {}/{}: {}",
                idx + 1,
                total_files,
                ini_file
            );

            match archive_system.open_file(ini_file).await {
                Ok(data) => {
                    // Try to parse as UTF-8
                    match String::from_utf8(data) {
                        Ok(content) => {
                            // Parse the INI content
                            match self.ini_parser.parse_ini_content(&content, ini_file) {
                                Ok(count) => {
                                    debug!("✅ Loaded {} objects from {}", count, ini_file);
                                    total_objects_loaded += count;
                                    files_processed += 1;
                                }
                                Err(e) => {
                                    warn!("⚠️ Failed to parse {}: {}", ini_file, e);
                                }
                            }
                        }
                        Err(_) => {
                            warn!("⚠️ Failed to decode {} as UTF-8", ini_file);
                        }
                    }
                }
                Err(e) => {
                    // File not found or other error - continue with next
                    debug!("File not found or not accessible: {}: {}", ini_file, e);
                }
            }
        }

        // Copy parsed definitions to internal cache
        let raw_definitions: HashMap<String, ObjectDefinition> = self
            .ini_parser
            .get_all_definitions()
            .iter()
            .map(|(name, def)| (name.clone(), def.clone()))
            .collect();
        let mut resolved_definitions = HashMap::with_capacity(raw_definitions.len());
        for name in raw_definitions.keys() {
            let mut stack = HashSet::new();
            if let Some(definition) = Self::resolve_inherited_definition(
                name,
                &raw_definitions,
                &mut resolved_definitions,
                &mut stack,
            ) {
                resolved_definitions.insert(name.clone(), definition);
            }
        }

        let mut definitions: Vec<(String, ObjectDefinition)> =
            resolved_definitions.into_iter().collect();
        definitions.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, def) in definitions {
            self.register_definition_indices(&name, &def);
            self.object_definitions.insert(name.clone(), def.clone());

            // Build texture and model caches for quick lookup
            if let Some(model) = &def.model_name {
                self.model_cache.insert(name.clone(), model.clone());
            }

            if let Some(texture) = def.get_primary_texture() {
                self.texture_cache.insert(name.clone(), texture.to_string());
            }
        }

        info!(
            "✅ WW3DAssetManager initialized: Loaded {} objects from {} INI files",
            total_objects_loaded, files_processed
        );

        self.initialized = true;
        Ok(())
    }

    fn resolve_inherited_definition(
        name: &str,
        raw_definitions: &HashMap<String, ObjectDefinition>,
        resolved_definitions: &mut HashMap<String, ObjectDefinition>,
        stack: &mut HashSet<String>,
    ) -> Option<ObjectDefinition> {
        if let Some(existing) = resolved_definitions.get(name) {
            return Some(existing.clone());
        }

        let raw = raw_definitions.get(name)?.clone();
        if !stack.insert(name.to_string()) {
            warn!(
                "Detected cyclic object inheritance while resolving '{}'",
                name
            );
            return Some(raw);
        }

        let resolved = if let Some(parent_name) = raw.parent_name.as_deref() {
            if let Some(parent) = Self::resolve_inherited_definition(
                parent_name,
                raw_definitions,
                resolved_definitions,
                stack,
            ) {
                Self::merge_definition_inheritance(parent, raw)
            } else {
                raw
            }
        } else if !Self::is_default_thing_template_name(name) {
            // C++ ThingFactory::newTemplate copies DefaultThingTemplate
            // into every new object before parse. ChildObject/ObjectReskin
            // already inherit that copy through their named parent.
            if let Some(default_name) = raw_definitions
                .keys()
                .find(|key| Self::is_default_thing_template_name(key))
                .cloned()
            {
                if let Some(default) = Self::resolve_inherited_definition(
                    &default_name,
                    raw_definitions,
                    resolved_definitions,
                    stack,
                ) {
                    Self::merge_default_thing_template(default, raw)
                } else {
                    raw
                }
            } else {
                raw
            }
        } else {
            raw
        };

        stack.remove(name);
        Some(resolved)
    }

    fn merge_definition_inheritance(
        mut parent: ObjectDefinition,
        child: ObjectDefinition,
    ) -> ObjectDefinition {
        parent.name = child.name.clone();
        parent.parent_name = child.parent_name.clone();

        if !child.object_type.is_empty() {
            parent.object_type = child.object_type;
        }
        if !child.display_name.is_empty() {
            parent.display_name = child.display_name;
        }
        if child.model_name.is_some() {
            parent.model_name = child.model_name;
        }
        if child.draw_module.is_some() {
            parent.draw_module = child.draw_module;
        }
        // C++ ObjectReskin/ChildObject draw data replaces the inherited module
        // only when the child actually declares Draw data.  Do not merge
        // unrelated state lists: that would fabricate a hybrid model table
        // which never existed in the Object INIs.
        if !child.draw_modules.is_empty() {
            parent.draw_modules = child.draw_modules;
        }
        // Behavior module tags are the C++ INI override identity.  A child
        // with the same tag replaces that one parent module in place; a new
        // tag appends in source order.  Merging only raw field names would
        // cross-contaminate unrelated modules such as DockUpdate and Contain.
        for child_module in child.behavior_modules {
            if let Some(child_tag) = child_module.module_tag.as_deref() {
                if let Some(index) = parent.behavior_modules.iter().position(|parent_module| {
                    parent_module
                        .module_tag
                        .as_deref()
                        .is_some_and(|parent_tag| parent_tag.eq_ignore_ascii_case(child_tag))
                }) {
                    parent.behavior_modules[index] = child_module;
                    continue;
                }
            }
            parent.behavior_modules.push(child_module);
        }
        if child.armor_type.is_some() {
            parent.armor_type = child.armor_type;
        }
        if child.hit_points.is_some() {
            parent.hit_points = child.hit_points;
        }
        // An explicit child `Scale = 1.0` must reset a parent's non-default
        // asset scale.  Comparing only the numeric value loses C++ INI
        // provenance for ChildObject/ObjectReskin inheritance.
        if child.scale_was_specified {
            parent.scale = child.scale;
            parent.scale_was_specified = true;
        }
        if child.owner.is_some() {
            parent.owner = child.owner;
        }
        if child.primary_weapon.is_some() {
            parent.primary_weapon = child.primary_weapon;
        }
        if child.secondary_weapon.is_some() {
            parent.secondary_weapon = child.secondary_weapon;
        }
        if child.tertiary_weapon.is_some() {
            parent.tertiary_weapon = child.tertiary_weapon;
        }

        // C++ ThingTemplate::parseWeaponTemplateSet clears every inherited
        // WeaponSet on the first child-authored WeaponSet
        // (`m_weaponsCopiedFromDefault`).  Retain the child's complete source
        // order rather than hybrid-merging condition rows: an omitted mine
        // row in a ChildObject must not leak through from its parent.
        if !child.weapon_sets.is_empty() {
            parent.weapon_sets = child.weapon_sets;
        }
        if !child.armor_sets.is_empty() {
            parent.armor_sets = child.armor_sets;
        }
        if child.subdual_damage_cap.is_some() {
            parent.subdual_damage_cap = child.subdual_damage_cap;
        }
        if child.subdual_heal_rate_frames.is_some() {
            parent.subdual_heal_rate_frames = child.subdual_heal_rate_frames;
        }
        if child.subdual_heal_amount.is_some() {
            parent.subdual_heal_amount = child.subdual_heal_amount;
        }

        // A ChildObject/ObjectReskin overrides the parent set it names, but
        // every row authored by the child stays in source order.  In
        // particular, a custom child with duplicate SET_NORMAL declarations
        // remains visibly ambiguous to RiderChangeContain rather than being
        // collapsed to the final declaration by the old compatibility map.
        if !child.locomotor_sets.is_empty() {
            parent.locomotor_sets.retain(|parent_set| {
                !child.locomotor_sets.iter().any(|child_set| {
                    child_set
                        .set_name
                        .eq_ignore_ascii_case(&parent_set.set_name)
                })
            });
            parent.locomotor_sets.extend(child.locomotor_sets);
        }

        for (slot, texture) in child.textures {
            parent.textures.insert(slot, texture);
        }
        for (key, value) in child.attributes {
            parent.attributes.insert(key, value);
        }

        parent
    }

    fn is_default_thing_template_name(name: &str) -> bool {
        name.eq_ignore_ascii_case("DefaultThingTemplate")
    }

    fn is_default_object_ini(path: &str) -> bool {
        path.replace('\\', "/")
            .eq_ignore_ascii_case("Data/INI/Default/Object.ini")
    }

    /// Copy DefaultThingTemplate scalars/collections the way C++
    /// `ThingFactory::newTemplate` does (`*newTemplate = *defaultT`).
    ///
    /// This is not ChildObject module merge: default DestroyDie / InactiveBody
    /// / W3DDefaultDraw must not append onto every authored object. Inheritable
    /// AutoHeal and Overrideable StealthUpdate stay on leftover ThingFactory.
    fn merge_default_thing_template(
        default: ObjectDefinition,
        mut child: ObjectDefinition,
    ) -> ObjectDefinition {
        child.attributes = Self::overlay_ini_attributes(default.attributes, child.attributes);

        if child.weapon_sets.is_empty() {
            child.weapon_sets = default.weapon_sets;
            if child.primary_weapon.is_none() {
                child.primary_weapon = default.primary_weapon;
            }
            if child.secondary_weapon.is_none() {
                child.secondary_weapon = default.secondary_weapon;
            }
            if child.tertiary_weapon.is_none() {
                child.tertiary_weapon = default.tertiary_weapon;
            }
        }
        if child.armor_sets.is_empty() {
            child.armor_sets = default.armor_sets;
            if child.armor_type.is_none() {
                child.armor_type = default.armor_type;
            }
        }
        if child.hit_points.is_none() {
            child.hit_points = default.hit_points;
        }
        if !child.scale_was_specified {
            child.scale = default.scale;
            child.scale_was_specified = default.scale_was_specified;
        }
        if child.owner.is_none() {
            child.owner = default.owner;
        }
        if child.subdual_damage_cap.is_none() {
            child.subdual_damage_cap = default.subdual_damage_cap;
        }
        if child.subdual_heal_rate_frames.is_none() {
            child.subdual_heal_rate_frames = default.subdual_heal_rate_frames;
        }
        if child.subdual_heal_amount.is_none() {
            child.subdual_heal_amount = default.subdual_heal_amount;
        }
        if child.locomotor_sets.is_empty() {
            child.locomotor_sets = default.locomotor_sets;
        }
        child
    }

    fn overlay_ini_attributes(
        mut base: HashMap<String, String>,
        overlay: HashMap<String, String>,
    ) -> HashMap<String, String> {
        for (key, value) in overlay {
            if let Some(existing) = base
                .keys()
                .find(|existing| existing.eq_ignore_ascii_case(&key))
                .cloned()
            {
                base.remove(&existing);
            }
            base.insert(key, value);
        }
        base
    }

    fn discover_object_ini_files(archive_system: &ArchiveFileSystem) -> Vec<String> {
        Self::select_catalogue_object_ini_files(archive_system.list_all_files())
    }

    /// C++ `GameEngine.cpp:458` loads `Data\\INI\\Default\\Object.ini` first,
    /// then `Data\\INI\\Object`. Live catalogue also keeps `crate.ini`.
    fn select_catalogue_object_ini_files<I, S>(paths: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut default_object_ini = None;
        let mut discovered: Vec<String> = paths
            .into_iter()
            .map(|path| path.as_ref().replace('\\', "/"))
            .filter(|path| {
                if Self::is_default_object_ini(path) {
                    if default_object_ini.is_none() {
                        default_object_ini = Some(path.clone());
                    }
                    return false;
                }
                let normalized = path.to_ascii_lowercase();
                (normalized.starts_with("data/ini/object/") && normalized.ends_with(".ini"))
                    || normalized == "data/ini/crate.ini"
            })
            .collect();

        discovered.sort_by_key(|path| path.to_ascii_lowercase());
        discovered.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

        discovered.insert(
            0,
            default_object_ini.unwrap_or_else(|| "Data/INI/Default/Object.ini".to_string()),
        );
        discovered
    }

    /// Get the texture filename for an object
    /// Matches C++ WW3DAssetManager::Get_Texture() behavior
    pub fn get_texture_for_object(&self, object_name: &str) -> Option<String> {
        self.get_texture_for_object_with_model(object_name, None)
    }

    /// Get the model filename for an object
    pub fn get_model_for_object(&self, object_name: &str) -> Option<String> {
        if let Some(model) = self.model_cache.get(object_name) {
            return Some(model.clone());
        }

        self.resolve_object_definition(object_name, None)
            .and_then(|def| def.model_name.clone())
    }

    /// Select an exact source-authored model for the supplied C++
    /// ModelConditionFlags bit bank.  `None` means this Object identity is not
    /// present in the loaded catalogue; callers may then keep their separately
    /// authored template model, but must never synthesize a suffix variant.
    pub fn select_model_for_object_conditions(
        &self,
        object_name: &str,
        condition_bits: u128,
    ) -> Option<AuthoredConditionModelSelection> {
        self.resolve_object_definition(object_name, None)
            .map(|definition| definition.select_primary_model_for_conditions(condition_bits))
    }

    /// Select all exact W3D models contributed by source-authored Draw modules.
    ///
    /// `None` means the Object has no retained selectable Draw state (or is
    /// absent from the catalogue), so callers may keep an independent template
    /// model. `Some(vec![])` instead means source state exists but no module is
    /// safely drawable for this condition bank; callers must fail closed.
    pub fn select_draw_models_for_object_conditions(
        &self,
        object_name: &str,
        condition_bits: u128,
    ) -> Option<Vec<AuthoredDrawModel>> {
        self.resolve_object_definition(object_name, None)
            .and_then(|definition| definition.select_draw_models_for_conditions(condition_bits))
    }

    /// Get the full object definition
    pub fn get_object_definition(&self, object_name: &str) -> Option<&ObjectDefinition> {
        self.object_definitions.get(object_name)
    }

    /// Resolve an object definition by its authored Object INI identity.
    ///
    /// `model_hint` is retained at this boundary for callers that carry W3D
    /// context, but it is deliberately not an identity fallback.  A single
    /// W3D basename can be used by distinct faction, condition-state, or
    /// reskin definitions; choosing the first matching definition would
    /// borrow the wrong texture/state rather than reproduce C++ rendering.
    pub fn resolve_object_definition(
        &self,
        object_name: &str,
        _model_hint: Option<&str>,
    ) -> Option<&ObjectDefinition> {
        if let Some(def) = self.object_definitions.get(object_name) {
            return Some(def);
        }

        let normalized_key = Self::normalize_object_key(object_name);
        if let Some(canonical) = self.normalized_name_lookup.get(&normalized_key) {
            if let Some(def) = self.object_definitions.get(canonical) {
                return Some(def);
            }
        }

        None
    }

    /// Get all loaded object definitions
    pub fn get_all_objects(&self) -> &HashMap<String, ObjectDefinition> {
        &self.object_definitions
    }

    /// Take a stable, owned view of the resolved retail Object INI catalogue.
    ///
    /// Callers which seed gameplay templates must not retain a borrow through
    /// the asset-manager mutex.  Sorting also makes a complete seed
    /// deterministic, which matters for reproducible offline saves and
    /// diagnostics.
    pub fn object_definitions_snapshot(&self) -> Vec<(String, ObjectDefinition)> {
        let mut definitions: Vec<_> = self
            .object_definitions
            .iter()
            .map(|(name, definition)| (name.clone(), definition.clone()))
            .collect();
        definitions.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        definitions
    }

    /// Overlay leftover map.ini Object CREATE_OVERRIDES onto the live catalog.
    pub fn overlay_object_create_overrides(
        &mut self,
        name: &str,
        reskin_from: &str,
        properties: &HashMap<String, String>,
    ) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let lookup_key = Self::normalize_object_key(name);
        let canonical = self
            .normalized_name_lookup
            .get(&lookup_key)
            .cloned()
            .unwrap_or_else(|| name.to_string());
        let mut definition =
            if let Some(existing) = self.object_definitions.get(&canonical).cloned() {
                existing
            } else if !reskin_from.is_empty() {
                if let Some(parent) = self.resolve_object_definition(reskin_from, None).cloned() {
                    let mut child = parent;
                    child.name = name.to_string();
                    child.parent_name = Some(reskin_from.to_string());
                    child
                } else {
                    let mut child = ObjectDefinition::new(name.to_string());
                    child.parent_name = Some(reskin_from.to_string());
                    child
                }
            } else {
                ObjectDefinition::new(name.to_string())
            };
        definition.apply_create_override_properties(properties);
        self.register_definition_indices(&definition.name, &definition);
        if let Some(model) = definition.model_name.as_deref() {
            self.model_cache
                .insert(definition.name.clone(), model.to_string());
        }
        if let Some(texture) = definition.get_primary_texture() {
            self.texture_cache
                .insert(definition.name.clone(), texture.to_string());
        }
        self.object_definitions
            .insert(definition.name.clone(), definition);
    }

    /// Check if an object is defined
    pub fn has_object(&self, object_name: &str) -> bool {
        self.resolve_object_definition(object_name, None).is_some()
    }

    /// Get total number of objects loaded
    pub fn object_count(&self) -> usize {
        self.object_definitions.len()
    }

    /// Check if manager is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get all texture filenames that should be preloaded
    /// Returns a Vec of unique texture filenames defined in INI object definitions
    pub fn get_all_texture_filenames(&self) -> Vec<String> {
        let mut textures = std::collections::HashSet::new();

        // Collect all texture filenames from all object definitions
        for (_, def) in self.object_definitions.iter() {
            if let Some(texture) = def.get_primary_texture() {
                textures.insert(texture.to_string());
            }
        }

        // Convert to Vec and sort for consistent ordering
        let mut texture_vec: Vec<String> = textures.into_iter().collect();
        texture_vec.sort();
        texture_vec
    }
}

impl Default for WW3DAssetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WW3DAssetManager {
    fn register_definition_indices(&mut self, name: &str, _def: &ObjectDefinition) {
        self.normalized_name_lookup
            .entry(Self::normalize_object_key(name))
            .or_insert_with(|| name.to_string());
    }

    fn get_texture_for_object_with_model(
        &self,
        object_name: &str,
        model_hint: Option<&str>,
    ) -> Option<String> {
        if let Some(texture) = self.texture_cache.get(object_name) {
            return Some(texture.clone());
        }

        if let Some(def) = self.resolve_object_definition(object_name, model_hint) {
            if let Some(texture) = def.get_primary_texture() {
                return Some(texture.to_string());
            }
        }

        None
    }

    #[inline]
    fn normalize_object_key(name: &str) -> String {
        name.trim().to_ascii_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_manager_creation() {
        let manager = WW3DAssetManager::new();
        assert!(!manager.is_initialized());
        assert_eq!(manager.object_count(), 0);
    }

    #[test]
    fn test_texture_lookup() {
        let mut manager = WW3DAssetManager::new();

        // Manually add a test definition
        let mut def = ObjectDefinition::new("TestUnit".to_string());
        def.textures
            .insert("0".to_string(), "test_texture.tga".to_string());
        manager
            .object_definitions
            .insert("TestUnit".to_string(), def);
        manager
            .texture_cache
            .insert("TestUnit".to_string(), "test_texture.tga".to_string());

        let texture = manager.get_texture_for_object("TestUnit");
        assert_eq!(texture, Some("test_texture.tga".to_string()));
    }

    #[test]
    fn model_hint_does_not_substitute_a_different_object_definition() {
        let mut manager = WW3DAssetManager::new();
        let mut def = ObjectDefinition::new("AmericaVehicleHumvee".to_string());
        def.model_name = Some("AVHUMMER".to_string());
        def.textures
            .insert("0".to_string(), "avhummer.tga".to_string());

        manager
            .object_definitions
            .insert("AmericaVehicleHumvee".to_string(), def.clone());
        manager.register_definition_indices("AmericaVehicleHumvee", &def);

        assert_eq!(
            manager.get_texture_for_object_with_model("NotInDefinitions", Some("avhummer")),
            None
        );
    }

    #[test]
    fn object_definition_snapshot_is_owned_and_name_sorted() {
        let mut manager = WW3DAssetManager::new();
        manager.object_definitions.insert(
            "ZuluUnit".to_string(),
            ObjectDefinition::new("ZuluUnit".to_string()),
        );
        manager.object_definitions.insert(
            "AlphaUnit".to_string(),
            ObjectDefinition::new("AlphaUnit".to_string()),
        );

        let mut snapshot = manager.object_definitions_snapshot();
        assert_eq!(
            snapshot
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["AlphaUnit", "ZuluUnit"]
        );
        snapshot[0].1.display_name = "owned copy".to_string();
        assert!(
            manager
                .get_object_definition("AlphaUnit")
                .is_some_and(|definition| definition.display_name.is_empty())
        );
    }

    #[test]
    fn explicit_child_scale_one_resets_inherited_asset_scale() {
        let mut parent = ObjectDefinition::new("ScaledParent".to_string());
        parent.scale = 0.66;
        parent.scale_was_specified = true;

        let mut child = ObjectDefinition::new("ResetChild".to_string());
        child.parent_name = Some("ScaledParent".to_string());
        child.scale = 1.0;
        child.scale_was_specified = true;

        let resolved = WW3DAssetManager::merge_definition_inheritance(parent, child);
        assert_eq!(resolved.scale, 1.0);
        assert!(resolved.scale_was_specified);
    }

    #[test]
    fn inherited_draw_state_table_survives_child_and_reskin_resolution() {
        let source = r#"
Object BaseConditionUnit
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = BaseModel
    End
    ConditionState = DAMAGED
      Model = BaseModelDamaged
    End
  End
End

ChildObject PlainChild BaseConditionUnit
  DisplayName = INHERITS_DRAW_DATA
End

ObjectReskin ReskinnedChild BaseConditionUnit
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = ReskinModel
    End
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(source, "draw_inheritance.ini")
            .expect("parse source definitions");
        let base = parser
            .get_definition("BaseConditionUnit")
            .expect("base")
            .clone();
        let plain_child = parser
            .get_definition("PlainChild")
            .expect("plain child")
            .clone();
        let reskin_child = parser
            .get_definition("ReskinnedChild")
            .expect("reskin child")
            .clone();

        let inherited = WW3DAssetManager::merge_definition_inheritance(base.clone(), plain_child);
        assert_eq!(
            inherited.select_primary_model_for_conditions(0),
            AuthoredConditionModelSelection::Model("BaseModel".to_string()),
            "a child without Draw must retain its parent's source state table"
        );

        let reskinned = WW3DAssetManager::merge_definition_inheritance(base, reskin_child);
        assert_eq!(
            reskinned.select_primary_model_for_conditions(0),
            AuthoredConditionModelSelection::Model("ReskinModel".to_string()),
            "a reskin Draw table replaces rather than hybrid-merges the parent table"
        );
    }

    #[test]
    fn child_weapon_set_replaces_parent_collection_without_leaking_mine_detail() {
        let source = r#"
Object ParentMineClearer
  WeaponSet
    Conditions = None
    Weapon = PRIMARY ParentGun
  End
  WeaponSet
    Conditions = MINE_CLEARING_DETAIL
    Weapon = PRIMARY DozerMineDisarmingWeapon
  End
End

ChildObject ChildWithoutMineDetail ParentMineClearer
  WeaponSet
    Conditions = None
    Weapon = PRIMARY ChildGun
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(source, "weapon_set_inheritance.ini")
            .expect("parse parent and child WeaponSets");
        let parent = parser
            .get_definition("ParentMineClearer")
            .expect("parent")
            .clone();
        let child = parser
            .get_definition("ChildWithoutMineDetail")
            .expect("child")
            .clone();

        let resolved = WW3DAssetManager::merge_definition_inheritance(parent, child);
        assert_eq!(resolved.weapon_sets.len(), 1);
        assert_eq!(resolved.base_weapon_name(0), Some("ChildGun"));
        assert_eq!(
            resolved.mine_clearing_primary_weapon_name(),
            None,
            "C++ clears every copied WeaponSet on the child's first authored row"
        );
    }

    #[test]
    fn child_behavior_tags_replace_temporary_weapon_modules_in_place() {
        let source = r#"
Object ParentTemporaryWeaponProbe
  Behavior = FireWeaponWhenDamagedBehavior ModuleTag_TemporaryDamage
    ReactionWeaponDamaged = ParentReactionWeapon
  End
  Behavior = FireWeaponWhenDeadBehavior ModuleTag_TemporaryDeath
    DeathWeapon = ParentDeathWeapon
  End
End

ChildObject ChildTemporaryWeaponProbe ParentTemporaryWeaponProbe
  Behavior = FireWeaponWhenDamagedBehavior moduletag_temporarydamage
    ReactionWeaponDamaged = ChildReactionWeapon
    ContinuousWeaponRubble = ChildContinuousWeapon
  End
  Behavior = FireWeaponWhenDeadBehavior ModuleTag_AdditionalDeath
    DeathWeapon = ChildAdditionalDeathWeapon
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(source, "temporary_weapon_inheritance.ini")
            .expect("parse temporary weapon source");
        let parent = parser
            .get_definition("ParentTemporaryWeaponProbe")
            .expect("parent")
            .clone();
        let child = parser
            .get_definition("ChildTemporaryWeaponProbe")
            .expect("child")
            .clone();

        let resolved = WW3DAssetManager::merge_definition_inheritance(parent, child);
        assert_eq!(resolved.behavior_modules.len(), 3);
        assert_eq!(
            resolved.behavior_modules[0].module_tag.as_deref(),
            Some("moduletag_temporarydamage"),
            "case-insensitive tag identity replaces the parent module at its original index"
        );
        assert_eq!(
            resolved.behavior_modules[0].attribute("ReactionWeaponDamaged"),
            Some("ChildReactionWeapon")
        );
        assert_eq!(
            resolved.behavior_modules[1].attribute("DeathWeapon"),
            Some("ParentDeathWeapon"),
            "unmentioned tagged parent behavior remains source-position stable"
        );
        assert_eq!(
            resolved.behavior_modules[2].attribute("DeathWeapon"),
            Some("ChildAdditionalDeathWeapon"),
            "a new tag appends after inherited modules"
        );
    }

    #[test]
    fn catalogue_discovery_loads_default_object_ini_first() {
        let discovered = WW3DAssetManager::select_catalogue_object_ini_files([
            "Data/INI/Object/America.ini",
            "Data/INI/Crate.ini",
            r"Data\INI\Default\Object.ini",
            "Data/INI/Object/China.ini",
            "Data/INI/Weapon.ini",
        ]);
        assert_eq!(
            discovered[0].replace('\\', "/").to_ascii_lowercase(),
            "data/ini/default/object.ini"
        );
        assert!(
            discovered
                .iter()
                .any(|path| path.eq_ignore_ascii_case("Data/INI/Crate.ini"))
        );
        assert!(
            discovered
                .iter()
                .any(|path| path.eq_ignore_ascii_case("Data/INI/Object/America.ini"))
        );
        assert!(
            !discovered
                .iter()
                .any(|path| path.to_ascii_lowercase().contains("weapon.ini"))
        );
    }

    #[test]
    fn catalogue_discovery_requests_default_object_ini_even_when_unlistable() {
        let discovered =
            WW3DAssetManager::select_catalogue_object_ini_files(["Data/INI/Object/America.ini"]);
        assert_eq!(discovered[0], "Data/INI/Default/Object.ini");
    }

    #[test]
    fn unauthored_object_copies_default_thing_template_without_default_modules() {
        let source = r#"
Object DefaultThingTemplate
  VisionRange = 0.0
  Geometry = SPHERE
  GeometryMajorRadius = 1.0
  GeometryIsSmall = Yes
  Scale = 1.0
  Shadow = NONE
  TransportSlotCount = 0
  EnergyProduction = 0
  WeaponSet
    Conditions = None
    Weapon = PRIMARY None
    Weapon = SECONDARY None
    Weapon = TERTIARY None
  End
  ArmorSet
    Conditions = None
    Armor = NoArmor
    DamageFX = None
  End
  Behavior = DestroyDie ModuleTag_DefaultDestroyDie
  End
End

Object BareUnit
  DisplayName = BARE
  VisionRange = 150.0
End

Object UnauthoredUnit
  DisplayName = UNAUTHORED
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(source, "default_thing_template.ini")
            .expect("parse default + objects");
        let raw: HashMap<String, ObjectDefinition> = parser
            .get_all_definitions()
            .iter()
            .map(|(name, definition)| (name.clone(), definition.clone()))
            .collect();
        let mut resolved = HashMap::new();
        let mut stack = HashSet::new();
        let authored = WW3DAssetManager::resolve_inherited_definition(
            "BareUnit",
            &raw,
            &mut resolved,
            &mut stack,
        )
        .expect("BareUnit");
        let unauthored = WW3DAssetManager::resolve_inherited_definition(
            "UnauthoredUnit",
            &raw,
            &mut resolved,
            &mut stack,
        )
        .expect("UnauthoredUnit");

        fn attr<'a>(definition: &'a ObjectDefinition, key: &str) -> Option<&'a str> {
            definition
                .attributes
                .iter()
                .find_map(|(name, value)| name.eq_ignore_ascii_case(key).then(|| value.as_str()))
        }

        assert_eq!(attr(&authored, "visionrange"), Some("150.0"));
        assert_eq!(attr(&authored, "geometry"), Some("SPHERE"));
        assert_eq!(attr(&authored, "geometryissmall"), Some("Yes"));
        assert_eq!(attr(&unauthored, "visionrange"), Some("0.0"));
        assert_eq!(attr(&unauthored, "shadow"), Some("NONE"));
        assert_eq!(attr(&unauthored, "transportslotcount"), Some("0"));
        assert_eq!(attr(&unauthored, "energyproduction"), Some("0"));
        assert_eq!(unauthored.weapon_sets.len(), 1);
        assert_eq!(unauthored.base_weapon_name(0), None);
        assert_eq!(unauthored.armor_sets.len(), 1);
        assert!(
            unauthored.behavior_modules.is_empty(),
            "Default DestroyDie must not append onto an object that never authored modules"
        );
        assert!(authored.behavior_modules.is_empty());
    }
}
