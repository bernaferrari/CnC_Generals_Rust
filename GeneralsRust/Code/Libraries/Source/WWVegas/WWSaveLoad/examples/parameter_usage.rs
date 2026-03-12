//! Parameter System Usage Example
//!
//! This example demonstrates how to use the parameter system for configuration management.
//! It shows creating various parameter types, validation, serialization, and parameter lists.

use ww_save_load::parameter::{
    EnumValue, Parameter, ParameterFactory, ParameterList, ParameterType, ParameterValue, Range,
    Script, Vector2, Vector3,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Parameter System Usage Example ===\n");

    // Create various parameter types
    create_basic_parameters()?;
    create_complex_parameters()?;
    create_parameter_list()?;
    demonstrate_validation()?;
    demonstrate_serialization()?;
    demonstrate_parameter_factory()?;

    Ok(())
}

fn create_basic_parameters() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Creating Basic Parameters:");

    // Integer parameter with range validation
    let health_param = Parameter::new_int("MaxHealth", 100, Some((1, 1000)))?;
    println!(
        "   Health: {} = {}",
        health_param.name,
        health_param.get_value().to_string()
    );

    // Float parameter with range validation
    let speed_param = Parameter::new_float("MovementSpeed", 5.5, Some((0.0, 100.0)))?;
    println!(
        "   Speed: {} = {}",
        speed_param.name,
        speed_param.get_value().to_string()
    );

    // String parameter
    let name_param = Parameter::new_string("UnitName", "Infantry".to_string())?;
    println!(
        "   Name: {} = {}",
        name_param.name,
        name_param.get_value().to_string()
    );

    // Boolean parameter
    let stealth_param = Parameter::new_bool("CanStealth", true)?;
    println!(
        "   Stealth: {} = {}",
        stealth_param.name,
        stealth_param.get_value().to_string()
    );

    // Angle parameter (in radians)
    let angle_param = Parameter::new_angle("FiringAngle", std::f32::consts::PI / 4.0)?;
    println!(
        "   Angle: {} = {} radians",
        angle_param.name,
        angle_param.get_value().to_string()
    );

    println!();
    Ok(())
}

fn create_complex_parameters() -> Result<(), Box<dyn std::error::Error>> {
    println!("2. Creating Complex Parameters:");

    // Vector2 parameter
    let position_2d = Parameter::new_vector2("Position2D", 10.0, 20.0)?;
    println!(
        "   Position2D: {} = {}",
        position_2d.name,
        position_2d.get_value().to_string()
    );

    // Vector3 parameter
    let position_3d = Parameter::new_vector3("Position3D", 1.0, 2.0, 3.0)?;
    println!(
        "   Position3D: {} = {}",
        position_3d.name,
        position_3d.get_value().to_string()
    );

    // Color parameter
    let color_param = Parameter::new_color("TintColor", 0.8, 0.6, 0.4)?;
    println!(
        "   Color: {} = {}",
        color_param.name,
        color_param.get_value().to_string()
    );

    // Enum parameter
    let quality_options = vec![
        EnumValue::new("Low".to_string(), 0),
        EnumValue::new("Medium".to_string(), 1),
        EnumValue::new("High".to_string(), 2),
        EnumValue::new("Ultra".to_string(), 3),
    ];
    let quality_param = Parameter::new_enum("GraphicsQuality", 2, quality_options)?;
    println!(
        "   Quality: {} = {}",
        quality_param.name,
        quality_param.get_value().to_string()
    );

    // Filename parameter
    let texture_param = Parameter::new_filename(
        "DiffuseTexture",
        "textures/unit_diffuse.dds".to_string(),
        Some("dds".to_string()),
        Some("Diffuse Texture Files".to_string()),
    )?;
    println!(
        "   Texture: {} = {}",
        texture_param.name,
        texture_param.get_value().to_string()
    );

    // Script parameter
    let script_value = ParameterValue::Script(Script::new(
        "OnDeath".to_string(),
        "explosion_type=large, damage_radius=10.0".to_string(),
    ));
    let script_param = Parameter::new("DeathScript".to_string(), script_value)?;
    println!(
        "   Script: {} = {}",
        script_param.name,
        script_param.get_value().to_string()
    );

    println!();
    Ok(())
}

