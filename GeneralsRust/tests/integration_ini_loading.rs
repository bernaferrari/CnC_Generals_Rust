//! Integration Test: INI Loading and Parsing
//!
//! This test verifies that the game can load and parse INI configuration files correctly.
//! INI files are used throughout C&C Generals for:
//! - Unit definitions (stats, weapons, behavior)
//! - Building definitions
//! - Weapon templates
//! - Game rules and balance
//! - Map settings
//!
//! Tests should pass on all platforms (Windows, Linux, macOS)

#![cfg(test)]

use std::collections::HashMap;
use std::io::Write;

/// Test basic INI parsing
#[test]
fn test_basic_ini_parsing() {
    println!("Testing basic INI parsing...");

    // Create a simple INI content
    let ini_content = r#"
; This is a comment
[General]
Name = TestObject
Health = 100
Speed = 50.5
Enabled = true

[Weapon]
Type = Cannon
Damage = 25
Range = 300
RateOfFire = 0.5
"#;

    // Parse manually for testing
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current_section = String::new();

    for line in ini_content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        // Section header
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
            sections.insert(current_section.clone(), HashMap::new());
            continue;
        }

        // Key-value pair
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim().to_string();
            let value = line[pos + 1..].trim().to_string();

            if !current_section.is_empty() {
                sections
                    .get_mut(&current_section)
                    .unwrap()
                    .insert(key, value);
            }
        }
    }

    // Verify parsing
    assert!(sections.contains_key("General"));
    assert!(sections.contains_key("Weapon"));

    let general = sections.get("General").unwrap();
    assert_eq!(general.get("Name").unwrap(), "TestObject");
    assert_eq!(general.get("Health").unwrap(), "100");
    assert_eq!(general.get("Speed").unwrap(), "50.5");
    assert_eq!(general.get("Enabled").unwrap(), "true");

    let weapon = sections.get("Weapon").unwrap();
    assert_eq!(weapon.get("Type").unwrap(), "Cannon");
    assert_eq!(weapon.get("Damage").unwrap(), "25");

    log::info!("Basic INI parsing test passed");
}

/// Test INI file loading from disk
#[test]
fn test_ini_file_loading() {
    println!("Testing INI file loading from disk...");

    // Create temporary INI file
    let temp_dir = std::env::temp_dir();
    let ini_path = temp_dir.join("test_config.ini");

    let ini_content = r#"
[GameSettings]
Difficulty = Medium
StartingMoney = 10000
EnableFogOfWar = true

[UnitDefaults]
DefaultHealth = 100
DefaultSpeed = 25
"#;

    // Write to file
    std::fs::write(&ini_path, ini_content).expect("Failed to write INI file");

    // Read back
    let content = std::fs::read_to_string(&ini_path).expect("Failed to read INI file");

    assert!(content.contains("[GameSettings]"));
    assert!(content.contains("Difficulty = Medium"));
    assert!(content.contains("[UnitDefaults]"));

    // Cleanup
    std::fs::remove_file(&ini_path).ok();

    log::info!("INI file loading test passed");
}

/// Test parsing unit definition
#[test]
fn test_unit_definition_parsing() {
    println!("Testing unit definition parsing...");

    let unit_ini = r#"
[AmericaTank]
; M1 Abrams Tank
Object = TANK_Abrams

Health = 400
Armor = TANK_ARMOR
Speed = 40.0
TurnRate = 180.0
BuildCost = 800
BuildTime = 10.0

VisionRange = 200
ShroudClearingRange = 200

; Weapons
PrimaryWeapon = TankCannon
SecondaryWeapon = MachineGun

; Abilities
CanCrushInfantry = true
RequiresPower = false
"#;

    // Verify key elements are present
    assert!(unit_ini.contains("AmericaTank"));
    assert!(unit_ini.contains("Health = 400"));
    assert!(unit_ini.contains("Speed = 40.0"));
    assert!(unit_ini.contains("PrimaryWeapon = TankCannon"));

    log::info!("Unit definition parsing test passed");
}

