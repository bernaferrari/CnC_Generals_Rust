//! Integration tests for the parameter system
//!
//! These tests verify that the parameter system works correctly with the broader
//! save/load system and maintains compatibility with the original C++ implementation.

use ww_save_load::parameter::{
    EnumValue, Matrix3D, OBBox, Parameter, ParameterError, ParameterFactory, ParameterList,
    ParameterType, ParameterValue, Rect, Script, Vector2, Vector3,
};

#[test]
fn test_parameter_list_operations() {
    let mut list = ParameterList::new();

    // Add parameters of different types
    let params = vec![
        Parameter::new_int("health", 100, Some((1, 1000))).unwrap(),
        Parameter::new_float("speed", 5.5, None).unwrap(),
        Parameter::new_string("name", "Test Unit".to_string()).unwrap(),
        Parameter::new_bool("active", true).unwrap(),
        Parameter::new_vector3("position", 10.0, 20.0, 30.0).unwrap(),
    ];

    // Add all parameters
    for param in params {
        list.add(param).unwrap();
    }

    assert_eq!(list.len(), 5);
    assert!(!list.is_empty());

    // Test parameter retrieval
    assert!(list.get("health").is_some());
    assert!(list.get("nonexistent").is_none());

    // Test parameter modification
    {
        let health_param = list.get_mut("health").unwrap();
        health_param.set_value(ParameterValue::Int(150)).unwrap();
        assert!(health_param.is_modified());
    }

    // Verify modification
    if let Some(health_param) = list.get("health") {
        assert_eq!(health_param.get_value(), &ParameterValue::Int(150));
    }

    // Test parameter removal
    let removed = list.remove("speed");
    assert!(removed.is_some());
    assert_eq!(list.len(), 4);
    assert!(list.get("speed").is_none());
}

#[test]
fn test_parameter_validation_comprehensive() {
    let mut list = ParameterList::new();

    // Add parameters with various validation rules
    list.add(Parameter::new_int("limited_int", 50, Some((0, 100))).unwrap())
        .unwrap();
    list.add(Parameter::new_float("limited_float", 2.5, Some((0.0, 10.0))).unwrap())
        .unwrap();
    list.add(Parameter::new_angle("rotation", std::f32::consts::PI).unwrap())
        .unwrap();

    // Initial validation should pass
    assert!(list.validate_all().is_ok());

    // Modify parameters to valid values
    list.get_mut("limited_int")
        .unwrap()
        .set_value(ParameterValue::Int(75))
        .unwrap();
    list.get_mut("limited_float")
        .unwrap()
        .set_value(ParameterValue::Float(7.5))
        .unwrap();

    // Validation should still pass
    assert!(list.validate_all().is_ok());

    // Test that validation catches out-of-range values
    // We can't easily test this without breaking encapsulation, so we'll test individual parameters
    let mut test_param = Parameter::new_int("test", 5, Some((0, 10))).unwrap();

    // Valid range
    assert!(test_param.set_value(ParameterValue::Int(8)).is_ok());
    assert!(test_param.validate().is_ok());

    // Invalid range - this should be caught by set_value
    assert!(test_param.set_value(ParameterValue::Int(15)).is_err());
}

#[test]
fn test_parameter_serialization_roundtrip() {
    let original_list = create_comprehensive_parameter_list();

    // Serialize the entire list
    let serialized = serde_json::to_string(&original_list).unwrap();

    // Deserialize
    let deserialized_list: ParameterList = serde_json::from_str(&serialized).unwrap();

    // Verify counts match
    assert_eq!(original_list.len(), deserialized_list.len());

    // Verify each parameter matches
    for name in original_list.names() {
        let original_param = original_list.get(name).unwrap();
        let deserialized_param = deserialized_list.get(name).unwrap();

        assert_eq!(original_param.name, deserialized_param.name);
        assert_eq!(original_param.get_value(), deserialized_param.get_value());
        assert_eq!(original_param.get_type(), deserialized_param.get_type());
    }
}

