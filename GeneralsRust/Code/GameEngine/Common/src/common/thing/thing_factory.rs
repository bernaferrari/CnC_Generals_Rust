////////////////////////////////////////////////////////////////////////////////
//																			//
//  (c) 2001-2003 Electronic Arts Inc.										//
//																			//
////////////////////////////////////////////////////////////////////////////////

//! Thing factory for creating objects and drawables
//! This is how we make our things!

use crate::common::{
    global_data,
    ini::{INILoadType as IniLoadType, INI},
    random_value::get_game_logic_random_value,
    rts::{AsciiString, UnsignedShort},
    system::subsystem_interface::{SubsystemInterface, SubsystemResult, SubsystemState},
    thing::{
        module::{Drawable, Object},
        thing_template::ThingTemplate,
    },
};
use log::{debug, info, warn};
use std::{
    any::Any,
    collections::{BTreeSet, HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Template hash size constant
const TEMPLATE_HASH_SIZE: usize = 12288;

/// Object status mask type
pub type ObjectStatusMaskType = u32;

/// Drawable status type
pub type DrawableStatus = u32;

/// Constants for drawable status
pub const DRAWABLE_STATUS_NONE: DrawableStatus = 0;

/// Constants for object status mask
pub const OBJECT_STATUS_MASK_NONE: ObjectStatusMaskType = 0;

/// Error types for thing creation
#[derive(Debug)]
pub enum ThingCreationError {
    BadArg,
    TemplateNotFound,
    CreationFailed(String),
}

impl std::fmt::Display for ThingCreationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThingCreationError::BadArg => write!(f, "Bad argument provided"),
            ThingCreationError::TemplateNotFound => write!(f, "Template not found"),
            ThingCreationError::CreationFailed(msg) => write!(f, "Creation failed: {}", msg),
        }
    }
}

impl std::error::Error for ThingCreationError {}

/// Hash map type for thing templates
type ThingTemplateHashMap = HashMap<AsciiString, Arc<ThingTemplate>>;

/// Thing template load type for override handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThingLoadType {
    Normal,
    CreateOverrides,
}

/// Thing factory implementation
pub struct ThingFactory {
    first_template: Option<Arc<ThingTemplate>>,
    next_template_id: UnsignedShort,
    template_hash_map: ThingTemplateHashMap,
}

/// Object creator callback to bridge Common ThingFactory -> GameLogic.
pub trait ObjectCreator: Send + Sync {
    fn create_object(
        &self,
        template: &ThingTemplate,
        status_bits: ObjectStatusMaskType,
        team: Option<Arc<dyn Team>>,
    ) -> Result<Box<dyn Object>, ThingCreationError>;
}

/// Drawable creator callback to bridge Common ThingFactory -> GameClient.
pub trait DrawableCreator: Send + Sync {
    fn create_drawable(
        &self,
        template: &ThingTemplate,
        status_bits: DrawableStatus,
    ) -> Result<Box<dyn Drawable>, ThingCreationError>;
}

static OBJECT_CREATOR: Mutex<Option<Arc<dyn ObjectCreator>>> = Mutex::new(None);
static DRAWABLE_CREATOR: Mutex<Option<Arc<dyn DrawableCreator>>> = Mutex::new(None);

pub fn set_object_creator(creator: Option<Arc<dyn ObjectCreator>>) {
    if let Ok(mut guard) = OBJECT_CREATOR.lock() {
        *guard = creator;
    }
}

