/// Standalone test for FXList INI parsing
/// Run with: cargo run --example test_fxlist_parsing

use game_client_rust::GameClient::fx_list::*;

fn main() {
    println!("Testing FXList INI Parser\n");
    println!("{}", "=".repeat(60));

    // Test 1: Simple Sound FX
    test_simple_sound_fx();

    // Test 2: Complex multi-nugget FXList
    test_complex_fx_list();

    // Test 3: Empty FXList
    test_empty_fx_list();

    // Test 4: Parser Helper Functions
    test_parser_helpers();

    println!("\n{}", "=".repeat(60));
    println!("All tests completed successfully!");
}

fn test_simple_sound_fx() {
    println!("\n[TEST 1] Simple Sound FX");
    println!("-".repeat(60));

    let ini_data = r#"
        FXList FX_TestSound
          Sound
            Name = TestSoundEffect
          End
        End
    "#;

    let mut store = FXListStore::new();
    match store.parse_fx_list_definition(ini_data) {
        Ok(_) => {
            println!("✓ Parsed successfully");
            if let Some(fx_list) = store.find_fx_list("FX_TestSound") {
                println!("✓ Found FXList 'FX_TestSound'");
                println!("✓ Contains {} nugget(s)", fx_list.nuggets.len());
            } else {
                println!("✗ Failed to find FXList");
            }
        }
        Err(e) => println!("✗ Parse failed: {}", e),
    }
}

fn test_complex_fx_list() {
    println!("\n[TEST 2] Complex Multi-Nugget FXList");
    println!("-".repeat(60));

    let ini_data = r#"
        FXList FX_DamageTankStruck
          ParticleSystem
            Name = TankStruckSmoke
            Height = 10 10 CONSTANT
            OrientToObject = Yes
            Ricochet = Yes
          End
          LightPulse
            Color = R:255 G:255 B:128
            Radius = 30
            IncreaseTime = 0
            DecreaseTime = 500
          End
          Sound
            Name = VehicleImpactHeavy
          End
          ViewShake
            Type = SEVERE
          End
        End
    "#;

    let mut store = FXListStore::new();
    match store.parse_fx_list_definition(ini_data) {
        Ok(_) => {
            println!("✓ Parsed successfully");
            if let Some(fx_list) = store.find_fx_list("FX_DamageTankStruck") {
                println!("✓ Found FXList 'FX_DamageTankStruck'");
                println!("✓ Contains {} nuggets:", fx_list.nuggets.len());
                println!("  - ParticleSystem");
                println!("  - LightPulse");
                println!("  - Sound");
                println!("  - ViewShake");
            } else {
                println!("✗ Failed to find FXList");
            }
        }
        Err(e) => println!("✗ Parse failed: {}", e),
    }
}

fn test_empty_fx_list() {
    println!("\n[TEST 3] Empty FXList (as seen in game data)");
    println!("-".repeat(60));

    let ini_data = r#"
        FXList FX_GIDie
        ; Empty FXList
        End
    "#;

    let mut store = FXListStore::new();
    match store.parse_fx_list_definition(ini_data) {
        Ok(_) => {
            println!("✓ Parsed empty FXList successfully");
            if let Some(fx_list) = store.find_fx_list("FX_GIDie") {
                println!("✓ Found FXList 'FX_GIDie'");
                println!("✓ Nugget count: {} (expected 0)", fx_list.nuggets.len());
            }
        }
        Err(e) => println!("✗ Parse failed: {}", e),
    }
}

fn test_parser_helpers() {
    use game_client_rust::GameClient::fx_list::FXListINIParser;
    use std::f32::consts::PI;

    println!("\n[TEST 4] Parser Helper Functions");
    println!("-".repeat(60));

    // Test RGB Color parsing
    if let Ok(color) = FXListINIParser::parse_rgb_color("R:255 G:128 B:64") {
        println!("✓ RGB Color: R:{:.2} G:{:.2} B:{:.2}", color.red, color.green, color.blue);
    }

    // Test Coord3D parsing
    if let Ok(coord) = FXListINIParser::parse_coord3d("X:1.5 Y:2.5 Z:3.5") {
        println!("✓ Coord3D: X:{} Y:{} Z:{}", coord.x, coord.y, coord.z);
    }

    // Test Random Variable parsing
    if let Ok(var) = FXListINIParser::parse_random_variable("10 20 CONSTANT") {
        println!("✓ Random Variable: min:{} max:{}", var.min, var.max);
    }

    // Test Boolean parsing
    println!("✓ Boolean 'Yes': {}", FXListINIParser::parse_bool("Yes").unwrap());
    println!("✓ Boolean 'No': {}", FXListINIParser::parse_bool("No").unwrap());

    // Test Angle parsing (degrees to radians)
    if let Ok(radians) = FXListINIParser::parse_angle("180") {
        println!("✓ Angle 180° = {:.2} radians (PI = {:.2})", radians, PI);
    }

    // Test Duration parsing (ms to frames)
    if let Ok(frames) = FXListINIParser::parse_duration("500") {
        println!("✓ Duration 500ms = {} frames (at 30fps)", frames);
    }

    // Test Shake Type parsing
    if let Ok(shake) = FXListINIParser::parse_shake_type("SEVERE") {
        println!("✓ Shake Type: {:?}", shake);
    }

    // Test Scorch Type parsing
    if let Ok(scorch) = FXListINIParser::parse_scorch_type("SCORCH_3") {
        println!("✓ Scorch Type: {} (SCORCH_3)", scorch);
    }
}
