//! Stealth & Detection System Input Validation Module
//!
//! Comprehensive parameter validation before use in stealth and detection calculations.
//! Ensures all inputs are within valid ranges and constraints before processing.

use crate::common::{ObjectID, INVALID_ID};
use std::fmt;

/// Maximum player ID.
const MAX_PLAYER_ID: u32 = (crate::common::MAX_PLAYER_COUNT as u32) - 1;

/// Maximum string length for template/upgrade names
const MAX_NAME_LENGTH: usize = 255;

/// Valid KindOf mask bits
const VALID_KINDOF_BITS: u32 = 0xFFFFFFFF;

/// Validation error details
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Type of validation failure
    pub error_type: ValidationErrorType,
    /// Detailed failure message
    pub message: String,
    /// Optional field/parameter name
    pub field: Option<String>,
}

/// Types of validation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationErrorType {
    /// Invalid object ID
    InvalidObjectId,
    /// Invalid player ID
    InvalidPlayerId,
    /// Invalid numeric range
    InvalidRange,
    /// Invalid string format
    InvalidString,
    /// Invalid condition flags
    InvalidFlags,
    /// Invalid complex type
    InvalidType,
    /// Batch validation failure
    BatchError,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.field {
            Some(field) => write!(
                f,
                "{} - field '{}': {}",
                self.error_type_str(),
                field,
                self.message
            ),
            None => write!(f, "{} - {}", self.error_type_str(), self.message),
        }
    }
}

impl ValidationError {
    /// Get string representation of error type
    fn error_type_str(&self) -> &str {
        match self.error_type {
            ValidationErrorType::InvalidObjectId => "InvalidObjectId",
            ValidationErrorType::InvalidPlayerId => "InvalidPlayerId",
            ValidationErrorType::InvalidRange => "InvalidRange",
            ValidationErrorType::InvalidString => "InvalidString",
            ValidationErrorType::InvalidFlags => "InvalidFlags",
            ValidationErrorType::InvalidType => "InvalidType",
            ValidationErrorType::BatchError => "BatchError",
        }
    }

    /// Create object ID error
    pub fn object_id(id: ObjectID, reason: &str) -> Self {
        Self {
            error_type: ValidationErrorType::InvalidObjectId,
            message: format!("ObjectID {} {}", id, reason),
            field: None,
        }
    }

    /// Create player ID error
    pub fn player_id(id: u32, reason: &str) -> Self {
        Self {
            error_type: ValidationErrorType::InvalidPlayerId,
            message: format!("PlayerID {} {}", id, reason),
            field: None,
        }
    }

    /// Create range error
    pub fn range(value: f32, min: f32, max: f32) -> Self {
        Self {
            error_type: ValidationErrorType::InvalidRange,
            message: format!("value {} not in range [{}, {}]", value, min, max),
            field: None,
        }
    }

    /// Create string error
    pub fn string(reason: &str) -> Self {
        Self {
            error_type: ValidationErrorType::InvalidString,
            message: reason.to_string(),
            field: None,
        }
    }

    /// Create batch error
    pub fn batch_error(reason: &str) -> Self {
        Self {
            error_type: ValidationErrorType::BatchError,
            message: reason.to_string(),
            field: None,
        }
    }

    /// Create flags error
    pub fn flags(reason: &str) -> Self {
        Self {
            error_type: ValidationErrorType::InvalidFlags,
            message: reason.to_string(),
            field: None,
        }
    }

    /// Create type error
    pub fn type_error(reason: &str) -> Self {
        Self {
            error_type: ValidationErrorType::InvalidType,
            message: reason.to_string(),
            field: None,
        }
    }

    /// Add field name for context
    pub fn with_field(mut self, field: &str) -> Self {
        self.field = Some(field.to_string());
        self
    }
}

/// Comprehensive validator for stealth and detection system inputs
pub struct StealthValidator;

impl StealthValidator {
    /// Validate ObjectID (must be non-zero)
    pub fn validate_object_id(id: ObjectID) -> Result<ObjectID, ValidationError> {
        if id == INVALID_ID {
            return Err(ValidationError::object_id(id, "is invalid/null"));
        }
        Ok(id)
    }

    /// Validate PlayerID (must be 0-7)
    pub fn validate_player_id(id: u32) -> Result<u32, ValidationError> {
        if id > MAX_PLAYER_ID {
            return Err(ValidationError::player_id(
                id,
                &format!("exceeds maximum {}", MAX_PLAYER_ID),
            ));
        }
        Ok(id)
    }