pub fn set_drawable_creator(creator: Option<Arc<dyn DrawableCreator>>) {
    if let Ok(mut guard) = DRAWABLE_CREATOR.lock() {
        *guard = creator;
    }
}
/// Read key=value lines from the INI stream until `End` is encountered.
///
/// This mirrors the C++ `initFromINI` approach where the INI cursor is already
/// positioned inside an `Object ... End` block.  Each line is split on `=` and
/// collected into a HashMap.  Sub-blocks (e.g. `Behavior ... End`,
/// `WeaponSet ... End`) are consumed but their contents are stored under a
/// compound key like `"WeaponSet0.Conditions"` so that dedicated parsers can
/// pick them up later.
fn consume_ini_properties(ini: &mut INI) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    let mut block_key_prefix: Option<String> = None;
    let mut block_depth = 0u32;
    let mut block_counter = 0u32;

    loop {
        ini.read_line().ok();
        if ini.is_end_of_file() {
            break;
        }

        let line = ini.get_buffer().to_string();
        if line.is_empty() {
            continue;
        }

        let first_token = line.split_whitespace().next().unwrap_or("");

        // Handle block nesting
        if first_token.eq_ignore_ascii_case("End") {
            if block_depth > 0 {
                block_depth -= 1;
                if block_depth == 0 {
                    block_key_prefix = None;
                }
                continue;
            }
            // Top-level End -- we're done
            break;
        }

        // Detect sub-block starts (lines without '=' that aren't End)
        if !line.contains('=') && block_depth == 0 {
            // This is a sub-block keyword like "Behavior", "WeaponSet", etc.
            // Generate a prefixed key for this block
            let block_name = first_token;
            if block_name.eq_ignore_ascii_case("WeaponSet") {
                block_key_prefix = Some(format!("WeaponSet{}", block_counter));
                block_counter += 1;
                block_depth = 1;
            } else if block_name.eq_ignore_ascii_case("ArmorSet") {
                block_key_prefix = Some(format!("ArmorSet{}", block_counter));
                block_counter += 1;
                block_depth = 1;
            } else {
                // Generic sub-block (Behavior, Draw, etc.) -- skip entirely
                block_depth = 1;
                block_key_prefix = None;
            }
            continue;
        }

        // Inside a sub-block
        if block_depth > 0 {
            if !line.contains('=') {
                // Nested sub-block start inside our block
                block_depth += 1;
                continue;
            }

            // Store with prefix if we have one (WeaponSet/ArmorSet)
            if let Some(ref prefix) = block_key_prefix {
                if let Some(eq_pos) = line.find('=') {
                    let key = format!("{}.{}", prefix, line[..eq_pos].trim());
                    let value = line[eq_pos + 1..].trim().to_string();
                    properties.insert(key, value);
                }
            }
            // Other sub-blocks are silently consumed
            continue;
        }

        // Top-level key = value
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            properties.insert(key, value);
        }
    }

    properties
}

impl ThingFactory {
    pub fn new() -> Self {
        Self {
            first_template: None,
            next_template_id: 1, // Not zero!
            template_hash_map: HashMap::with_capacity(TEMPLATE_HASH_SIZE),
        }
    }

    /// Create a new template with the given name
    pub fn new_template(&mut self, name: &str) -> Arc<ThingTemplate> {
        let mut new_template = ThingTemplate::new();

        // If default template is present, copy data from it
        if let Some(default_template) = self.find_template("DefaultThingTemplate", false) {
            new_template.copy_from(&default_template);
            new_template.set_copied_from_default();
        }

        // Give template a unique identifier
        new_template.set_template_id(self.next_template_id);
        self.next_template_id += 1;
        debug_assert!(self.next_template_id != 0, "Template ID wrapped to zero");

        // Assign name
        new_template.set_template_name(AsciiString::from(name));

        let template_arc = Arc::new(new_template);

        // Add to list and hash map
        self.add_template(template_arc.clone());

        template_arc
    }

    /// Create a new override template
    pub fn new_override(
        &mut self,
        thing_template: Arc<ThingTemplate>,
    ) -> Result<Arc<ThingTemplate>, String> {
        // Verify the template exists in master list
        let template_exists = self
            .find_template(&thing_template.get_name(), false)
            .is_some();
        if !template_exists {
            return Err(format!(
                "Template '{}' not in master list",
                thing_template.get_name()
            ));
        }

        // Find final override of the parent template
        let final_override = ThingTemplate::get_final_override(&thing_template);

        // Create new template and copy data
        let mut new_template = ThingTemplate::new();
        new_template.copy_from(final_override.as_ref());
        new_template.set_copied_from_default();
        new_template.mark_as_override();

        let new_template_arc = Arc::new(new_template);

        // Link as override
        final_override.set_next_override(Some(new_template_arc.clone()));

        Ok(new_template_arc)
    }

    /// Get the first template in the list
    pub fn first_template(&self) -> Option<&Arc<ThingTemplate>> {
        self.first_template.as_ref()
    }