fn create_parameter_list() -> Result<(), Box<dyn std::error::Error>> {
    println!("3. Creating Parameter List:");

    let mut param_list = ParameterList::new();

    // Add various parameters
    param_list.add(Parameter::new_int("Health", 100, Some((1, 1000)))?)?;
    param_list.add(Parameter::new_float("Speed", 5.5, None)?)?;
    param_list.add(Parameter::new_string("Name", "Tank".to_string())?)?;
    param_list.add(Parameter::new_bool("Armored", true)?)?;
    param_list.add(Parameter::new_vector3("Size", 2.0, 1.5, 3.0)?)?;
    param_list.add(Parameter::new_separator("--- Combat ---")?)?;
    param_list.add(Parameter::new_int("Damage", 50, Some((1, 200)))?)?;
    param_list.add(Parameter::new_float("FireRate", 1.2, Some((0.1, 5.0)))?)?;

    println!(
        "   Parameter list contains {} parameters:",
        param_list.len()
    );
    for (name, param) in param_list.iter() {
        let type_str = format!("{:?}", param.get_type());
        println!(
            "     {}: {} = {}",
            name,
            type_str,
            param.get_value().to_string()
        );
    }

    // Demonstrate parameter access
    if let Some(health_param) = param_list.get("Health") {
        println!(
            "   Direct access - Health: {}",
            health_param.get_value().to_string()
        );
    }

    // Modify a parameter
    if let Some(speed_param) = param_list.get_mut("Speed") {
        speed_param.set_value(ParameterValue::Float(7.5))?;
        println!(
            "   Modified Speed to: {}",
            speed_param.get_value().to_string()
        );
    }

    println!();
    Ok(())
}

fn demonstrate_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("4. Demonstrating Validation:");

    // Create parameter with range validation
    let mut range_param = Parameter::new_int("LimitedValue", 50, Some((0, 100)))?;
    println!("   Initial value: {}", range_param.get_value().to_string());

    // Valid modification
    match range_param.set_value(ParameterValue::Int(75)) {
        Ok(_) => println!("   ✓ Successfully set value to 75"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Invalid modification (out of range)
    match range_param.set_value(ParameterValue::Int(150)) {
        Ok(_) => println!("   ✓ Successfully set value to 150"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Type mismatch
    match range_param.set_value(ParameterValue::String("invalid".to_string())) {
        Ok(_) => println!("   ✓ Successfully set string value"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Enum validation
    let options = vec![
        EnumValue::new("Option1".to_string(), 1),
        EnumValue::new("Option2".to_string(), 2),
    ];

    match Parameter::new_enum("TestEnum", 3, options) {
        Ok(_) => println!("   ✓ Created enum with invalid value"),
        Err(e) => println!("   ✗ Enum validation error: {}", e),
    }

    println!();
    Ok(())
}

fn demonstrate_serialization() -> Result<(), Box<dyn std::error::Error>> {
    println!("5. Demonstrating Serialization:");

    // Create a parameter
    let original = Parameter::new_int("TestParam", 42, Some((0, 100)))?;
    println!(
        "   Original parameter: {} = {}",
        original.name,
        original.get_value().to_string()
    );

    // Serialize to JSON
    let json_str = serde_json::to_string_pretty(&original)?;
    println!("   Serialized JSON:");
    println!("{}", json_str);

    // Deserialize from JSON
    let deserialized: Parameter = serde_json::from_str(&json_str)?;
    println!(
        "   Deserialized parameter: {} = {}",
        deserialized.name,
        deserialized.get_value().to_string()
    );

    // Verify they match
    println!(
        "   Parameters match: {}",
        original.name == deserialized.name && original.get_value() == deserialized.get_value()
    );

    println!();
    Ok(())
}

fn demonstrate_parameter_factory() -> Result<(), Box<dyn std::error::Error>> {
    println!("6. Demonstrating Parameter Factory:");

    // Create parameters from string data (like parsing config files)
    let factory_examples = vec![
        (ParameterType::Int, "MaxAmmo", "30"),
        (ParameterType::Float, "ReloadTime", "2.5"),
        (ParameterType::Bool, "AutoFire", "true"),
        (ParameterType::String, "WeaponType", "Rifle"),
        (ParameterType::Vector2, "Recoil", "(1.5, 2.0)"),
        (ParameterType::Vector3, "MuzzleFlash", "(0.8, 0.6, 0.2)"),
        (ParameterType::Color, "TraceColor", "(1.0, 0.8, 0.0)"),
        (ParameterType::Angle, "Spread", "0.1745"), // ~10 degrees in radians
    ];

    for (param_type, name, data) in factory_examples {
        match ParameterFactory::construct(param_type, name, data) {
            Ok(param) => {
                println!(
                    "   ✓ Created {:?} '{}' = {}",
                    param_type,
                    name,
                    param.get_value().to_string()
                );
            }
            Err(e) => {
                println!("   ✗ Failed to create {:?} '{}': {}", param_type, name, e);
            }
        }
    }

    println!();
    Ok(())
}