    /// Validate condition flags (u32 bitfield)
    pub fn validate_condition_flags(flags: u32) -> Result<u32, ValidationError> {
        // All u32 values are currently accepted for condition flags.
        Ok(flags)
    }

    /// Validate stealth strength (0.0-100.0 range)
    pub fn validate_stealth_strength(value: f32) -> Result<f32, ValidationError> {
        if !value.is_finite() {
            return Err(ValidationError::range(value, 0.0, 100.0).with_field("stealth_strength"));
        }
        if value < 0.0 || value > 100.0 {
            return Err(ValidationError::range(value, 0.0, 100.0).with_field("stealth_strength"));
        }
        Ok(value)
    }

    /// Validate detection strength (0.0-100.0 range)
    pub fn validate_detection_strength(value: f32) -> Result<f32, ValidationError> {
        if !value.is_finite() {
            return Err(ValidationError::range(value, 0.0, 100.0).with_field("detection_strength"));
        }
        if value < 0.0 || value > 100.0 {
            return Err(ValidationError::range(value, 0.0, 100.0).with_field("detection_strength"));
        }
        Ok(value)
    }

    /// Validate opacity (0.0-1.0 range)
    pub fn validate_opacity(value: f32) -> Result<f32, ValidationError> {
        if !value.is_finite() {
            return Err(ValidationError::range(value, 0.0, 1.0).with_field("opacity"));
        }
        if value < 0.0 || value > 1.0 {
            return Err(ValidationError::range(value, 0.0, 1.0).with_field("opacity"));
        }
        Ok(value)
    }

    /// Validate distance (must be non-negative)
    pub fn validate_distance(value: f32) -> Result<f32, ValidationError> {
        if !value.is_finite() {
            return Err(ValidationError::range(value, 0.0, f32::MAX).with_field("distance"));
        }
        if value < 0.0 {
            return Err(
                ValidationError::string("distance must be non-negative").with_field("distance")
            );
        }
        Ok(value)
    }

    /// Validate frame count (must be non-negative)
    pub fn validate_frames(value: u32) -> Result<u32, ValidationError> {
        // All u32 values are non-negative by definition
        Ok(value)
    }

    /// Validate radius (must be positive)
    pub fn validate_radius(value: f32) -> Result<f32, ValidationError> {
        if !value.is_finite() {
            return Err(ValidationError::range(value, 0.0, f32::MAX).with_field("radius"));
        }
        if value <= 0.0 {
            return Err(
                ValidationError::string("radius must be positive (> 0)").with_field("radius")
            );
        }
        Ok(value)
    }

    /// Validate template name (non-empty, max length)
    pub fn validate_template_name(name: &str) -> Result<String, ValidationError> {
        if name.is_empty() {
            return Err(ValidationError::string("template name cannot be empty")
                .with_field("template_name"));
        }
        if name.len() > MAX_NAME_LENGTH {
            return Err(ValidationError::string(&format!(
                "template name exceeds maximum length of {}",
                MAX_NAME_LENGTH
            ))
            .with_field("template_name"));
        }
        Ok(name.to_string())
    }

    /// Validate upgrade name (non-empty, max length)
    pub fn validate_upgrade_name(name: &str) -> Result<String, ValidationError> {
        if name.is_empty() {
            return Err(
                ValidationError::string("upgrade name cannot be empty").with_field("upgrade_name")
            );
        }
        if name.len() > MAX_NAME_LENGTH {
            return Err(ValidationError::string(&format!(
                "upgrade name exceeds maximum length of {}",
                MAX_NAME_LENGTH
            ))
            .with_field("upgrade_name"));
        }
        Ok(name.to_string())
    }

    /// Validate KindOf mask
    pub fn validate_kindof_mask(mask: u32) -> Result<u32, ValidationError> {
        // All u32 values are valid kindof masks
        // Validate against VALID_KINDOF_BITS if constraints exist
        if (mask & VALID_KINDOF_BITS) != mask {
            return Err(
                ValidationError::flags("invalid kindof mask bits").with_field("kindof_mask")
            );
        }
        Ok(mask)
    }

    /// Validate detection modifier structure
    pub fn validate_detection_modifier(
        distance_mod: f32,
        movement_mod: f32,
        unit_type_mod: f32,
        rider_mod: f32,
        los_mod: f32,
        garrisoned_mod: f32,
    ) -> Result<(), ValidationError> {
        // All modifiers should be positive finite values
        for (name, value) in &[
            ("distance_modifier", distance_mod),
            ("movement_modifier", movement_mod),
            ("unit_type_modifier", unit_type_mod),
            ("rider_modifier", rider_mod),
            ("los_modifier", los_mod),
            ("garrisoned_modifier", garrisoned_mod),
        ] {
            if !value.is_finite() {
                return Err(ValidationError::range(*value, 0.0, f32::MAX).with_field(name));
            }
            if *value < 0.0 {
                return Err(
                    ValidationError::string("modifier must be non-negative").with_field(name)
                );
            }
        }
        Ok(())
    }