    /// Find a template by name
    pub fn find_template(&self, name: &str, check: bool) -> Option<Arc<ThingTemplate>> {
        if let Some(template) = self.template_hash_map.get(name) {
            return Some(template.clone());
        }

        #[cfg(feature = "load_test_assets")]
        {
            const TEST_STRING: &str = "TEST_";
            if name.starts_with(TEST_STRING) {
                // Create test template on demand
                let mut template = self.new_template("Un-namedTemplate");
                template.init_for_lta(&AsciiString::from(name));

                // Update hash map
                self.template_hash_map.remove("Un-namedTemplate");
                self.template_hash_map
                    .insert(AsciiString::from(name), template.clone());

                return Some(template);
            }
        }

        if check && !name.is_empty() {
            panic!("Failed to find thing template {} (case sensitive)", name);
        }

        None
    }

    /// Find template by ID
    pub fn find_by_template_id(&self, id: UnsignedShort) -> Option<Arc<ThingTemplate>> {
        let mut current = self.first_template.as_ref();
        while let Some(template) = current {
            if template.get_template_id() == id {
                return Some(template.clone());
            }
            current = template.get_next_template().as_ref();
        }
        None
    }

    /// Create a new object from template
    pub fn new_object(
        &self,
        template: &ThingTemplate,
        team: Option<Arc<dyn Team>>,
        status_bits: ObjectStatusMaskType,
    ) -> Result<Box<dyn Object>, ThingCreationError> {
        // Hold an optional Arc to a variation template if we find one
        let variation_holder: Option<Arc<ThingTemplate>>;

        let variations = template.get_build_variations();
        let final_template = if !variations.is_empty() {
            let max = variations.len().saturating_sub(1) as i32;
            let index = if max == 0 {
                0
            } else {
                get_game_logic_random_value(0, max) as usize
            };
            if let Some(variation_name) = variations.get(index) {
                if let Some(variation) = self.find_template(variation_name.as_str(), false) {
                    variation_holder = Some(variation);
                    // Return reference to the held variation
                    variation_holder.as_ref().unwrap().as_ref()
                } else {
                    variation_holder = None;
                    template
                }
            } else {
                variation_holder = None;
                template
            }
        } else {
            variation_holder = None;
            template
        };
        let _ = &variation_holder; // suppress unused_assignments

        // Verify template is not drawable-only
        if final_template.is_kind_of(0x2000) {
            // KINDOF_DRAWABLE_ONLY placeholder
            return Err(ThingCreationError::CreationFailed(format!(
                "Cannot create Objects with template {}, only Drawables",
                final_template.get_name()
            )));
        }

        // Create the object (this would interface with GameLogic)
        let obj = self.create_object_impl(final_template, status_bits, team)?;

        // Run create functions for all behavior modules
        for module in obj.get_behavior_modules() {
            if let Some(create_interface) = module.get_create_interface() {
                create_interface.on_create();
            }
        }

        // Register with partition manager
        // ThePartitionManager->registerObject(obj);

        // Initialize the object
        obj.init_object();

        Ok(obj)
    }

    /// Create a new drawable from template
    pub fn new_drawable(
        &self,
        template: &ThingTemplate,
        status_bits: DrawableStatus,
    ) -> Result<Box<dyn Drawable>, ThingCreationError> {
        // Create the drawable (this would interface with GameClient)
        let drawable = self.create_drawable_impl(template, status_bits)?;

        Ok(drawable)
    }

    /// Parse object definition from INI
    pub fn parse_object_definition(
        &mut self,
        ini: &mut INI,
        name: &str,
        reskin_from: &str,
    ) -> Result<(), String> {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            // Set global variable for debugging
            // TheThingTemplateBeingParsedName = name.clone();
        }

        // Find existing template or create new one
        let mut thing_template = if let Some(existing) = self.find_template(name, false) {
            if ini.get_load_type() != IniLoadType::CreateOverrides {
                return Err(format!("Duplicate thing template {} found!", name));
            }
            self.new_override(existing)?
        } else {
            let template = self.new_template(name);
            if ini.get_load_type() == IniLoadType::CreateOverrides {
                // Mark as override for proper cleanup
                // template.mark_as_override();
            }
            template
        };

