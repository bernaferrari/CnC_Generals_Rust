use generals_main::assets::archive::ArchiveFileSystem;
use generals_main::assets::audio::AudioManager;
use log::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🎵 C&C Generals Music Loading Test");
    println!("==================================");

    // Initialize archive system
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;
    println!("✅ Archive system initialized");

    // Initialize audio manager
    let mut audio_manager = AudioManager::new()?;
    println!("✅ Audio manager initialized");

    // Test the actual music files found in archives (short filenames as in C++)
    let test_tracks = vec![
        "usa_10.mp3",
        "usa_11.mp3",
        "chi_10.mp3",
        "chi_11.mp3",
        "gla_10.mp3",
        "gla_11.mp3",
        "c_chix01.mp3",
    ];

    println!("\n🔍 Testing individual track loading...");
    for track in &test_tracks {
        if archive_system.does_file_exist(track) {
            println!("✅ {} - EXISTS", track);

            // Try to load the actual music file data
            match archive_system.open_file(track).await {
                Ok(data) => {
                    println!("   📊 Size: {} bytes", data.len());

                    // Try to play it briefly (this will test the audio decoding)
                    match audio_manager
                        .play_background_music(&mut archive_system, track)
                        .await
                    {
                        Ok(()) => {
                            println!("   🎵 Audio decoding successful!");

                            // Stop after a brief moment
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            audio_manager.stop_background_music();
                            println!("   ⏹️  Stopped successfully");
                        }
                        Err(e) => {
                            println!("   ❌ Audio decoding failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("   ❌ Failed to load file data: {}", e);
                }
            }
        } else {
            println!("❌ {} - NOT FOUND", track);
        }
    }

    println!("\n🎯 Testing faction music functions...");

    // Test USA faction music
    println!("\n--- Testing USA faction music ---");
    match audio_manager
        .play_faction_music(&mut archive_system, "usa")
        .await
    {
        Ok(()) => {
            println!("✅ USA faction music loaded successfully!");
            println!(
                "🎵 Currently playing: {}",
                audio_manager.get_current_track().unwrap_or("Unknown")
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            audio_manager.stop_background_music();
        }
        Err(e) => {
            println!("❌ USA faction music failed: {}", e);
        }
    }

    // Test China faction music
    println!("\n--- Testing China faction music ---");
    match audio_manager
        .play_faction_music(&mut archive_system, "china")
        .await
    {
        Ok(()) => {
            println!("✅ China faction music loaded successfully!");
            println!(
                "🎵 Currently playing: {}",
                audio_manager.get_current_track().unwrap_or("Unknown")
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            audio_manager.stop_background_music();
        }
        Err(e) => {
            println!("❌ China faction music failed: {}", e);
        }
    }

    // Test GLA faction music
    println!("\n--- Testing GLA faction music ---");
    match audio_manager
        .play_faction_music(&mut archive_system, "gla")
        .await
    {
        Ok(()) => {
            println!("✅ GLA faction music loaded successfully!");
            println!(
                "🎵 Currently playing: {}",
                audio_manager.get_current_track().unwrap_or("Unknown")
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            audio_manager.stop_background_music();
        }
        Err(e) => {
            println!("❌ GLA faction music failed: {}", e);
        }
    }

    println!("\n🎲 Testing random music selection...");
    match audio_manager
        .play_random_cnc_music(&mut archive_system)
        .await
    {
        Ok(()) => {
            println!("✅ Random music selection successful!");
            println!(
                "🎵 Currently playing: {}",
                audio_manager.get_current_track().unwrap_or("Unknown")
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
            audio_manager.stop_background_music();
        }
        Err(e) => {
            println!("❌ Random music selection failed: {}", e);
        }
    }

    println!("\n🎉 Music loading test completed!");
    println!("The music files should now load correctly in the game.");

    Ok(())
}