    /// Validate disguise state (0-2 range typically)
    pub fn validate_disguise_state(state: u8) -> Result<u8, ValidationError> {
        if state > 2 {
            return Err(
                ValidationError::type_error(&format!("invalid disguise state: {}", state))
                    .with_field("disguise_state"),
            );
        }
        Ok(state)
    }

    /// Validate stealth grant (0-1 boolean-like value)
    pub fn validate_stealth_grant(grant: bool) -> Result<bool, ValidationError> {
        Ok(grant)
    }

    /// Validate special power parameters structure
    pub fn validate_special_power_params(
        frames: u32,
        strength: f32,
        radius: f32,
    ) -> Result<(), ValidationError> {
        Self::validate_frames(frames)?;
        Self::validate_stealth_strength(strength)?;
        Self::validate_radius(radius)?;
        Ok(())
    }

    /// Batch validate multiple ObjectIDs
    pub fn validate_object_ids(ids: &[ObjectID]) -> Result<Vec<ObjectID>, ValidationError> {
        let mut valid_ids = Vec::with_capacity(ids.len());
        for (idx, id) in ids.iter().enumerate() {
            match Self::validate_object_id(*id) {
                Ok(id) => valid_ids.push(id),
                Err(e) => {
                    return Err(ValidationError::batch_error(&format!(
                        "batch validation failed at index {}: {}",
                        idx, e.message
                    ))
                    .with_field("object_ids"));
                }
            }
        }
        Ok(valid_ids)
    }

    /// Comprehensive configuration validation
    pub fn validate_all(
        stealth_strength: f32,
        detection_strength: f32,
        opacity: f32,
        distance: f32,
        radius: f32,
        player_id: u32,
        object_id: ObjectID,
    ) -> Result<(), ValidationError> {
        Self::validate_object_id(object_id)?;
        Self::validate_player_id(player_id)?;
        Self::validate_stealth_strength(stealth_strength)?;
        Self::validate_detection_strength(detection_strength)?;
        Self::validate_opacity(opacity)?;
        Self::validate_distance(distance)?;
        Self::validate_radius(radius)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_object_id_valid() {
        assert_eq!(StealthValidator::validate_object_id(1), Ok(1));
        assert_eq!(StealthValidator::validate_object_id(42), Ok(42));
        assert_eq!(
            StealthValidator::validate_object_id(0xFFFFFFFF),
            Ok(0xFFFFFFFF)
        );
    }

    #[test]
    fn test_validate_object_id_invalid() {
        let result = StealthValidator::validate_object_id(INVALID_ID);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error_type, ValidationErrorType::InvalidObjectId);
    }

    #[test]
    fn test_validate_player_id_valid() {
        for id in 0..=MAX_PLAYER_ID {
            assert_eq!(StealthValidator::validate_player_id(id), Ok(id));
        }
    }

    #[test]
    fn test_validate_player_id_invalid() {
        let result = StealthValidator::validate_player_id(crate::common::MAX_PLAYER_COUNT as u32);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error_type, ValidationErrorType::InvalidPlayerId);

        let result = StealthValidator::validate_player_id(100);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_stealth_strength_valid() {
        assert_eq!(StealthValidator::validate_stealth_strength(0.0), Ok(0.0));
        assert_eq!(StealthValidator::validate_stealth_strength(50.0), Ok(50.0));
        assert_eq!(
            StealthValidator::validate_stealth_strength(100.0),
            Ok(100.0)
        );
    }