#[test]
fn test_parameter_factory_comprehensive() {
    // Test factory with various data formats
    let test_cases = vec![
        (ParameterType::Int, "test_int", "42", true),
        (ParameterType::Float, "test_float", "3.14159", true),
        (ParameterType::Bool, "test_bool_true", "true", true),
        (ParameterType::Bool, "test_bool_false", "false", true),
        (ParameterType::String, "test_string", "Hello World", true),
        (ParameterType::Vector2, "test_vec2", "(1.5, 2.5)", true),
        (ParameterType::Vector3, "test_vec3", "(1.0, 2.0, 3.0)", true),
        (ParameterType::Color, "test_color", "(0.8, 0.6, 0.4)", true),
        (ParameterType::Angle, "test_angle", "1.5708", true), // π/2
        (ParameterType::Separator, "test_separator", "", true),
        // Invalid cases
        (ParameterType::Int, "bad_int", "not_a_number", false),
        (ParameterType::Float, "bad_float", "invalid", false),
        (ParameterType::Bool, "bad_bool", "maybe", false),
        (ParameterType::Vector2, "bad_vec2", "(1.0)", false), // Wrong component count
        (ParameterType::Vector3, "bad_vec3", "(1.0, 2.0)", false), // Wrong component count
    ];

    for (param_type, name, data, should_succeed) in test_cases {
        let result = ParameterFactory::construct(param_type, name, data);

        if should_succeed {
            assert!(
                result.is_ok(),
                "Failed to create {:?} from '{}': {:?}",
                param_type,
                data,
                result.err()
            );
            let param = result.unwrap();
            assert_eq!(param.name, name);
            assert_eq!(param.get_type(), param_type);
        } else {
            assert!(
                result.is_err(),
                "Expected failure for {:?} from '{}' but got success",
                param_type,
                data
            );
        }
    }
}

#[test]
fn test_parameter_copy_compatibility() {
    let mut source_list = ParameterList::new();
    let mut target_list = ParameterList::new();

    // Create source parameters
    source_list
        .add(Parameter::new_int("health", 100, None).unwrap())
        .unwrap();
    source_list
        .add(Parameter::new_float("speed", 5.5, None).unwrap())
        .unwrap();
    source_list
        .add(Parameter::new_string("name", "Source".to_string()).unwrap())
        .unwrap();
    source_list
        .add(Parameter::new_bool("active", true).unwrap())
        .unwrap();

    // Create target parameters with same names but different values
    target_list
        .add(Parameter::new_int("health", 50, None).unwrap())
        .unwrap();
    target_list
        .add(Parameter::new_float("speed", 2.0, None).unwrap())
        .unwrap();
    target_list
        .add(Parameter::new_string("name", "Target".to_string()).unwrap())
        .unwrap();
    target_list
        .add(Parameter::new_bool("active", false).unwrap())
        .unwrap();
    target_list
        .add(Parameter::new_int("extra", 999, None).unwrap())
        .unwrap(); // Extra param

    // Copy compatible values
    let errors = target_list.copy_compatible_values(&source_list);
    assert!(errors.is_empty(), "Unexpected copy errors: {:?}", errors);

    // Verify values were copied
    assert_eq!(
        target_list.get("health").unwrap().get_value(),
        &ParameterValue::Int(100)
    );
    assert_eq!(
        target_list.get("speed").unwrap().get_value(),
        &ParameterValue::Float(5.5)
    );
    assert_eq!(
        target_list.get("name").unwrap().get_value(),
        &ParameterValue::String("Source".to_string())
    );
    assert_eq!(
        target_list.get("active").unwrap().get_value(),
        &ParameterValue::Bool(true)
    );

    // Extra parameter should remain unchanged
    assert_eq!(
        target_list.get("extra").unwrap().get_value(),
        &ParameterValue::Int(999)
    );

    // Verify modified flags are set
    assert!(target_list.get("health").unwrap().is_modified());
    assert!(target_list.get("speed").unwrap().is_modified());
    assert!(target_list.get("name").unwrap().is_modified());
    assert!(target_list.get("active").unwrap().is_modified());
    assert!(!target_list.get("extra").unwrap().is_modified());
}

