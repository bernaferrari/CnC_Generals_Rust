//! # Parameter System
//!
//! This module provides a comprehensive parameter system for configuration management in the
//! WWSaveLoad library. It supports various parameter types including primitives, strings,
//! vectors, matrices, enums, and complex game object definitions.
//!
//! The system is designed to be type-safe, efficient, and compatible with the original
//! C++ parameter system while leveraging Rust's strengths in memory safety and error handling.
//!
//! ## Features
//!
//! - **Type Safety**: All parameter types are validated at compile-time and runtime
//! - **Memory Safety**: No manual memory management required
//! - **Error Handling**: Comprehensive error reporting using Result types
//! - **Serialization**: Compatible with save/load system
//! - **Validation**: Built-in range checking and value validation
//! - **Performance**: Zero-cost abstractions where possible
//!
//! ## Example Usage
//!
//! ```rust
//! use ww_save_load::parameter::{Parameter, ParameterType, ParameterValue, ParameterList};
//!
//! // Create an integer parameter with range validation
//! let mut int_param = Parameter::new_int("health", 100, Some((0, 1000))).unwrap();
//! int_param.set_value(ParameterValue::Int(150)).unwrap();
//!
//! // Create a string parameter
//! let mut str_param = Parameter::new_string("name", "Player".to_string()).unwrap();
//!
//! // Create a parameter list
//! let mut params = ParameterList::new();
//! params.add(int_param).unwrap();
//! params.add(str_param).unwrap();
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Error types for parameter operations
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterError {
    /// Invalid parameter type conversion
    TypeMismatch {
        expected: ParameterType,
        found: ParameterType,
    },
    /// Value outside allowed range
    ValueOutOfRange {
        value: String,
        min: String,
        max: String,
    },
    /// Invalid parameter name
    InvalidName(String),
    /// Parameter not found
    NotFound(String),
    /// Invalid enum value
    InvalidEnumValue { value: i32, valid_values: Vec<i32> },
    /// Invalid string format
    InvalidStringFormat(String),
    /// IO or serialization error
    SerializationError(String),
    /// Generic parameter error
    Generic(String),
}

impl fmt::Display for ParameterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParameterError::TypeMismatch { expected, found } => {
                write!(
                    f,
                    "Type mismatch: expected {:?}, found {:?}",
                    expected, found
                )
            }
            ParameterError::ValueOutOfRange { value, min, max } => {
                write!(f, "Value {} is outside range [{}, {}]", value, min, max)
            }
            ParameterError::InvalidName(name) => {
                write!(f, "Invalid parameter name: {}", name)
            }
            ParameterError::NotFound(name) => {
                write!(f, "Parameter not found: {}", name)
            }
            ParameterError::InvalidEnumValue {
                value,
                valid_values,
            } => {
                write!(
                    f,
                    "Invalid enum value {}, valid values: {:?}",
                    value, valid_values
                )
            }
            ParameterError::InvalidStringFormat(msg) => {
                write!(f, "Invalid string format: {}", msg)
            }
            ParameterError::SerializationError(msg) => {
                write!(f, "Serialization error: {}", msg)
            }
            ParameterError::Generic(msg) => {
                write!(f, "Parameter error: {}", msg)
            }
        }
    }
}

impl std::error::Error for ParameterError {}

/// Result type for parameter operations
pub type ParameterResult<T> = Result<T, ParameterError>;

/// Parameter types corresponding to the original C++ enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParameterType {
    Int,
    Float,
    String,
    Vector2,
    Vector3,
    Matrix3D,
    Bool,
    Transition,
    ModelDefinitionId,
    Filename,
    Enum,
    GameObjDefinitionId,
    Script,
    SoundFilename,
    Angle,
    WeaponObjDefinitionId,
    AmmoObjDefinitionId,
    SoundDefinitionId,
    Color,
    PhysDefinitionId,
    ExplosionDefinitionId,
    DefinitionIdList,
    Zone,
    FilenameList,
    Separator,
    GenericDefinitionId,
    ScriptList,
    Rect,
    TextureFilename,
    StringsDbId,
}

/// 2D Vector type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Vector2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0)
    }
}

/// 3D Vector type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

/// 3D Matrix type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Matrix3D {
    pub m: [[f32; 4]; 4],
}

impl Matrix3D {
    pub fn identity() -> Self {
        let mut m = [[0.0; 4]; 4];
        m[0][0] = 1.0;
        m[1][1] = 1.0;
        m[2][2] = 1.0;
        m[3][3] = 1.0;
        Self { m }
    }
}

