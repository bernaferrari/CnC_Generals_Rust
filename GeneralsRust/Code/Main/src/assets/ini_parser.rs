////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// INI file parsing system - Matches C++ ObjectDefinition loading from INI files
// Reference: /GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2 and game object system

use anyhow::Result;
use log::{debug, trace};
use std::collections::HashMap;

/// Represents a drawable object definition from INI files
/// Matches C++ ObjectDefinition structure
#[derive(Debug, Clone)]
pub struct ObjectDefinition {
    /// Object name (e.g., "USA_Ranger", "ChinaInfantry")
    pub name: String,

    /// Optional parent object for ChildObject/ObjectReskin inheritance.
    pub parent_name: Option<String>,

    /// Object type (e.g., "Infantry", "Vehicle", "Building", "Aircraft")
    pub object_type: String,

    /// Display name for the UI
    pub display_name: String,

    /// Model filename (e.g., "USA_INFANTRY_RANGER.w3d")
    pub model_name: Option<String>,

    /// Texture names referenced by this object
    /// Maps material slot to texture filename
    pub textures: HashMap<String, String>,

    /// Draw module (rendering behavior)
    pub draw_module: Option<String>,

    /// Armor type
    pub armor_type: Option<String>,

    /// Health points
    pub hit_points: Option<u32>,

    /// Scale factor for the model
    pub scale: f32,

    /// Owner player (faction)
    pub owner: Option<String>,

    /// Other attributes from INI
    pub attributes: HashMap<String, String>,
}

impl ObjectDefinition {
    /// Create a new object definition
    pub fn new(name: String) -> Self {
        Self {
            name,
            parent_name: None,
            object_type: String::new(),
            display_name: String::new(),
            model_name: None,
            textures: HashMap::new(),
            draw_module: None,
            armor_type: None,
            hit_points: None,
            scale: 1.0,
            owner: None,
            attributes: HashMap::new(),
        }
    }

    /// Get the primary texture for this object
    pub fn get_primary_texture(&self) -> Option<&str> {
        self.textures
            .get("0")
            .map(|s| s.as_str())
            .or_else(|| self.textures.values().next().map(|s| s.as_str()))
    }
}

/// INI Parser for Generals object definitions
pub struct IniParser {
    /// Loaded object definitions indexed by name
    definitions: HashMap<String, ObjectDefinition>,
}