#[test]
fn test_enum_parameter_comprehensive() {
    let options = vec![
        EnumValue::new("None".to_string(), 0),
        EnumValue::new("Low".to_string(), 1),
        EnumValue::new("Medium".to_string(), 2),
        EnumValue::new("High".to_string(), 3),
        EnumValue::new("Maximum".to_string(), 4),
    ];

    // Test valid enum creation
    let mut enum_param = Parameter::new_enum("Quality", 2, options.clone()).unwrap();
    assert_eq!(enum_param.get_type(), ParameterType::Enum);

    if let ParameterValue::Enum {
        value,
        options: param_options,
    } = enum_param.get_value()
    {
        assert_eq!(*value, 2);
        assert_eq!(param_options.len(), 5);

        // Find the "Medium" option
        let medium_option = param_options.iter().find(|opt| opt.value == 2).unwrap();
        assert_eq!(medium_option.name, "Medium");
    } else {
        panic!("Expected enum value");
    }

    // Test valid value change
    let new_enum_value = ParameterValue::Enum {
        value: 4,
        options: options.clone(),
    };
    assert!(enum_param.set_value(new_enum_value).is_ok());

    // Test invalid enum value creation
    let invalid_result = Parameter::new_enum("BadQuality", 99, options);
    assert!(invalid_result.is_err());
    if let Err(ParameterError::InvalidEnumValue {
        value,
        valid_values,
    }) = invalid_result
    {
        assert_eq!(value, 99);
        assert_eq!(valid_values, vec![0, 1, 2, 3, 4]);
    } else {
        panic!("Expected InvalidEnumValue error");
    }
}

#[test]
fn test_complex_parameter_types() {
    // Test Matrix3D parameter
    let matrix_value = ParameterValue::Matrix3D(Matrix3D::identity());
    let matrix_param = Parameter::new("Transform".to_string(), matrix_value).unwrap();
    assert_eq!(matrix_param.get_type(), ParameterType::Matrix3D);

    // Test Rect parameter
    let rect_value = ParameterValue::Rect(Rect::new(0.0, 0.0, 100.0, 50.0));
    let rect_param = Parameter::new("Bounds".to_string(), rect_value).unwrap();
    assert_eq!(rect_param.get_type(), ParameterType::Rect);

    // Test OBBox (Zone) parameter
    let obbox_value = ParameterValue::Zone(OBBox::new(
        Vector3::new(10.0, 20.0, 30.0),
        Vector3::new(5.0, 5.0, 5.0),
        Matrix3D::identity(),
    ));
    let zone_param = Parameter::new("CaptureZone".to_string(), obbox_value).unwrap();
    assert_eq!(zone_param.get_type(), ParameterType::Zone);

    // Test Script parameter
    let script_value = ParameterValue::Script(Script::new(
        "OnCreate".to_string(),
        "spawn_effects=true, delay=2.0".to_string(),
    ));
    let script_param = Parameter::new("CreationScript".to_string(), script_value).unwrap();
    assert_eq!(script_param.get_type(), ParameterType::Script);

    // Test TextureFilename parameter
    let texture_value = ParameterValue::TextureFilename {
        path: "textures/ground.dds".to_string(),
        show_alpha: true,
        show_texture: false,
    };
    let texture_param = Parameter::new("GroundTexture".to_string(), texture_value).unwrap();
    assert_eq!(texture_param.get_type(), ParameterType::TextureFilename);

    // Test list types
    let filename_list_value = ParameterValue::FilenameList(vec![
        "model1.w3d".to_string(),
        "model2.w3d".to_string(),
        "model3.w3d".to_string(),
    ]);
    let filename_list_param =
        Parameter::new("ModelFiles".to_string(), filename_list_value).unwrap();
    assert_eq!(filename_list_param.get_type(), ParameterType::FilenameList);

    let script_list_value = ParameterValue::ScriptList {
        names: vec!["Script1".to_string(), "Script2".to_string()],
        parameters: vec!["param1=1".to_string(), "param2=2".to_string()],
    };
    let script_list_param = Parameter::new("Scripts".to_string(), script_list_value).unwrap();
    assert_eq!(script_list_param.get_type(), ParameterType::ScriptList);
}