/// Rectangle type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}

/// Oriented Bounding Box for zones
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OBBox {
    pub center: Vector3,
    pub extent: Vector3,
    pub orientation: Matrix3D,
}

impl OBBox {
    pub fn new(center: Vector3, extent: Vector3, orientation: Matrix3D) -> Self {
        Self {
            center,
            extent,
            orientation,
        }
    }
}

/// Script information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Script {
    pub name: String,
    pub parameters: String,
}

impl Script {
    pub fn new(name: String, parameters: String) -> Self {
        Self { name, parameters }
    }
}

/// Enum value with display name
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumValue {
    pub name: String,
    pub value: i32,
}

impl EnumValue {
    pub fn new(name: String, value: i32) -> Self {
        Self { name, value }
    }
}

/// Parameter values - all possible value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterValue {
    Int(i32),
    Float(f32),
    String(String),
    Vector2(Vector2),
    Vector3(Vector3),
    Matrix3D(Matrix3D),
    Bool(bool),
    Transition(i32), // Transition ID
    ModelDefinitionId(i32),
    Filename(String),
    Enum {
        value: i32,
        options: Vec<EnumValue>,
    },
    GameObjDefinitionId {
        value: i32,
        base_class: Option<String>,
    },
    Script(Script),
    SoundFilename(String),
    Angle(f32), // In radians
    WeaponObjDefinitionId {
        value: i32,
        base_class: Option<String>,
    },
    AmmoObjDefinitionId {
        value: i32,
        base_class: Option<String>,
    },
    SoundDefinitionId(i32),
    Color(Vector3), // RGB as Vector3
    PhysDefinitionId {
        value: i32,
        base_class: Option<String>,
    },
    ExplosionDefinitionId {
        value: i32,
        base_class: Option<String>,
    },
    DefinitionIdList {
        values: Vec<i32>,
        class_id: u32,
        selected_class_id: Option<u32>,
    },
    Zone(OBBox),
    FilenameList(Vec<String>),
    Separator, // Visual separator with no data
    GenericDefinitionId {
        value: i32,
        class_id: i32,
    },
    ScriptList {
        names: Vec<String>,
        parameters: Vec<String>,
    },
    Rect(Rect),
    TextureFilename {
        path: String,
        show_alpha: bool,
        show_texture: bool,
    },
    StringsDbId(i32),
}

impl ParameterValue {
    /// Get the parameter type of this value
    pub fn get_type(&self) -> ParameterType {
        match self {
            ParameterValue::Int(_) => ParameterType::Int,
            ParameterValue::Float(_) => ParameterType::Float,
            ParameterValue::String(_) => ParameterType::String,
            ParameterValue::Vector2(_) => ParameterType::Vector2,
            ParameterValue::Vector3(_) => ParameterType::Vector3,
            ParameterValue::Matrix3D(_) => ParameterType::Matrix3D,
            ParameterValue::Bool(_) => ParameterType::Bool,
            ParameterValue::Transition(_) => ParameterType::Transition,
            ParameterValue::ModelDefinitionId(_) => ParameterType::ModelDefinitionId,
            ParameterValue::Filename(_) => ParameterType::Filename,
            ParameterValue::Enum { .. } => ParameterType::Enum,
            ParameterValue::GameObjDefinitionId { .. } => ParameterType::GameObjDefinitionId,
            ParameterValue::Script(_) => ParameterType::Script,
            ParameterValue::SoundFilename(_) => ParameterType::SoundFilename,
            ParameterValue::Angle(_) => ParameterType::Angle,
            ParameterValue::WeaponObjDefinitionId { .. } => ParameterType::WeaponObjDefinitionId,
            ParameterValue::AmmoObjDefinitionId { .. } => ParameterType::AmmoObjDefinitionId,
            ParameterValue::SoundDefinitionId(_) => ParameterType::SoundDefinitionId,
            ParameterValue::Color(_) => ParameterType::Color,
            ParameterValue::PhysDefinitionId { .. } => ParameterType::PhysDefinitionId,
            ParameterValue::ExplosionDefinitionId { .. } => ParameterType::ExplosionDefinitionId,
            ParameterValue::DefinitionIdList { .. } => ParameterType::DefinitionIdList,
            ParameterValue::Zone(_) => ParameterType::Zone,
            ParameterValue::FilenameList(_) => ParameterType::FilenameList,
            ParameterValue::Separator => ParameterType::Separator,
            ParameterValue::GenericDefinitionId { .. } => ParameterType::GenericDefinitionId,
            ParameterValue::ScriptList { .. } => ParameterType::ScriptList,
            ParameterValue::Rect(_) => ParameterType::Rect,
            ParameterValue::TextureFilename { .. } => ParameterType::TextureFilename,
            ParameterValue::StringsDbId(_) => ParameterType::StringsDbId,
        }
    }