        // Handle reskinning
        if !reskin_from.is_empty() {
            if let Some(_reskin_template) = self.find_template(reskin_from, false) {
                // Note: This would require mutable access to thing_template
                // thing_template.copy_from(&reskin_template);
                // thing_template.set_copied_from_default();
                // thing_template.set_reskinned_from(&reskin_template);
                // ini.init_from_ini(&mut thing_template, thing_template.get_reskin_field_parse());
            } else {
                return Err(format!(
                    "ObjectReskin must come after the original Object ({}, {})",
                    reskin_from, name
                ));
            }
        } else {
            // Regular initialization -- parse INI fields into the template.
            //
            // C++ does: ini->initFromINI(self, self->getFieldParse());
            // We read the remaining key=value lines from the INI block and
            // apply them via parse_object_fields_from_ini.
            let properties = consume_ini_properties(ini);
            // Use Arc::make_mut to get mutable access for parsing
            let tmpl = Arc::make_mut(&mut thing_template);
            tmpl.parse_object_fields_from_ini(&properties);

            // Re-insert the modified template back into the hash map since
            // Arc::make_mut may have cloned it.
            self.template_hash_map
                .insert(AsciiString::from(name), thing_template.clone());
        }

        // Validate the template
        // thing_template.validate();

        if ini.get_load_type() == IniLoadType::CreateOverrides {
            // thing_template.resolve_names();
        }

        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            // Clear global variable
            // TheThingTemplateBeingParsedName.clear();
        }

        Ok(())
    }

    /// Add template to the database
    fn add_template(&mut self, template: Arc<ThingTemplate>) {
        // Check for duplicates
        if self.template_hash_map.contains_key(template.get_name()) {
            panic!(
                "Duplicate Thing Template name found: {}",
                template.get_name()
            );
        }

        // Link to list
        if let Some(ref mut _first) = self.first_template {
            // template.set_next_template(Some(first.clone()));
        }

        // Add to hash map
        self.template_hash_map
            .insert(template.get_name().clone(), template.clone());

        // Update first template
        self.first_template = Some(template);
    }

    /// Free all template database data
    fn free_database(&mut self) {
        // Clear all templates
        self.first_template = None;
        self.template_hash_map.clear();
    }

    /// Implementation detail - create object
    fn create_object_impl(
        &self,
        _template: &ThingTemplate,
        _status_bits: ObjectStatusMaskType,
        _team: Option<Arc<dyn Team>>,
    ) -> Result<Box<dyn Object>, ThingCreationError> {
        let creator = OBJECT_CREATOR
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .ok_or_else(|| {
                ThingCreationError::CreationFailed("Object creator not registered".to_string())
            })?;
        creator.create_object(_template, _status_bits, _team)
    }

    /// Implementation detail - create drawable
    fn create_drawable_impl(
        &self,
        _template: &ThingTemplate,
        _status_bits: DrawableStatus,
    ) -> Result<Box<dyn Drawable>, ThingCreationError> {
        let creator = DRAWABLE_CREATOR
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .ok_or_else(|| {
                ThingCreationError::CreationFailed("Drawable creator not registered".to_string())
            })?;
        creator.create_drawable(_template, _status_bits)
    }
}

impl SubsystemInterface for ThingFactory {
    fn name(&self) -> &str {
        "ThingFactory"
    }

    fn init(&mut self) -> SubsystemResult<()> {
        // Initialization if needed
        Ok(())
    }

    fn reset(&mut self) -> SubsystemResult<()> {
        // Go through all templates and delete overrides
        let mut templates_to_remove = Vec::new();

        for (name, template) in &self.template_hash_map {
            // Check if template was created for this map only
            if template.is_override() {
                templates_to_remove.push(name.clone());
            }

            // Delete overrides for this template
            template.delete_overrides();
        }

        // Remove templates that were map-specific
        for name in templates_to_remove {
            self.template_hash_map.remove(&name);
        }

        // Update first template if it was removed
        if let Some(ref first) = self.first_template {
            if !self.template_hash_map.contains_key(first.get_name()) {
                // Find new first template
                self.first_template = self.template_hash_map.values().next().cloned();
            }
        }
        Ok(())
    }

    fn update(&mut self, _delta_time: std::time::Duration) -> SubsystemResult<()> {
        // No regular updates needed
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        // Clean up resources
        self.template_hash_map.clear();
        self.first_template = None;
        Ok(())
    }