impl IniParser {
    /// Create a new INI parser
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
        }
    }

    /// Parse INI content from bytes
    pub fn parse_ini_content(&mut self, content: &str, filename: &str) -> Result<usize> {
        debug!("Parsing INI file: {}", filename);

        let lines: Vec<&str> = content.lines().collect();
        let mut current_object: Option<ObjectDefinition> = None;
        let mut current_condition_state = String::new();
        let mut object_count = 0;
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            let trimmed = Self::strip_inline_comment(trimmed).trim();

            // Skip empty lines and comments
            if trimmed.is_empty()
                || trimmed.starts_with(';')
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
            {
                continue;
            }

            // Section headers like [ObjectList] are ignored
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                continue;
            }

            // Object definition header: Object/ChildObject/ObjectReskin
            if Self::is_object_header(trimmed) {
                // Save previous object if any
                if let Some(obj) = current_object.take() {
                    self.definitions.insert(obj.name.clone(), obj);
                    object_count += 1;
                }

                let (class_name, parent_name) = Self::parse_object_header(trimmed)
                    .unwrap_or_else(|| ("UnnamedObject".to_string(), None));
                let mut object = ObjectDefinition::new(class_name);
                object.parent_name = parent_name;
                current_object = Some(object);
                current_condition_state.clear();
                trace!("Found object: {}", current_object.as_ref().unwrap().name);
                continue;
            }

            // End of object definition
            if trimmed.eq_ignore_ascii_case("End") {
                if current_object.is_some() && Self::is_object_terminator(&lines, index + 1) {
                    if let Some(obj) = current_object.take() {
                        self.definitions.insert(obj.name.clone(), obj);
                        object_count += 1;
                    }
                    current_condition_state.clear();
                } else {
                    // Nested block terminator (Draw/Behavior/Body/etc.).
                    current_condition_state.clear();
                }
                continue;
            }

            // Parse key = value pairs within an object
            if let Some(obj) = &mut current_object {
                if trimmed.eq_ignore_ascii_case("DefaultConditionState") {
                    current_condition_state = "default".to_string();
                    continue;
                }

                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos].trim();
                    let value = Self::strip_inline_comment(trimmed[eq_pos + 1..].trim()).trim();

                    // Remove quotes if present
                    let value = if (value.starts_with('"') && value.ends_with('"'))
                        || (value.starts_with('\'') && value.ends_with('\''))
                    {
                        &value[1..value.len() - 1]
                    } else {
                        value
                    };

                    // Parse specific fields
                    match key.to_lowercase().as_str() {
                        "type" => obj.object_type = value.to_string(),
                        "displayname" => obj.display_name = value.to_string(),
                        "conditionstate" => {
                            current_condition_state = value.to_ascii_lowercase();
                        }
                        "model" | "modelname" | "w3dmodel" => {
                            Self::assign_model_name(obj, value, &current_condition_state);
                        }
                        "drawmodule" | "draw" => obj.draw_module = Some(value.to_string()),
                        "armortype" => obj.armor_type = Some(value.to_string()),
                        "hitpoints" | "health" | "maxhealth" => {
                            obj.hit_points = value.parse().ok();
                        }
                        "scale" => {
                            obj.scale = value.parse().unwrap_or(1.0);
                        }
                        "owner" => obj.owner = Some(value.to_string()),
                        // Texture references (various formats used in C&C)
                        key if key.contains("texture") => {
                            obj.textures.insert(key.to_string(), value.to_string());
                        }
                        // Store other attributes
                        _ => {
                            obj.attributes.insert(key.to_string(), value.to_string());
                        }
                    }
                }
            }
        }

        // Don't forget the last object if file doesn't end with "End"
        if let Some(obj) = current_object.take() {
            self.definitions.insert(obj.name.clone(), obj);
            object_count += 1;
        }

        debug!("Parsed {} objects from {}", object_count, filename);
        Ok(object_count)
    }

    fn is_object_header(line: &str) -> bool {
        Self::parse_object_header(line).is_some()
    }

    fn parse_object_header(line: &str) -> Option<(String, Option<String>)> {
        if line.contains('=') {
            return None;
        }

        let mut tokens = line.split_whitespace();
        let head = tokens.next()?;
        match head {
            "Object" => tokens.next().map(|name| (name.to_string(), None)),
            "ChildObject" | "ObjectReskin" => {
                let name = tokens.next()?.to_string();
                let parent_name = tokens.next().map(|s| s.to_string());
                Some((name, parent_name))
            }
            _ => None,
        }
    }

    fn is_object_terminator(lines: &[&str], start_idx: usize) -> bool {
        for line in lines.iter().skip(start_idx) {
            let trimmed = line.trim();
            let trimmed = Self::strip_inline_comment(trimmed).trim();
            if trimmed.is_empty()
                || trimmed.starts_with(';')
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
            {
                continue;
            }
            return Self::is_object_header(trimmed);
        }
        true
    }

    fn strip_inline_comment(value: &str) -> &str {
        let bytes = value.as_bytes();
        let mut in_single = false;
        let mut in_double = false;
        let mut i = 0usize;

        while i < bytes.len() {
            match bytes[i] {
                b'\'' if !in_double => in_single = !in_single,
                b'"' if !in_single => in_double = !in_double,
                b';' | b'#' if !in_single && !in_double => return value[..i].trim_end(),
                b'/' if !in_single && !in_double && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                    return value[..i].trim_end()
                }
                _ => {}
            }
            i += 1;
        }

        value
    }

    fn assign_model_name(obj: &mut ObjectDefinition, value: &str, _condition_state: &str) {
        if value.is_empty() || value.eq_ignore_ascii_case("none") {
            return;
        }

        if obj.model_name.is_none() {
            obj.model_name = Some(value.to_string());
        }
    }

    /// Get an object definition by name
    pub fn get_definition(&self, name: &str) -> Option<&ObjectDefinition> {
        self.definitions.get(name)
    }

    /// Get all definitions
    pub fn get_all_definitions(&self) -> &HashMap<String, ObjectDefinition> {
        &self.definitions
    }

    /// Get total number of definitions loaded
    pub fn definition_count(&self) -> usize {
        self.definitions.len()
    }

    /// Clear all loaded definitions
    pub fn clear(&mut self) {
        self.definitions.clear();
    }
}

impl Default for IniParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_ini() {
        let ini_content = r#"
; Test INI content
Object USA_Ranger
  Type = Infantry
  DisplayName = "USA Ranger"
  Model = "USA_INFANTRY_RANGER.w3d"
  Texture = "USA_RANGER.tga"
  ArmorType = infantry
  HitPoints = 60
End
"#;