    #[test]
    fn test_validate_stealth_strength_invalid() {
        let result = StealthValidator::validate_stealth_strength(-1.0);
        assert!(result.is_err());

        let result = StealthValidator::validate_stealth_strength(101.0);
        assert!(result.is_err());

        let result = StealthValidator::validate_stealth_strength(f32::NAN);
        assert!(result.is_err());

        let result = StealthValidator::validate_stealth_strength(f32::INFINITY);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_detection_strength_valid() {
        assert_eq!(StealthValidator::validate_detection_strength(0.0), Ok(0.0));
        assert_eq!(
            StealthValidator::validate_detection_strength(75.0),
            Ok(75.0)
        );
        assert_eq!(
            StealthValidator::validate_detection_strength(100.0),
            Ok(100.0)
        );
    }

    #[test]
    fn test_validate_detection_strength_invalid() {
        let result = StealthValidator::validate_detection_strength(-0.5);
        assert!(result.is_err());

        let result = StealthValidator::validate_detection_strength(100.1);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_opacity_valid() {
        assert_eq!(StealthValidator::validate_opacity(0.0), Ok(0.0));
        assert_eq!(StealthValidator::validate_opacity(0.5), Ok(0.5));
        assert_eq!(StealthValidator::validate_opacity(1.0), Ok(1.0));
    }

    #[test]
    fn test_validate_opacity_invalid() {
        let result = StealthValidator::validate_opacity(-0.1);
        assert!(result.is_err());

        let result = StealthValidator::validate_opacity(1.1);
        assert!(result.is_err());

        let result = StealthValidator::validate_opacity(f32::NAN);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_distance_valid() {
        assert_eq!(StealthValidator::validate_distance(0.0), Ok(0.0));
        assert_eq!(StealthValidator::validate_distance(100.5), Ok(100.5));
        assert_eq!(StealthValidator::validate_distance(1000.0), Ok(1000.0));
    }

    #[test]
    fn test_validate_distance_invalid() {
        let result = StealthValidator::validate_distance(-1.0);
        assert!(result.is_err());

        let result = StealthValidator::validate_distance(f32::NEG_INFINITY);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_frames_valid() {
        assert_eq!(StealthValidator::validate_frames(0), Ok(0));
        assert_eq!(StealthValidator::validate_frames(30), Ok(30));
        assert_eq!(StealthValidator::validate_frames(u32::MAX), Ok(u32::MAX));
    }

    #[test]
    fn test_validate_radius_valid() {
        assert_eq!(StealthValidator::validate_radius(0.1), Ok(0.1));
        assert_eq!(StealthValidator::validate_radius(50.0), Ok(50.0));
        assert_eq!(StealthValidator::validate_radius(500.0), Ok(500.0));
    }

    #[test]
    fn test_validate_radius_invalid() {
        let result = StealthValidator::validate_radius(0.0);
        assert!(result.is_err());

        let result = StealthValidator::validate_radius(-10.0);
        assert!(result.is_err());

        let result = StealthValidator::validate_radius(f32::INFINITY);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_template_name_valid() {
        assert!(StealthValidator::validate_template_name("Tank").is_ok());
        assert!(StealthValidator::validate_template_name("Infantry Unit").is_ok());
        let long_name = "A".repeat(MAX_NAME_LENGTH);
        assert!(StealthValidator::validate_template_name(&long_name).is_ok());
    }

    #[test]
    fn test_validate_template_name_invalid() {
        let result = StealthValidator::validate_template_name("");
        assert!(result.is_err());

        let long_name = "A".repeat(MAX_NAME_LENGTH + 1);
        let result = StealthValidator::validate_template_name(&long_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_upgrade_name_valid() {
        assert!(StealthValidator::validate_upgrade_name("Armor").is_ok());
        assert!(StealthValidator::validate_upgrade_name("Stealth Detection").is_ok());
    }

    #[test]
    fn test_validate_upgrade_name_invalid() {
        let result = StealthValidator::validate_upgrade_name("");
        assert!(result.is_err());

        let long_name = "X".repeat(MAX_NAME_LENGTH + 1);
        let result = StealthValidator::validate_upgrade_name(&long_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_kindof_mask_valid() {
        assert!(StealthValidator::validate_kindof_mask(0).is_ok());
        assert!(StealthValidator::validate_kindof_mask(0xFFFFFFFF).is_ok());
        assert!(StealthValidator::validate_kindof_mask(0x00000001).is_ok());
    }

    #[test]
    fn test_validate_detection_modifier_valid() {
        let result = StealthValidator::validate_detection_modifier(1.0, 0.8, 1.2, 0.9, 1.0, 1.5);
        assert!(result.is_ok());

        let result = StealthValidator::validate_detection_modifier(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_detection_modifier_invalid() {
        let result = StealthValidator::validate_detection_modifier(-0.5, 0.8, 1.2, 0.9, 1.0, 1.5);
        assert!(result.is_err());

        let result =
            StealthValidator::validate_detection_modifier(1.0, f32::NAN, 1.2, 0.9, 1.0, 1.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_disguise_state_valid() {
        assert_eq!(StealthValidator::validate_disguise_state(0), Ok(0));
        assert_eq!(StealthValidator::validate_disguise_state(1), Ok(1));
        assert_eq!(StealthValidator::validate_disguise_state(2), Ok(2));
    }

    #[test]
    fn test_validate_disguise_state_invalid() {
        let result = StealthValidator::validate_disguise_state(3);
        assert!(result.is_err());

        let result = StealthValidator::validate_disguise_state(255);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_stealth_grant_valid() {
        assert_eq!(StealthValidator::validate_stealth_grant(true), Ok(true));
        assert_eq!(StealthValidator::validate_stealth_grant(false), Ok(false));
    }

    #[test]
    fn test_validate_special_power_params_valid() {
        let result = StealthValidator::validate_special_power_params(30, 75.0, 100.0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_special_power_params_invalid() {
        let result = StealthValidator::validate_special_power_params(30, 150.0, 100.0);
        assert!(result.is_err());

        let result = StealthValidator::validate_special_power_params(30, 75.0, -10.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_object_ids_batch_valid() {
        let ids = vec![1, 2, 3, 4, 5];
        let result = StealthValidator::validate_object_ids(&ids);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ids);
    }

    #[test]
    fn test_validate_object_ids_batch_invalid() {
        let ids = vec![1, 2, INVALID_ID, 4, 5];
        let result = StealthValidator::validate_object_ids(&ids);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error_type, ValidationErrorType::BatchError);
    }

    #[test]
    fn test_validate_object_ids_empty() {
        let ids: Vec<ObjectID> = vec![];
        let result = StealthValidator::validate_object_ids(&ids);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_validate_all_valid() {
        let result = StealthValidator::validate_all(50.0, 60.0, 0.8, 100.0, 25.0, 3, 42);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_all_invalid_object_id() {
        let result = StealthValidator::validate_all(50.0, 60.0, 0.8, 100.0, 25.0, 3, INVALID_ID);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_all_invalid_player_id() {
        let result = StealthValidator::validate_all(
            50.0,
            60.0,
            0.8,
            100.0,
            25.0,
            crate::common::MAX_PLAYER_COUNT as u32,
            42,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_all_invalid_stealth_strength() {
        let result = StealthValidator::validate_all(150.0, 60.0, 0.8, 100.0, 25.0, 3, 42);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_all_invalid_radius() {
        let result = StealthValidator::validate_all(50.0, 60.0, 0.8, 100.0, 0.0, 3, 42);
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError::object_id(0, "is invalid");
        assert!(err.to_string().contains("InvalidObjectId"));
        assert!(err.to_string().contains("0"));

        let err_with_field = err.with_field("stealth_id");
        assert!(err_with_field.to_string().contains("stealth_id"));
    }

    #[test]
    fn test_condition_flags_valid() {
        assert!(StealthValidator::validate_condition_flags(0).is_ok());
        assert!(StealthValidator::validate_condition_flags(0xFFFFFFFF).is_ok());
        assert!(StealthValidator::validate_condition_flags(0x12345678).is_ok());
    }

    #[test]
    fn test_edge_case_max_values() {
        assert!(StealthValidator::validate_stealth_strength(100.0).is_ok());
        assert!(StealthValidator::validate_detection_strength(100.0).is_ok());
        assert!(StealthValidator::validate_opacity(1.0).is_ok());
        assert!(StealthValidator::validate_player_id(7).is_ok());
    }

    #[test]
    fn test_edge_case_min_values() {
        assert!(StealthValidator::validate_stealth_strength(0.0).is_ok());
        assert!(StealthValidator::validate_detection_strength(0.0).is_ok());
        assert!(StealthValidator::validate_opacity(0.0).is_ok());
        assert!(StealthValidator::validate_player_id(0).is_ok());
        assert!(StealthValidator::validate_distance(0.0).is_ok());
    }

    #[test]
    fn test_validation_error_types() {
        let err = ValidationError::object_id(0, "invalid");
        assert_eq!(err.error_type, ValidationErrorType::InvalidObjectId);

        let err = ValidationError::player_id(crate::common::MAX_PLAYER_COUNT as u32, "too high");
        assert_eq!(err.error_type, ValidationErrorType::InvalidPlayerId);

        let err = ValidationError::range(150.0, 0.0, 100.0);
        assert_eq!(err.error_type, ValidationErrorType::InvalidRange);

        let err = ValidationError::string("empty");
        assert_eq!(err.error_type, ValidationErrorType::InvalidString);

        let err = ValidationError::flags("bad");
        assert_eq!(err.error_type, ValidationErrorType::InvalidFlags);

        let err = ValidationError::type_error("bad");
        assert_eq!(err.error_type, ValidationErrorType::InvalidType);
    }
}
