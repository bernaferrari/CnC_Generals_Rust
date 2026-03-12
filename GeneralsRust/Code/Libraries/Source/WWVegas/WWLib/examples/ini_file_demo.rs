//! INI file I/O demo
//!
//! This example demonstrates loading from and saving to actual files.

use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;
use wwlib_rust::ini::{INIClass, INIResult};

fn main() -> INIResult<()> {
    println!("=== WWLib Rust INI File I/O Demo ===\n");

    // Create a temporary INI file
    let mut temp_file = NamedTempFile::new().map_err(|e| {
        wwlib_rust::ini::INIError::IoError(format!("Failed to create temp file: {}", e))
    })?;

    let sample_ini_content = r#"; Sample game configuration file
[Display]
Width=1920
Height=1080
Windowed=false
VSync=true

[Gameplay]
Difficulty=2
PlayerName=General
UnitLimit=100
StartingCash=1000.0
"#;

    // Write sample content to file
    write!(temp_file, "{}", sample_ini_content).map_err(|e| {
        wwlib_rust::ini::INIError::IoError(format!("Failed to write to temp file: {}", e))
    })?;

    let temp_path = temp_file.path();
    println!("1. Created temporary INI file at: {:?}", temp_path);

    // Load INI from file
    println!("\n2. Loading INI from file...");
    let mut ini = INIClass::from_file(temp_path)?;

    println!(
        "   - Successfully loaded from: {:?}",
        ini.get_filename().unwrap()
    );
    println!("   - Sections found: {}", ini.section_count());

    // Read and display values
    println!("\n3. Reading values from file:");
    println!("   - Display Width: {}", ini.get_int("Display", "Width", 0));
    println!(
        "   - Display Height: {}",
        ini.get_int("Display", "Height", 0)
    );
    println!(
        "   - Windowed: {}",
        ini.get_bool("Display", "Windowed", true)
    );
    println!("   - VSync: {}", ini.get_bool("Display", "VSync", false));
    println!(
        "   - Difficulty: {}",
        ini.get_int("Gameplay", "Difficulty", 1)
    );
    println!(
        "   - Player Name: {}",
        ini.get_string("Gameplay", "PlayerName", "Unknown")
    );
    println!(
        "   - Unit Limit: {}",
        ini.get_int("Gameplay", "UnitLimit", 50)
    );
    println!(
        "   - Starting Cash: {}",
        ini.get_float("Gameplay", "StartingCash", 0.0)
    );

    // Modify some values
    println!("\n4. Modifying values...");
    ini.put_string("Gameplay", "PlayerName", "Supreme Commander");
    ini.put_int("Gameplay", "UnitLimit", 200);
    ini.put_bool("Display", "Windowed", true);
    ini.put_string("Audio", "MasterVolume", "95");
    ini.put_string("Audio", "SoundEnabled", "true");

    // Create a new temporary file for saving
    let mut save_file = NamedTempFile::new().map_err(|e| {
        wwlib_rust::ini::INIError::IoError(format!("Failed to create save file: {}", e))
    })?;
    let save_path = save_file.path().to_owned();

    // Save to new file
    println!("5. Saving modified INI to new file...");
    ini.save(&save_path)?;

    // Read back the saved file to verify
    println!("\n6. Verifying saved file:");
    let saved_content = fs::read_to_string(&save_path).map_err(|e| {
        wwlib_rust::ini::INIError::IoError(format!("Failed to read saved file: {}", e))
    })?;

    println!("   - Saved file content:");
    for (i, line) in saved_content.lines().enumerate() {
        println!("     {:2}: {}", i + 1, line);
    }

    // Load the saved file into a new INI instance
    println!("\n7. Loading saved file into new INI instance...");
    let ini2 = INIClass::from_file(&save_path)?;

    println!("   - Verifying loaded values:");
    println!(
        "     - Player Name: {}",
        ini2.get_string("Gameplay", "PlayerName", "Unknown")
    );
    println!(
        "     - Unit Limit: {}",
        ini2.get_int("Gameplay", "UnitLimit", 0)
    );
    println!(
        "     - Windowed: {}",
        ini2.get_bool("Display", "Windowed", false)
    );
    println!(
        "     - Master Volume: {}",
        ini2.get_string("Audio", "MasterVolume", "")
    );
    println!(
        "     - Sound Enabled: {}",
        ini2.get_bool("Audio", "SoundEnabled", false)
    );

    // Test error handling with non-existent file
    println!("\n8. Testing error handling with non-existent file...");
    match INIClass::from_file("/non/existent/path/config.ini") {
        Ok(_) => println!("   - Unexpected success!"),
        Err(e) => println!("   - Expected error: {}", e),
    }

    // Show comparison
    println!("\n9. Comparing original vs modified:");
    println!("   Original values:");
    let original_ini = INIClass::from_file(temp_path)?;
    println!(
        "     - Player Name: {}",
        original_ini.get_string("Gameplay", "PlayerName", "Unknown")
    );
    println!(
        "     - Unit Limit: {}",
        original_ini.get_int("Gameplay", "UnitLimit", 0)
    );
    println!(
        "     - Windowed: {}",
        original_ini.get_bool("Display", "Windowed", true)
    );

    println!("   Modified values:");
    println!(
        "     - Player Name: {}",
        ini2.get_string("Gameplay", "PlayerName", "Unknown")
    );
    println!(
        "     - Unit Limit: {}",
        ini2.get_int("Gameplay", "UnitLimit", 0)
    );
    println!(
        "     - Windowed: {}",
        ini2.get_bool("Display", "Windowed", true)
    );

    println!("\n=== File I/O Demo completed successfully! ===");
    Ok(())
}