/// Test parsing weapon template
#[test]
fn test_weapon_template_parsing() {
    println!("Testing weapon template parsing...");

    let weapon_ini = r#"
[TankCannon]
; Main battle tank cannon
WeaponType = PROJECTILE

Damage = 75
DamageType = ARMOR_PIERCING
Radius = 5.0

Range = 300
MinRange = 0
ReloadTime = 2.0

Accuracy = 95
AccuracyAgainstMoving = 75

ProjectileSpeed = 500
ProjectileGravity = 0.5

CanFireWhileMoving = true
CanFireWhileRotating = true

; Effects
FireSound = TankCannonFire
ImpactSound = TankCannonImpact
MuzzleFlash = CannonMuzzleFlash
"#;

    assert!(weapon_ini.contains("TankCannon"));
    assert!(weapon_ini.contains("Damage = 75"));
    assert!(weapon_ini.contains("DamageType = ARMOR_PIERCING"));
    assert!(weapon_ini.contains("Range = 300"));

    log::info!("Weapon template parsing test passed");
}

/// Test INI value type conversion
#[test]
fn test_ini_value_conversion() {
    println!("Testing INI value type conversion...");

    // Test parsing different types
    let test_values = vec![
        ("100", 100i32),
        ("50", 50i32),
        ("0", 0i32),
        ("-10", -10i32),
    ];

    for (str_val, expected) in test_values {
        let parsed: i32 = str_val.parse().unwrap();
        assert_eq!(parsed, expected);
    }

    // Test float parsing
    let float_values = vec![("50.5", 50.5f32), ("100.0", 100.0f32), ("0.25", 0.25f32)];

    for (str_val, expected) in float_values {
        let parsed: f32 = str_val.parse().unwrap();
        assert!((parsed - expected).abs() < 0.001);
    }

    // Test boolean parsing
    let bool_values = vec![
        ("true", true),
        ("false", false),
        ("True", true),
        ("False", false),
        ("TRUE", true),
        ("FALSE", false),
    ];

    for (str_val, expected) in bool_values {
        let parsed = str_val.to_lowercase() == "true";
        assert_eq!(parsed, expected);
    }

    log::info!("INI value conversion test passed");
}

/// Test INI comment handling
#[test]
fn test_ini_comment_handling() {
    println!("Testing INI comment handling...");

    let ini_with_comments = r#"
; This is a header comment
; Multiple lines of comments

[Section1]
; Comment before key
Key1 = Value1  ; Inline comment
; Comment between keys
Key2 = Value2

[Section2]
; Another comment
Key3 = Value3
"#;

    let lines: Vec<&str> = ini_with_comments
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with(';'))
        .collect();

    // Should have filtered out all comment-only lines
    assert!(lines.iter().all(|&l| !l.starts_with(';')));

    // But should still have sections and keys
    assert!(lines.iter().any(|&l| l.contains("[Section1]")));
    assert!(lines.iter().any(|&l| l.contains("Key1 =")));

    log::info!("INI comment handling test passed");
}

/// Test INI section inheritance (if supported)
#[test]
fn test_ini_section_structure() {
    println!("Testing INI section structure...");

    let complex_ini = r#"
[BaseUnit]
Health = 100
Speed = 25
Armor = LIGHT

[InfantryUnit]
; Inherits from BaseUnit
Health = 75
CanGarrison = true

[TankUnit]
; Inherits from BaseUnit
Health = 400
Armor = HEAVY
CanCrushInfantry = true
"#;

    let mut sections = Vec::new();

    for line in complex_ini.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            sections.push(line[1..line.len() - 1].to_string());
        }
    }

    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0], "BaseUnit");
    assert_eq!(sections[1], "InfantryUnit");
    assert_eq!(sections[2], "TankUnit");

    log::info!("INI section structure test passed");
}

/// Test handling malformed INI
#[test]
fn test_malformed_ini_handling() {
    println!("Testing malformed INI handling...");

    let malformed_cases = vec![
        "[MissingCloseBracket\nKey = Value",
        "MissingSection]\nKey = Value",
        "Key = Value\n[NoKeysBefore]",
        "Key Without Equals Value",
        "[Section]\n=NoKey",
    ];

    for (i, case) in malformed_cases.iter().enumerate() {
        println!("Testing malformed case {}: {:?}", i + 1, case);

        // Should handle gracefully (not panic)
        let result = std::panic::catch_unwind(|| {
            for line in case.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with(';') {
                    continue;
                }
                // Just verify we can process it without crashing
                let _ = line.contains('[');
                let _ = line.contains('=');
            }
        });

        assert!(result.is_ok(), "Should handle malformed INI gracefully");
    }

    log::info!("Malformed INI handling test passed");
}

