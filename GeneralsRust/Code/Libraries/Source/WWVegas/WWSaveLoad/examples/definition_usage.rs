//! Example usage of the Definition system

use ww_save_load::definition::*;
use ww_save_load::saveload::*;

/// Example weapon definition
#[derive(Debug, Default)]
struct WeaponDefinition {
    base: Definition,
    damage: i32,
    range: f32,
    fire_rate: f32,
}

// Use the macros to implement the required traits
ww_save_load::declare_editable!(WeaponDefinition, Definition);
ww_save_load::impl_definition_class!(WeaponDefinition, 0x12340001);

impl WeaponDefinition {
    pub fn new(id: u32, name: &str, damage: i32, range: f32, fire_rate: f32) -> Self {
        Self {
            base: Definition::with_id_and_name(id, name.to_string()),
            damage,
            range,
            fire_rate,
        }
    }
}

fn main() {
    println!("Definition System Example");
    println!("========================");

    // Create a weapon definition
    let mut ak47 = WeaponDefinition::new(1001, "AK-47", 35, 150.0, 2.5);

    println!("Created weapon: {}", ak47.get_name());
    println!("  ID: {}", ak47.get_id());
    println!("  Class ID: {:#08x}", ak47.get_class_id());
    println!("  Damage: {}", ak47.damage);
    println!("  Range: {}", ak47.range);
    println!("  Fire Rate: {}", ak47.fire_rate);

    // Test editable interface
    println!("\nEditable Class Info:");
    println!("  Type: {}", ak47.get_type_name());
    println!("  Description: {}", ak47.get_description());
    println!("  Properties: {:?}", ak47.get_editable_properties());

    // Test configuration validation
    match ak47.is_valid_config() {
        Ok(()) => println!("\n✓ Configuration is valid"),
        Err(e) => println!("\n✗ Configuration error: {}", e),
    }

    // Test user data
    ak47.set_user_data(0xDEADBEEF);
    println!("User data: {:#08x}", ak47.get_user_data());

    // Test save/load enabled status
    println!("Save enabled: {}", ak47.is_save_enabled());
    ak47.enable_save(false);
    println!("Save enabled after disable: {}", ak47.is_save_enabled());

    // Test creation
    match ak47.create() {
        Ok(_) => println!("\n✓ Successfully created new instance"),
        Err(e) => println!("\n✗ Creation failed: {}", e),
    }

    // Test ID change
    let old_id = ak47.get_id();
    if ak47.set_id(2002).is_ok() {
        println!("\n✓ ID changed from {} to {}", old_id, ak47.get_id());
    }

    println!("\nDefinition system example completed successfully!");
}