#[test]
fn test_parameter_units_and_metadata() {
    let mut param = Parameter::new_float("Temperature", 25.0, None).unwrap();

    // Test units
    assert_eq!(param.get_units(), None);
    param.set_units(Some("°C".to_string()));
    assert_eq!(param.get_units(), Some(&"°C".to_string()));

    // Test modified flag
    assert!(!param.is_modified());
    param.set_modified(true);
    assert!(param.is_modified());

    // Test that setting value marks as modified
    param.set_modified(false);
    assert!(!param.is_modified());
    param.set_value(ParameterValue::Float(30.0)).unwrap();
    assert!(param.is_modified());
}

#[test]
fn test_parameter_list_modified_tracking() {
    let mut list = ParameterList::new();

    // Add some parameters
    list.add(Parameter::new_int("param1", 100, None).unwrap())
        .unwrap();
    list.add(Parameter::new_float("param2", 5.5, None).unwrap())
        .unwrap();
    list.add(Parameter::new_string("param3", "test".to_string()).unwrap())
        .unwrap();

    // Initially no parameters should be modified
    let modified = list.get_modified();
    assert_eq!(modified.len(), 0);

    // Modify one parameter
    list.get_mut("param2")
        .unwrap()
        .set_value(ParameterValue::Float(7.5))
        .unwrap();

    let modified = list.get_modified();
    assert_eq!(modified.len(), 1);
    assert_eq!(modified[0].name, "param2");

    // Mark all as unmodified
    list.mark_all_unmodified();
    let modified = list.get_modified();
    assert_eq!(modified.len(), 0);
}

/// Helper function to create a comprehensive parameter list for testing
fn create_comprehensive_parameter_list() -> ParameterList {
    let mut list = ParameterList::new();

    // Add various parameter types
    list.add(Parameter::new_int("health", 100, Some((1, 1000))).unwrap())
        .unwrap();
    list.add(Parameter::new_float("speed", 5.5, Some((0.0, 100.0))).unwrap())
        .unwrap();
    list.add(Parameter::new_string("name", "Test Unit".to_string()).unwrap())
        .unwrap();
    list.add(Parameter::new_bool("active", true).unwrap())
        .unwrap();
    list.add(Parameter::new_angle("rotation", std::f32::consts::PI / 4.0).unwrap())
        .unwrap();
    list.add(Parameter::new_vector2("position2d", 10.0, 20.0).unwrap())
        .unwrap();
    list.add(Parameter::new_vector3("position3d", 1.0, 2.0, 3.0).unwrap())
        .unwrap();
    list.add(Parameter::new_color("tint", 0.8, 0.6, 0.4).unwrap())
        .unwrap();
    list.add(
        Parameter::new_filename(
            "texture",
            "test.dds".to_string(),
            Some("dds".to_string()),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    list.add(Parameter::new_separator("--- Section ---").unwrap())
        .unwrap();

    // Add enum parameter
    let enum_options = vec![
        EnumValue::new("Low".to_string(), 0),
        EnumValue::new("High".to_string(), 1),
    ];
    list.add(Parameter::new_enum("quality", 1, enum_options).unwrap())
        .unwrap();

    list
}