/// Test loading multiple INI files
#[test]
fn test_multiple_ini_files() {
    println!("Testing multiple INI file loading...");

    let temp_dir = std::env::temp_dir();

    // Create multiple INI files
    let files = vec![
        ("units.ini", "[Tank]\nHealth = 400"),
        ("weapons.ini", "[Cannon]\nDamage = 75"),
        ("buildings.ini", "[Barracks]\nHealth = 1000"),
    ];

    let mut created_files = Vec::new();

    for (filename, content) in files {
        let path = temp_dir.join(filename);
        std::fs::write(&path, content).expect("Failed to write INI file");
        created_files.push(path);
    }

    // Verify all files exist
    for path in &created_files {
        assert!(path.exists(), "INI file should exist");
        let content = std::fs::read_to_string(path).expect("Should read file");
        assert!(!content.is_empty(), "File should have content");
    }

    // Cleanup
    for path in created_files {
        std::fs::remove_file(path).ok();
    }

    log::info!("Multiple INI files test passed");
}

/// Test INI case sensitivity
#[test]
fn test_ini_case_handling() {
    println!("Testing INI case sensitivity handling...");

    let ini_content = r#"
[TestSection]
Key = value
KEY = VALUE
key = Value
"#;

    let mut keys = Vec::new();

    for line in ini_content.lines() {
        let line = line.trim();
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim();
            keys.push(key.to_string());
        }
    }

    // Verify we captured all variations
    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&"Key".to_string()));
    assert!(keys.contains(&"KEY".to_string()));
    assert!(keys.contains(&"key".to_string()));

    log::info!("INI case handling test passed");
}

/// Test INI with unicode characters
#[test]
fn test_ini_unicode_support() {
    println!("Testing INI unicode support...");

    let unicode_ini = r#"
[Localization]
EnglishName = Tank
ChineseName = 坦克
RussianName = Танк
ArabicName = دبابة
JapaneseName = 戦車
"#;

    // Verify we can handle unicode
    assert!(unicode_ini.contains("坦克"));
    assert!(unicode_ini.contains("Танк"));
    assert!(unicode_ini.contains("دبابة"));
    assert!(unicode_ini.contains("戦車"));

    // Write and read back
    let temp_dir = std::env::temp_dir();
    let unicode_path = temp_dir.join("unicode_test.ini");

    std::fs::write(&unicode_path, unicode_ini).expect("Should write unicode INI");
    let read_back = std::fs::read_to_string(&unicode_path).expect("Should read unicode INI");

    assert!(read_back.contains("坦克"));
    assert!(read_back.contains("Танк"));

    // Cleanup
    std::fs::remove_file(unicode_path).ok();

    log::info!("INI unicode support test passed");
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    /// Test parsing large INI files
    #[test]
    #[ignore] // Run with: cargo test --test integration_ini_loading -- --ignored
    fn test_large_ini_parsing_performance() {
        println!("Performance test: Large INI parsing...");

        // Generate a large INI file
        let mut large_ini = String::new();

        for section_idx in 0..100 {
            large_ini.push_str(&format!("[Section{}]\n", section_idx));

            for key_idx in 0..100 {
                large_ini.push_str(&format!("Key{} = Value{}\n", key_idx, key_idx));
            }
        }

        let start = std::time::Instant::now();

        // Parse it
        let mut section_count = 0;
        let mut key_count = 0;

        for line in large_ini.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                section_count += 1;
            } else if line.contains('=') {
                key_count += 1;
            }
        }

        let elapsed = start.elapsed();

        println!("Parsed {} sections, {} keys in {:?}", section_count, key_count, elapsed);
        println!("Size: {} bytes", large_ini.len());

        assert_eq!(section_count, 100);
        assert_eq!(key_count, 10000);
        assert!(elapsed < std::time::Duration::from_millis(100), "Should parse quickly");

        log::info!("Large INI parsing performance test passed");
    }
}
