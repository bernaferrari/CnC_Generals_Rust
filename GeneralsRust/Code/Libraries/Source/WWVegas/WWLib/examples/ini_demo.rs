//! INI demo showcasing INI file parsing and writing functionality
//!
//! This example demonstrates the key features of the INI module:
//! - Loading INI data from various sources
//! - Reading different data types (strings, integers, floats, booleans, hex)
//! - Writing data back to INI format
//! - Case-insensitive lookups
//! - Error handling

use std::io::Cursor;
use wwlib_rust::ini::{INIClass, INIResult};

fn main() -> INIResult<()> {
    println!("=== WWLib Rust INI Demo ===\n");

    // Example INI content (similar to what might be found in C&C Generals)
    let ini_content = r#"
; Command & Conquer Generals Game Configuration
; This is a sample INI file showing various data types

[Graphics]
Resolution=1920x1080
FullScreen=true
VSync=false
Quality=High
AntiAliasing=4
Brightness=0.8
HexColor=0xFF0000

[Audio]
MasterVolume=80
SoundEffects=true
Music=false
Channels=2

[Network]
PlayerName=Commander
MaxPlayers=8
Port=8080
; Comment about server settings
ServerIP=192.168.1.100

[GameSettings]
Difficulty=Normal
; Various game modes
GameMode=Skirmish
AILevel=$03
MapSize=Large
TimeLimit=30.0
"#;

    println!("1. Loading INI data from string...");
    let mut ini = INIClass::new();
    ini.load_from_reader(&mut Cursor::new(ini_content))?;

    println!("   - Loaded {} sections", ini.section_count());
    println!("   - Sections: {:?}", ini.get_section_names());

    println!("\n2. Reading different data types:");

    // String values
    let resolution = ini.get_string("Graphics", "Resolution", "800x600");
    println!("   - Resolution: {}", resolution);

    // Boolean values
    let fullscreen = ini.get_bool("Graphics", "FullScreen", false);
    println!("   - Full Screen: {}", fullscreen);

    // Integer values
    let anti_aliasing = ini.get_int("Graphics", "AntiAliasing", 0);
    println!("   - Anti-Aliasing: {}", anti_aliasing);

    // Float values
    let brightness = ini.get_float("Graphics", "Brightness", 1.0);
    println!("   - Brightness: {}", brightness);

    // Hex values
    let hex_color = ini.get_hex("Graphics", "HexColor", 0);
    println!("   - Hex Color: 0x{:06X}", hex_color);

    // Hex with $ prefix
    let ai_level = ini.get_int("GameSettings", "AILevel", 1);
    println!("   - AI Level (hex): {}", ai_level);

    println!("\n3. Testing case-insensitive lookup:");
    let player_name_1 = ini.get_string("Network", "PlayerName", "Unknown");
    let player_name_2 = ini.get_string("NETWORK", "playername", "Unknown");
    let player_name_3 = ini.get_string("network", "PLAYERNAME", "Unknown");
    println!("   - PlayerName (normal): {}", player_name_1);
    println!("   - PlayerName (UPPER): {}", player_name_2);
    println!("   - PlayerName (mixed): {}", player_name_3);
    println!(
        "   - All equal: {}",
        player_name_1 == player_name_2 && player_name_2 == player_name_3
    );

    println!("\n4. Modifying INI data:");
    ini.put_string("Graphics", "Quality", "Ultra");
    ini.put_int("Network", "MaxPlayers", 16);
    ini.put_float("GameSettings", "TimeLimit", 45.5);
    ini.put_bool("Audio", "Music", true);
    ini.put_hex("Graphics", "BackgroundColor", 0x123456);

    // Add a new section
    ini.put_string("Debug", "LogLevel", "Verbose");
    ini.put_bool("Debug", "ShowFPS", true);

    println!("   - Modified existing values and added Debug section");

    println!("\n5. Verifying changes:");
    println!(
        "   - Quality: {}",
        ini.get_string("Graphics", "Quality", "")
    );
    println!(
        "   - Max Players: {}",
        ini.get_int("Network", "MaxPlayers", 0)
    );
    println!(
        "   - Time Limit: {}",
        ini.get_float("GameSettings", "TimeLimit", 0.0)
    );
    println!("   - Music: {}", ini.get_bool("Audio", "Music", false));
    println!(
        "   - Background Color: 0x{:06X}",
        ini.get_hex("Graphics", "BackgroundColor", 0)
    );
    println!(
        "   - Log Level: {}",
        ini.get_string("Debug", "LogLevel", "")
    );
    println!("   - Show FPS: {}", ini.get_bool("Debug", "ShowFPS", false));

    println!("\n6. Section and entry information:");
    for section_name in ini.get_section_names() {
        let entry_count = ini.entry_count(section_name);
        println!(
            "   - Section '{}' has {} entries",
            section_name, entry_count
        );

        let keys = ini.get_entry_keys(section_name);
        if keys.len() <= 3 {
            println!("     Keys: {:?}", keys);
        } else {
            println!("     Keys: {:?}... ({} total)", &keys[0..3], keys.len());
        }
    }

    println!("\n7. Saving INI data:");
    let mut output = Vec::new();
    ini.save_to_writer(&mut output)?;
    let saved_content = String::from_utf8(output).unwrap();

    println!("   - Saved INI data ({} bytes):", saved_content.len());
    println!("   - First few lines:");
    for (i, line) in saved_content.lines().take(10).enumerate() {
        if !line.trim().is_empty() {
            println!("     {}: {}", i + 1, line);
        }
    }

    println!("\n8. Testing utility functions:");
    println!(
        "   - Is Graphics section present: {}",
        ini.section_present("Graphics")
    );
    println!(
        "   - Is PlayerName present: {}",
        ini.is_present("Network", Some("PlayerName"))
    );
    println!(
        "   - Is NonExistent present: {}",
        ini.is_present("Network", Some("NonExistent"))
    );
    println!("   - Total size estimate: {} bytes", ini.size());

    println!("\n9. Testing enumerate entries:");
    // Add some numbered entries for testing
    ini.put_string("Maps", "Map1", "Desert Storm");
    ini.put_string("Maps", "Map2", "Urban Combat");
    ini.put_string("Maps", "Map5", "Mountain Pass");
    ini.put_string("Maps", "Other", "Not a map");

    let map_count = ini.enumerate_entries("Maps", "Map", 1, 10);
    println!("   - Found {} numbered map entries", map_count);

    println!("\n10. Error handling example:");
    match ini.load_from_reader(&mut Cursor::new("InvalidLine\n[Section]\nKey=Value")) {
        Ok(_) => println!("   - Unexpected success"),
        Err(e) => println!("   - Expected error: {}", e),
    }

    println!("\n=== Demo completed successfully! ===");
    Ok(())
}