    fn state(&self) -> SubsystemState {
        SubsystemState::Running // Simplified for now
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self as &(dyn Any + Send + Sync)
    }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self as &mut (dyn Any + Send + Sync)
    }

    fn post_process_load(&mut self) -> SubsystemResult<()> {
        // Go through all thing templates and resolve names
        for _template in self.template_hash_map.values() {
            // template.resolve_names();
        }

        Ok(())
    }
}

/// Forward declaration for Team
pub trait Team: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn team_id(&self) -> Option<u32> {
        None
    }
}

/// Global thing factory singleton
static THING_FACTORY: Mutex<Option<ThingFactory>> = Mutex::new(None);

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObjectDeclaration {
    name: String,
    reskin_from: Option<String>,
}

/// Get the global thing factory instance
pub fn get_thing_factory() -> Result<
    std::sync::MutexGuard<'static, Option<ThingFactory>>,
    std::sync::PoisonError<std::sync::MutexGuard<'static, Option<ThingFactory>>>,
> {
    THING_FACTORY.lock()
}

/// Initialize the global thing factory
pub fn init_thing_factory() -> Result<(), String> {
    let mut factory = ThingFactory::new();
    factory
        .init()
        .map_err(|e| format!("Failed to initialize thing factory: {:?}", e))?;

    let loaded = load_runtime_object_templates(&mut factory)?;
    if loaded > 0 {
        info!("ThingFactory loaded {} object declarations", loaded);
    } else {
        debug!("ThingFactory initialized without runtime object declarations");
    }

    let mut factory_guard = get_thing_factory().map_err(|_| "Failed to lock thing factory")?;
    *factory_guard = Some(factory);
    Ok(())
}

/// Shutdown the global thing factory
pub fn shutdown_thing_factory() {
    if let Ok(mut factory_guard) = get_thing_factory() {
        *factory_guard = None;
    }
}

fn load_runtime_object_templates(factory: &mut ThingFactory) -> Result<usize, String> {
    let sources = discover_object_ini_sources();
    if sources.is_empty() {
        return Ok(0);
    }

    let mut declarations = Vec::new();
    for path in sources {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) => {
                warn!("Failed to read object INI '{}': {}", path.display(), err);
                continue;
            }
        };

        let mut parsed = parse_object_declarations(&contents);
        if parsed.is_empty() {
            debug!(
                "No object declarations discovered in '{}'",
                path.to_string_lossy()
            );
        }
        declarations.append(&mut parsed);
    }

    let mut seen = HashSet::new();
    let mut bases = Vec::new();
    let mut reskins = Vec::new();
    for declaration in declarations {
        let normalized = declaration.name.to_ascii_lowercase();
        if !seen.insert(normalized) {
            continue;
        }
        if declaration.reskin_from.is_some() {
            reskins.push(declaration);
        } else {
            bases.push(declaration);
        }
    }

    let mut loaded = 0usize;
    for declaration in bases.into_iter().chain(reskins.into_iter()) {
        if factory.find_template(&declaration.name, false).is_some() {
            continue;
        }

        let _template = factory.new_template(&declaration.name);
        loaded += 1;
    }

    Ok(loaded)
}

fn discover_object_ini_sources() -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();

    if let Ok(cwd) = env::current_dir() {
        for ancestor in cwd.ancestors() {
            roots.insert(ancestor.to_path_buf());
        }
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            for ancestor in parent.ancestors() {
                roots.insert(ancestor.to_path_buf());
            }
        }
    }

    let mod_dir = {
        let guard = global_data::read();
        guard.writable.mod_dir.clone()
    };
    if !mod_dir.trim().is_empty() {
        let mod_root = PathBuf::from(mod_dir.trim());
        roots.insert(mod_root.clone());
        if let Ok(canonical) = fs::canonicalize(&mod_root) {
            roots.insert(canonical);
        }
    }

    let mut seen = HashSet::new();
    let mut files = Vec::new();

    for root in roots {
        push_object_ini_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/Default/Object.ini"),
        );
        push_object_ini_file(&mut files, &mut seen, root.join("Data/INI/Object.ini"));
        push_object_ini_dir(&mut files, &mut seen, &root.join("Data/INI/Object"));

        for extracted in [
            root.join("windows_game/extracted_big_files/INIZH"),
            root.join("windows_game/extracted_big_files_v2/INIZH"),
        ] {
            push_object_ini_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/Default/Object.ini"),
            );
            push_object_ini_file(&mut files, &mut seen, extracted.join("Data/INI/Object.ini"));
            push_object_ini_dir(&mut files, &mut seen, &extracted.join("Data/INI/Object"));
        }
    }

    files
}