    /// Convert value to string representation
    pub fn to_string(&self) -> String {
        match self {
            ParameterValue::Int(v) => v.to_string(),
            ParameterValue::Float(v) => v.to_string(),
            ParameterValue::String(v) => v.clone(),
            ParameterValue::Bool(v) => v.to_string(),
            ParameterValue::Angle(v) => format!("{:.6}", v),
            ParameterValue::Filename(v) => v.clone(),
            ParameterValue::SoundFilename(v) => v.clone(),
            ParameterValue::Vector2(v) => format!("({}, {})", v.x, v.y),
            ParameterValue::Vector3(v) => format!("({}, {}, {})", v.x, v.y, v.z),
            ParameterValue::Color(v) => format!("RGB({}, {}, {})", v.x, v.y, v.z),
            ParameterValue::Rect(r) => {
                format!("({}, {}, {}, {})", r.left, r.top, r.right, r.bottom)
            }
            ParameterValue::Enum { value, .. } => value.to_string(),
            ParameterValue::Script(s) => format!("{}({})", s.name, s.parameters),
            ParameterValue::Separator => "<separator>".to_string(),
            ParameterValue::StringsDbId(v) => v.to_string(),
            _ => format!("{:?}", self),
        }
    }
}

/// Range validation for numeric types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Range {
    IntRange { min: i32, max: i32 },
    FloatRange { min: f32, max: f32 },
}

impl Range {
    pub fn validate_int(&self, value: i32) -> ParameterResult<()> {
        match self {
            Range::IntRange { min, max } => {
                if value >= *min && value <= *max {
                    Ok(())
                } else {
                    Err(ParameterError::ValueOutOfRange {
                        value: value.to_string(),
                        min: min.to_string(),
                        max: max.to_string(),
                    })
                }
            }
            _ => Err(ParameterError::TypeMismatch {
                expected: ParameterType::Int,
                found: ParameterType::Float,
            }),
        }
    }

    pub fn validate_float(&self, value: f32) -> ParameterResult<()> {
        match self {
            Range::FloatRange { min, max } => {
                if value >= *min && value <= *max {
                    Ok(())
                } else {
                    Err(ParameterError::ValueOutOfRange {
                        value: value.to_string(),
                        min: min.to_string(),
                        max: max.to_string(),
                    })
                }
            }
            _ => Err(ParameterError::TypeMismatch {
                expected: ParameterType::Float,
                found: ParameterType::Int,
            }),
        }
    }
}

/// Individual parameter with metadata and validation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    /// Parameter name/identifier
    pub name: String,
    /// Current parameter value
    pub value: ParameterValue,
    /// Whether the parameter has been modified since creation
    pub is_modified: bool,
    /// Display units (e.g., "meters", "seconds")
    pub units: Option<String>,
    /// Range validation for numeric types
    pub range: Option<Range>,
    /// File extension for filename parameters
    pub file_extension: Option<String>,
    /// Description for filename parameters
    pub file_description: Option<String>,
}