        let mut parser = IniParser::new();
        let count = parser.parse_ini_content(ini_content, "test.ini").unwrap();

        assert_eq!(count, 1);
        let def = parser.get_definition("USA_Ranger").unwrap();
        assert_eq!(def.object_type, "Infantry");
        assert_eq!(def.display_name, "USA Ranger");
        assert_eq!(def.model_name, Some("USA_INFANTRY_RANGER.w3d".to_string()));
        assert_eq!(def.hit_points, Some(60));
    }

    #[test]
    fn test_parse_multiple_objects() {
        let ini_content = r#"
Object Unit1
  Type = Infantry
End

Object Unit2
  Type = Vehicle
End

Object Unit3
  Type = Building
End
"#;

        let mut parser = IniParser::new();
        let count = parser.parse_ini_content(ini_content, "test.ini").unwrap();

        assert_eq!(count, 3);
        assert!(parser.get_definition("Unit1").is_some());
        assert!(parser.get_definition("Unit2").is_some());
        assert!(parser.get_definition("Unit3").is_some());
    }

    #[test]
    fn test_parse_object_reskin_parent_header() {
        let ini_content = r#"
Object BaseTree
  Type = Structure
  Model = BASETREE
End

ObjectReskin FancyTree BaseTree
  ModelName = FANCYTREE
End
"#;

        let mut parser = IniParser::new();
        let count = parser.parse_ini_content(ini_content, "test.ini").unwrap();

        assert_eq!(count, 2);
        let def = parser.get_definition("FancyTree").unwrap();
        assert_eq!(def.parent_name.as_deref(), Some("BaseTree"));
        assert_eq!(def.model_name.as_deref(), Some("FANCYTREE"));
    }

    #[test]
    fn test_nested_end_does_not_terminate_object() {
        let ini_content = r#"
Object TestStructure
  Draw = W3DModelDraw ModuleTag_01
    ConditionState = NONE
      Model = TESTMODEL
    End
    ConditionState = RUBBLE
      Model = NONE
    End
  End
  KindOf = STRUCTURE SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 1500
  End
End
"#;

        let mut parser = IniParser::new();
        let count = parser
            .parse_ini_content(ini_content, "test_nested.ini")
            .unwrap();

        assert_eq!(count, 1);
        let def = parser.get_definition("TestStructure").unwrap();
        assert_eq!(def.model_name.as_deref(), Some("TESTMODEL"));
        assert_eq!(def.hit_points, Some(1500));
        assert_eq!(
            def.attributes.get("KindOf").map(|s| s.as_str()),
            Some("STRUCTURE SELECTABLE")
        );
    }

    #[test]
    fn test_child_object_header_parsing() {
        let ini_content = r#"
ChildObject ChildTemplate ParentTemplate
  Model = CHILDMODEL
End
"#;

        let mut parser = IniParser::new();
        let count = parser.parse_ini_content(ini_content, "child.ini").unwrap();
        assert_eq!(count, 1);
        let def = parser.get_definition("ChildTemplate").unwrap();
        assert_eq!(def.model_name.as_deref(), Some("CHILDMODEL"));
    }

    #[test]
    fn test_modelname_and_draw_parse() {
        let ini_content = r#"
ObjectReskin Bush08 Bush01
  Draw = W3DTreeDraw ModuleTag_01
    ModelName = PTBush08
    TextureName = PTBush01.tga
  End
End
"#;

        let mut parser = IniParser::new();
        let count = parser
            .parse_ini_content(ini_content, "nature_prop.ini")
            .unwrap();
        assert_eq!(count, 1);
        let def = parser.get_definition("Bush08").unwrap();
        assert_eq!(def.model_name.as_deref(), Some("PTBush08"));
        assert_eq!(def.draw_module.as_deref(), Some("W3DTreeDraw ModuleTag_01"));
    }

    #[test]
    fn test_object_assignment_does_not_start_template() {
        let ini_content = r#"
Object TestStructure
  Behavior = GrantScienceUpgrade ModuleTag_Science
    GrantScience = SCIENCE_Test
    Object = TestHelperObject
  End
End
"#;

        let mut parser = IniParser::new();
        let count = parser
            .parse_ini_content(ini_content, "object_assignment.ini")
            .unwrap();

        assert_eq!(count, 1);
        assert!(parser.get_definition("TestStructure").is_some());
        assert!(parser.get_definition("=").is_none());
    }
}
