/*
** Simple Model Loading Test
** Test model loading with short timeouts until it works
*/

use generals_main::assets::archive::ArchiveFileSystem;
use generals_main::assets::models::W3DLoader;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Simple Model Loading Test");
    println!("=============================");

    // Initialize archive system
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;
    println!("✅ Archive system initialized");

    // Test models that we know exist
    let test_models = vec![
        "uirebel",    // Small GLA soldier model (~5KB)
        "avhummer",   // USA Humvee model (~40KB)
        "uvlitetank", // Light tank model (~70KB)
    ];

    let loader = W3DLoader::new();

    for (index, model_name) in test_models.iter().enumerate() {
        println!(
            "\n🎯 Test {}/{}: Loading {}",
            index + 1,
            test_models.len(),
            model_name
        );

        // Short timeout - 3 seconds max per model
        match tokio::time::timeout(
            Duration::from_secs(3),
            loader.load_model(&mut archive_system, model_name),
        )
        .await
        {
            Ok(Ok(model)) => {
                println!(
                    "✅ SUCCESS: {} loaded ({} meshes)",
                    model_name,
                    model.meshes.len()
                );
            }
            Ok(Err(e)) => {
                println!("❌ FAILED: {}: {}", model_name, e);
            }
            Err(_) => {
                println!("⏰ TIMEOUT: {} took longer than 3 seconds", model_name);
            }
        }

        // Small delay between models
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("\n🎉 Model loading test completed!");
    Ok(())
}