impl Parameter {
    /// Create a new parameter with the given name and value
    pub fn new(name: String, value: ParameterValue) -> ParameterResult<Self> {
        if name.is_empty() {
            return Err(ParameterError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        Ok(Parameter {
            name,
            value,
            is_modified: false,
            units: None,
            range: None,
            file_extension: None,
            file_description: None,
        })
    }

    /// Create a new integer parameter with optional range validation
    pub fn new_int(name: &str, value: i32, range: Option<(i32, i32)>) -> ParameterResult<Self> {
        let mut param = Self::new(name.to_string(), ParameterValue::Int(value))?;
        if let Some((min, max)) = range {
            param.range = Some(Range::IntRange { min, max });
            param.range.as_ref().unwrap().validate_int(value)?;
        }
        Ok(param)
    }

    /// Create a new float parameter with optional range validation
    pub fn new_float(name: &str, value: f32, range: Option<(f32, f32)>) -> ParameterResult<Self> {
        let mut param = Self::new(name.to_string(), ParameterValue::Float(value))?;
        if let Some((min, max)) = range {
            param.range = Some(Range::FloatRange { min, max });
            param.range.as_ref().unwrap().validate_float(value)?;
        }
        Ok(param)
    }

    /// Create a new string parameter
    pub fn new_string(name: &str, value: String) -> ParameterResult<Self> {
        Self::new(name.to_string(), ParameterValue::String(value))
    }

    /// Create a new boolean parameter
    pub fn new_bool(name: &str, value: bool) -> ParameterResult<Self> {
        Self::new(name.to_string(), ParameterValue::Bool(value))
    }

    /// Create a new angle parameter (in radians)
    pub fn new_angle(name: &str, value: f32) -> ParameterResult<Self> {
        let mut param = Self::new(name.to_string(), ParameterValue::Angle(value))?;
        // Default angle range: 0 to 2π
        param.range = Some(Range::FloatRange {
            min: 0.0,
            max: 2.0 * std::f32::consts::PI,
        });
        param.range.as_ref().unwrap().validate_float(value)?;
        Ok(param)
    }

    /// Create a new filename parameter
    pub fn new_filename(
        name: &str,
        value: String,
        extension: Option<String>,
        description: Option<String>,
    ) -> ParameterResult<Self> {
        let mut param = Self::new(name.to_string(), ParameterValue::Filename(value))?;
        param.file_extension = extension;
        param.file_description = description;
        Ok(param)
    }

    /// Create a new enum parameter
    pub fn new_enum(name: &str, value: i32, options: Vec<EnumValue>) -> ParameterResult<Self> {
        // Validate that the value exists in the options
        let valid_values: Vec<i32> = options.iter().map(|opt| opt.value).collect();
        if !valid_values.contains(&value) {
            return Err(ParameterError::InvalidEnumValue {
                value,
                valid_values,
            });
        }

        Self::new(name.to_string(), ParameterValue::Enum { value, options })
    }

    /// Create a new vector2 parameter
    pub fn new_vector2(name: &str, x: f32, y: f32) -> ParameterResult<Self> {
        Self::new(
            name.to_string(),
            ParameterValue::Vector2(Vector2::new(x, y)),
        )
    }

    /// Create a new vector3 parameter
    pub fn new_vector3(name: &str, x: f32, y: f32, z: f32) -> ParameterResult<Self> {
        Self::new(
            name.to_string(),
            ParameterValue::Vector3(Vector3::new(x, y, z)),
        )
    }

    /// Create a new color parameter
    pub fn new_color(name: &str, r: f32, g: f32, b: f32) -> ParameterResult<Self> {
        Self::new(
            name.to_string(),
            ParameterValue::Color(Vector3::new(r, g, b)),
        )
    }

    /// Create a separator parameter for UI organization
    pub fn new_separator(name: &str) -> ParameterResult<Self> {
        Self::new(name.to_string(), ParameterValue::Separator)
    }

    /// Set the parameter value with validation
    pub fn set_value(&mut self, new_value: ParameterValue) -> ParameterResult<()> {
        // Type checking
        if std::mem::discriminant(&self.value) != std::mem::discriminant(&new_value) {
            return Err(ParameterError::TypeMismatch {
                expected: self.value.get_type(),
                found: new_value.get_type(),
            });
        }

        // Range validation for numeric types
        match (&new_value, &self.range) {
            (ParameterValue::Int(value), Some(range)) => {
                range.validate_int(*value)?;
            }
            (ParameterValue::Float(value), Some(range)) => {
                range.validate_float(*value)?;
            }
            (ParameterValue::Angle(value), Some(range)) => {
                range.validate_float(*value)?;
            }
            _ => {} // No validation needed
        }

        // Enum validation
        if let ParameterValue::Enum { value, options } = &new_value {
            let valid_values: Vec<i32> = options.iter().map(|opt| opt.value).collect();
            if !valid_values.contains(value) {
                return Err(ParameterError::InvalidEnumValue {
                    value: *value,
                    valid_values,
                });
            }
        }

        self.value = new_value;
        self.is_modified = true;
        Ok(())
    }

    /// Get the parameter value
    pub fn get_value(&self) -> &ParameterValue {
        &self.value
    }

    /// Get the parameter type
    pub fn get_type(&self) -> ParameterType {
        self.value.get_type()
    }

    /// Check if the parameter is of a specific type
    pub fn is_type(&self, param_type: ParameterType) -> bool {
        self.get_type() == param_type
    }

    /// Set the units display string
    pub fn set_units(&mut self, units: Option<String>) {
        self.units = units;
    }

    /// Get the units display string
    pub fn get_units(&self) -> Option<&String> {
        self.units.as_ref()
    }

    /// Mark parameter as modified
    pub fn set_modified(&mut self, modified: bool) {
        self.is_modified = modified;
    }

    /// Check if parameter is modified
    pub fn is_modified(&self) -> bool {
        self.is_modified
    }

    /// Copy value from another parameter of compatible type
    pub fn copy_value_from(&mut self, other: &Parameter) -> ParameterResult<()> {
        if self.get_type() != other.get_type() {
            return Err(ParameterError::TypeMismatch {
                expected: self.get_type(),
                found: other.get_type(),
            });
        }

        self.set_value(other.value.clone())
    }

    /// Validate the current parameter value
    pub fn validate(&self) -> ParameterResult<()> {
        match (&self.value, &self.range) {
            (ParameterValue::Int(value), Some(range)) => range.validate_int(*value),
            (ParameterValue::Float(value), Some(range)) => range.validate_float(*value),
            (ParameterValue::Angle(value), Some(range)) => range.validate_float(*value),
            (ParameterValue::Enum { value, options }, _) => {
                let valid_values: Vec<i32> = options.iter().map(|opt| opt.value).collect();
                if valid_values.contains(value) {
                    Ok(())
                } else {
                    Err(ParameterError::InvalidEnumValue {
                        value: *value,
                        valid_values,
                    })
                }
            }
            _ => Ok(()),
        }
    }
}

/// List of parameters with management functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterList {
    /// Parameters stored by name for fast lookup
    parameters: HashMap<String, Parameter>,
    /// Ordered list of parameter names to maintain insertion order
    order: Vec<String>,
}

impl ParameterList {
    /// Create a new empty parameter list
    pub fn new() -> Self {
        Self {
            parameters: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Add a parameter to the list
    pub fn add(&mut self, parameter: Parameter) -> ParameterResult<()> {
        if self.parameters.contains_key(&parameter.name) {
            return Err(ParameterError::Generic(format!(
                "Parameter '{}' already exists",
                parameter.name
            )));
        }

        self.order.push(parameter.name.clone());
        self.parameters.insert(parameter.name.clone(), parameter);
        Ok(())
    }

    /// Add a parameter with automatic construction
    pub fn add_typed(
        &mut self,
        name: &str,
        _param_type: ParameterType,
        initial_value: ParameterValue,
    ) -> ParameterResult<()> {
        let parameter = Parameter::new(name.to_string(), initial_value)?;
        self.add(parameter)
    }

    /// Get a parameter by name
    pub fn get(&self, name: &str) -> Option<&Parameter> {
        self.parameters.get(name)
    }

    /// Get a mutable reference to a parameter by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Parameter> {
        self.parameters.get_mut(name)
    }

    /// Remove a parameter by name
    pub fn remove(&mut self, name: &str) -> Option<Parameter> {
        if let Some(param) = self.parameters.remove(name) {
            self.order.retain(|n| n != name);
            Some(param)
        } else {
            None
        }
    }

    /// Get the number of parameters
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }

    /// Get all parameter names in insertion order
    pub fn names(&self) -> &[String] {
        &self.order
    }

    /// Get an iterator over parameters in insertion order
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Parameter)> {
        self.order
            .iter()
            .filter_map(move |name| self.parameters.get(name).map(|param| (name, param)))
    }

    /// Get parameter names that can be used to access parameters mutably
    pub fn parameter_names(&self) -> Vec<&String> {
        self.order.iter().collect()
    }

    /// Clear all parameters
    pub fn clear(&mut self) {
        self.parameters.clear();
        self.order.clear();
    }

    /// Validate all parameters
    pub fn validate_all(&self) -> ParameterResult<()> {
        for (name, param) in &self.parameters {
            param.validate().map_err(|e| {
                ParameterError::Generic(format!(
                    "Validation failed for parameter '{}': {}",
                    name, e
                ))
            })?;
        }
        Ok(())
    }

    /// Get all modified parameters
    pub fn get_modified(&self) -> Vec<&Parameter> {
        self.parameters
            .values()
            .filter(|param| param.is_modified)
            .collect()
    }

    /// Mark all parameters as unmodified
    pub fn mark_all_unmodified(&mut self) {
        for param in self.parameters.values_mut() {
            param.set_modified(false);
        }
    }

    /// Copy values from another parameter list for matching names and types
    pub fn copy_compatible_values(&mut self, other: &ParameterList) -> Vec<ParameterError> {
        let mut errors = Vec::new();

        for (name, other_param) in &other.parameters {
            if let Some(our_param) = self.parameters.get_mut(name) {
                if let Err(e) = our_param.copy_value_from(other_param) {
                    errors.push(e);
                }
            }
        }

        errors
    }
}