fn push_object_ini_file(files: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if path.is_file() {
        let key = fs::canonicalize(&path).unwrap_or(path.clone());
        if seen.insert(key) {
            files.push(path);
        }
    }
}

fn push_object_ini_dir(files: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let mut ini_files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_ini = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("ini"))
            .unwrap_or(false);
        if is_ini && path.is_file() {
            ini_files.push(path);
        }
    }
    ini_files.sort();

    for path in ini_files {
        push_object_ini_file(files, seen, path);
    }
}

fn strip_ini_comment(line: &str) -> &str {
    line.split_once(';')
        .map(|(prefix, _)| prefix)
        .unwrap_or(line)
}

fn parse_object_declarations(contents: &str) -> Vec<ObjectDeclaration> {
    let mut declarations = Vec::new();
    let lines: Vec<&str> = contents.lines().collect();
    let mut index = 0usize;

    while index < lines.len() {
        let line = strip_ini_comment(lines[index]).trim();
        if line.is_empty() {
            index += 1;
            continue;
        }

        let tokens: Vec<&str> = line
            .split_whitespace()
            .filter(|token| *token != "=")
            .collect();
        let Some(keyword) = tokens.first().copied() else {
            index += 1;
            continue;
        };

        if keyword.eq_ignore_ascii_case("Object") {
            if let Some(name) = tokens.get(1) {
                declarations.push(ObjectDeclaration {
                    name: (*name).to_string(),
                    reskin_from: None,
                });
            }
            index = skip_ini_block(&lines, index + 1);
            continue;
        }

        if keyword.eq_ignore_ascii_case("ObjectReskin") {
            if let (Some(name), Some(source)) = (tokens.get(1), tokens.get(2)) {
                declarations.push(ObjectDeclaration {
                    name: (*name).to_string(),
                    reskin_from: Some((*source).to_string()),
                });
            }
            index = skip_ini_block(&lines, index + 1);
            continue;
        }

        index += 1;
    }

    declarations
}

fn skip_ini_block(lines: &[&str], mut index: usize) -> usize {
    let mut nested_depth = 0usize;

    while index < lines.len() {
        let line = strip_ini_comment(lines[index]).trim();
        if line.is_empty() {
            index += 1;
            continue;
        }

        let tokens: Vec<&str> = line
            .split_whitespace()
            .filter(|token| *token != "=")
            .collect();
        let Some(first) = tokens.first().copied() else {
            index += 1;
            continue;
        };

        if first.eq_ignore_ascii_case("End") {
            if nested_depth == 0 {
                return index + 1;
            }
            nested_depth = nested_depth.saturating_sub(1);
            index += 1;
            continue;
        }

        if !line.contains('=') {
            nested_depth += 1;
        }

        index += 1;
    }

    index
}

#[cfg(test)]
mod tests {
    use super::{parse_object_declarations, ObjectDeclaration};

    #[test]
    fn parse_object_declarations_handles_object_and_reskin_blocks() {
        let contents = r#"
            Object AmericaVehicleHumvee
              Draw = W3DModelDraw ModuleTag_01
                OkToChangeModelColor = Yes
              End
            End

            ObjectReskin AmericaVehicleBattleBus AmericaVehicleHumvee
              Draw = W3DModelDraw ModuleTag_02
              End
            End
        "#;

        let parsed = parse_object_declarations(contents);
        assert_eq!(
            parsed,
            vec![
                ObjectDeclaration {
                    name: "AmericaVehicleHumvee".to_string(),
                    reskin_from: None,
                },
                ObjectDeclaration {
                    name: "AmericaVehicleBattleBus".to_string(),
                    reskin_from: Some("AmericaVehicleHumvee".to_string()),
                },
            ]
        );
    }
}