impl Default for ParameterList {
    fn default() -> Self {
        Self::new()
    }
}

/// Parameter factory for creating parameters from raw data
pub struct ParameterFactory;

impl ParameterFactory {
    /// Construct a parameter from type and raw data (similar to C++ virtual constructor)
    pub fn construct(
        param_type: ParameterType,
        name: &str,
        data: &str,
    ) -> ParameterResult<Parameter> {
        match param_type {
            ParameterType::Int => {
                let value: i32 = data.parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid int: {}", e))
                })?;
                Parameter::new_int(name, value, None)
            }
            ParameterType::Float => {
                let value: f32 = data.parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid float: {}", e))
                })?;
                Parameter::new_float(name, value, None)
            }
            ParameterType::String => Parameter::new_string(name, data.to_string()),
            ParameterType::Bool => {
                let value: bool = data.parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid bool: {}", e))
                })?;
                Parameter::new_bool(name, value)
            }
            ParameterType::Angle => {
                let value: f32 = data.parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid angle: {}", e))
                })?;
                Parameter::new_angle(name, value)
            }
            ParameterType::Filename => Parameter::new_filename(name, data.to_string(), None, None),
            ParameterType::Vector2 => {
                let parts: Vec<&str> = data
                    .trim_matches(|c| c == '(' || c == ')')
                    .split(',')
                    .collect();
                if parts.len() != 2 {
                    return Err(ParameterError::InvalidStringFormat(
                        "Vector2 must have 2 components".to_string(),
                    ));
                }
                let x: f32 = parts[0].trim().parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid Vector2 x: {}", e))
                })?;
                let y: f32 = parts[1].trim().parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid Vector2 y: {}", e))
                })?;
                Parameter::new_vector2(name, x, y)
            }
            ParameterType::Vector3 => {
                let parts: Vec<&str> = data
                    .trim_matches(|c| c == '(' || c == ')')
                    .split(',')
                    .collect();
                if parts.len() != 3 {
                    return Err(ParameterError::InvalidStringFormat(
                        "Vector3 must have 3 components".to_string(),
                    ));
                }
                let x: f32 = parts[0].trim().parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid Vector3 x: {}", e))
                })?;
                let y: f32 = parts[1].trim().parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid Vector3 y: {}", e))
                })?;
                let z: f32 = parts[2].trim().parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid Vector3 z: {}", e))
                })?;
                Parameter::new_vector3(name, x, y, z)
            }
            ParameterType::Color => {
                let parts: Vec<&str> = data
                    .trim_matches(|c| c == '(' || c == ')')
                    .split(',')
                    .collect();
                if parts.len() != 3 {
                    return Err(ParameterError::InvalidStringFormat(
                        "Color must have 3 components (RGB)".to_string(),
                    ));
                }
                let r: f32 = parts[0].trim().parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid Color R: {}", e))
                })?;
                let g: f32 = parts[1].trim().parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid Color G: {}", e))
                })?;
                let b: f32 = parts[2].trim().parse().map_err(|e| {
                    ParameterError::InvalidStringFormat(format!("Invalid Color B: {}", e))
                })?;
                Parameter::new_color(name, r, g, b)
            }
            ParameterType::Separator => Parameter::new_separator(name),
            _ => {
                // For complex types, create with default values
                let value = match param_type {
                    ParameterType::Matrix3D => ParameterValue::Matrix3D(Matrix3D::identity()),
                    ParameterType::Rect => ParameterValue::Rect(Rect::new(0.0, 0.0, 0.0, 0.0)),
                    ParameterType::Zone => ParameterValue::Zone(OBBox::new(
                        Vector3::zero(),
                        Vector3::zero(),
                        Matrix3D::identity(),
                    )),
                    ParameterType::Script => {
                        ParameterValue::Script(Script::new(String::new(), String::new()))
                    }
                    ParameterType::TextureFilename => ParameterValue::TextureFilename {
                        path: data.to_string(),
                        show_alpha: false,
                        show_texture: false,
                    },
                    ParameterType::SoundFilename => ParameterValue::SoundFilename(data.to_string()),
                    ParameterType::FilenameList => ParameterValue::FilenameList(Vec::new()),
                    ParameterType::ScriptList => ParameterValue::ScriptList {
                        names: Vec::new(),
                        parameters: Vec::new(),
                    },
                    ParameterType::DefinitionIdList => ParameterValue::DefinitionIdList {
                        values: Vec::new(),
                        class_id: 0,
                        selected_class_id: None,
                    },
                    ParameterType::StringsDbId => {
                        let value: i32 = data.parse().unwrap_or(0);
                        ParameterValue::StringsDbId(value)
                    }
                    _ => {
                        // ID types default to 0
                        let value: i32 = data.parse().unwrap_or(0);
                        match param_type {
                            ParameterType::ModelDefinitionId => {
                                ParameterValue::ModelDefinitionId(value)
                            }
                            ParameterType::GameObjDefinitionId => {
                                ParameterValue::GameObjDefinitionId {
                                    value,
                                    base_class: None,
                                }
                            }
                            ParameterType::WeaponObjDefinitionId => {
                                ParameterValue::WeaponObjDefinitionId {
                                    value,
                                    base_class: None,
                                }
                            }
                            ParameterType::AmmoObjDefinitionId => {
                                ParameterValue::AmmoObjDefinitionId {
                                    value,
                                    base_class: None,
                                }
                            }
                            ParameterType::SoundDefinitionId => {
                                ParameterValue::SoundDefinitionId(value)
                            }
                            ParameterType::PhysDefinitionId => ParameterValue::PhysDefinitionId {
                                value,
                                base_class: None,
                            },
                            ParameterType::ExplosionDefinitionId => {
                                ParameterValue::ExplosionDefinitionId {
                                    value,
                                    base_class: None,
                                }
                            }
                            ParameterType::GenericDefinitionId => {
                                ParameterValue::GenericDefinitionId { value, class_id: 0 }
                            }
                            ParameterType::Transition => ParameterValue::Transition(value),
                            _ => {
                                return Err(ParameterError::Generic(format!(
                                    "Unsupported parameter type: {:?}",
                                    param_type
                                )))
                            }
                        }
                    }
                };
                Parameter::new(name.to_string(), value)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_int_parameter() {
        let param = Parameter::new_int("health", 100, Some((0, 1000))).unwrap();
        assert_eq!(param.name, "health");
        assert_eq!(param.get_value(), &ParameterValue::Int(100));
        assert_eq!(param.get_type(), ParameterType::Int);
        assert!(!param.is_modified());
    }

    #[test]
    fn test_create_string_parameter() {
        let param = Parameter::new_string("name", "Player".to_string()).unwrap();
        assert_eq!(param.name, "name");
        assert_eq!(
            param.get_value(),
            &ParameterValue::String("Player".to_string())
        );
        assert_eq!(param.get_type(), ParameterType::String);
    }

    #[test]
    fn test_parameter_range_validation() {
        let mut param = Parameter::new_int("level", 5, Some((1, 10))).unwrap();

        // Valid value
        assert!(param.set_value(ParameterValue::Int(8)).is_ok());
        assert!(param.is_modified());

        // Invalid value (out of range)
        assert!(param.set_value(ParameterValue::Int(15)).is_err());
    }

    #[test]
    fn test_parameter_type_validation() {
        let mut int_param = Parameter::new_int("test", 5, None).unwrap();

        // Wrong type should fail
        let result = int_param.set_value(ParameterValue::String("hello".to_string()));
        assert!(result.is_err());

        if let Err(ParameterError::TypeMismatch { expected, found }) = result {
            assert_eq!(expected, ParameterType::Int);
            assert_eq!(found, ParameterType::String);
        } else {
            panic!("Expected type mismatch error");
        }
    }

    #[test]
    fn test_enum_parameter() {
        let options = vec![
            EnumValue::new("Low".to_string(), 0),
            EnumValue::new("Medium".to_string(), 1),
            EnumValue::new("High".to_string(), 2),
        ];

        let param = Parameter::new_enum("quality", 1, options.clone()).unwrap();
        assert_eq!(param.get_type(), ParameterType::Enum);

        if let ParameterValue::Enum { value, .. } = param.get_value() {
            assert_eq!(*value, 1);
        } else {
            panic!("Expected enum value");
        }

        // Test invalid enum value
        let result = Parameter::new_enum("quality", 5, options);
        assert!(result.is_err());
    }

    #[test]
    fn test_parameter_list() {
        let mut list = ParameterList::new();

        let int_param = Parameter::new_int("health", 100, None).unwrap();
        let str_param = Parameter::new_string("name", "Player".to_string()).unwrap();

        assert!(list.add(int_param).is_ok());
        assert!(list.add(str_param).is_ok());

        assert_eq!(list.len(), 2);
        assert!(list.get("health").is_some());
        assert!(list.get("name").is_some());
        assert!(list.get("nonexistent").is_none());

        // Test duplicate name
        let duplicate = Parameter::new_int("health", 200, None).unwrap();
        assert!(list.add(duplicate).is_err());
    }

    #[test]
    fn test_parameter_factory() {
        let int_param = ParameterFactory::construct(ParameterType::Int, "test", "42").unwrap();
        assert_eq!(int_param.get_type(), ParameterType::Int);
        assert_eq!(int_param.get_value(), &ParameterValue::Int(42));

        let float_param =
            ParameterFactory::construct(ParameterType::Float, "test", "3.14").unwrap();
        assert_eq!(float_param.get_type(), ParameterType::Float);

        let bool_param = ParameterFactory::construct(ParameterType::Bool, "test", "true").unwrap();
        assert_eq!(bool_param.get_value(), &ParameterValue::Bool(true));

        let vector2_param =
            ParameterFactory::construct(ParameterType::Vector2, "test", "(1.0, 2.0)").unwrap();
        if let ParameterValue::Vector2(v) = vector2_param.get_value() {
            assert_eq!(v.x, 1.0);
            assert_eq!(v.y, 2.0);
        } else {
            panic!("Expected Vector2");
        }
    }

    #[test]
    fn test_copy_value_between_parameters() {
        let source = Parameter::new_int("source", 42, None).unwrap();
        let mut target = Parameter::new_int("target", 0, None).unwrap();

        assert!(target.copy_value_from(&source).is_ok());
        assert_eq!(target.get_value(), &ParameterValue::Int(42));
        assert!(target.is_modified());

        // Test type mismatch
        let string_param = Parameter::new_string("str", "hello".to_string()).unwrap();
        assert!(target.copy_value_from(&string_param).is_err());
    }

    #[test]
    fn test_angle_parameter() {
        let angle = Parameter::new_angle("rotation", std::f32::consts::PI).unwrap();
        assert_eq!(angle.get_type(), ParameterType::Angle);

        if let ParameterValue::Angle(value) = angle.get_value() {
            assert_eq!(*value, std::f32::consts::PI);
        } else {
            panic!("Expected angle value");
        }

        // Test invalid angle (outside 0-2π range)
        let result = Parameter::new_angle("rotation", -1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_vector_parameters() {
        let vec2 = Parameter::new_vector2("position", 1.0, 2.0).unwrap();
        if let ParameterValue::Vector2(v) = vec2.get_value() {
            assert_eq!(v.x, 1.0);
            assert_eq!(v.y, 2.0);
        }

        let vec3 = Parameter::new_vector3("position3d", 1.0, 2.0, 3.0).unwrap();
        if let ParameterValue::Vector3(v) = vec3.get_value() {
            assert_eq!(v.x, 1.0);
            assert_eq!(v.y, 2.0);
            assert_eq!(v.z, 3.0);
        }

        let color = Parameter::new_color("tint", 0.5, 0.7, 0.9).unwrap();
        if let ParameterValue::Color(c) = color.get_value() {
            assert_eq!(c.x, 0.5);
            assert_eq!(c.y, 0.7);
            assert_eq!(c.z, 0.9);
        }
    }

    #[test]
    fn test_parameter_validation() {
        let mut list = ParameterList::new();

        let valid_param = Parameter::new_int("valid", 5, Some((0, 10))).unwrap();
        list.add(valid_param).unwrap();

        // Should pass validation
        assert!(list.validate_all().is_ok());

        // Manually create invalid parameter for testing
        let mut invalid_param = Parameter::new_int("invalid", 5, Some((0, 10))).unwrap();
        invalid_param.value = ParameterValue::Int(15); // Out of range
        list.add(invalid_param).unwrap();

        // Should fail validation
        assert!(list.validate_all().is_err());
    }

    #[test]
    fn test_parameter_serialization() {
        let param = Parameter::new_int("test", 42, Some((0, 100))).unwrap();

        // Test serialization
        let serialized = serde_json::to_string(&param).unwrap();
        assert!(serialized.contains("test"));
        assert!(serialized.contains("42"));

        // Test deserialization
        let deserialized: Parameter = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.get_value(), &ParameterValue::Int(42));
    }
}
