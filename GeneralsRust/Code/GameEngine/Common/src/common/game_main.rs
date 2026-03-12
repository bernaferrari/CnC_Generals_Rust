// GameMain.rs
// The main entry point for the game
// Author: Converted from Michael S. Booth's C++ implementation, April 2001

use std::env;

use log::{error, info};

use crate::common::game_engine::create_game_engine;

/// Main entry point for the game system (exact C++ GameMain signature)
///
/// void GameMain(int argc, char *argv[])
///
/// This matches the C++ GameMain() function exactly:
/// 1. Creates the game engine using factory function (TheGameEngine = CreateGameEngine())
/// 2. Initializes it with command line arguments (TheGameEngine->init(argc, argv))
/// 3. Runs the main execute loop (TheGameEngine->execute())
/// 4. Cleans up when done (delete TheGameEngine)
pub async fn game_main(
    argc: i32,
    argv: *mut *mut i8,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!(
        "GameMain: Starting Command & Conquer Generals Zero Hour - Rust Edition (argc={})",
        argc
    );

    // Convert C-style argc/argv to Rust Vec<String> for internal use
    let args = unsafe {
        if argc <= 0 || argv.is_null() {
            vec!["generals".to_string()]
        } else {
            let argv_slice = std::slice::from_raw_parts(argv, argc as usize);
            argv_slice
                .iter()
                .filter_map(|&arg_ptr| {
                    if arg_ptr.is_null() {
                        None
                    } else {
                        std::ffi::CStr::from_ptr(arg_ptr)
                            .to_str()
                            .ok()
                            .map(|s| s.to_string())
                    }
                })
                .collect()
        }
    };

    info!("GameMain: Converted arguments: {:?}", args);

    // Initialize the game engine using factory function (matching C++ CreateGameEngine())
    let mut the_game_engine = create_game_engine();

    // Initialize the engine with command line arguments (matching C++ TheGameEngine->init())
    match the_game_engine.init(args).await {
        Ok(()) => {
            info!("GameMain: Engine initialization completed successfully");
        }
        Err(e) => {
            error!("GameMain: Engine initialization failed: {}", e);
            return Err(Box::new(e));
        }
    }

    // Run the main game loop (matching C++ TheGameEngine->execute())
    match the_game_engine.execute().await {
        Ok(()) => {
            info!("GameMain: Game execution completed normally");
        }
        Err(e) => {
            error!("GameMain: Game execution failed: {}", e);
            return Err(Box::new(e));
        }
    }

    // Engine shutdown is handled automatically by the execute() method
    // (matching C++ delete TheGameEngine; TheGameEngine = NULL;)
    info!("GameMain: Completed successfully");

    Ok(())
}

/// Simple wrapper that mimics the C++ GameMain signature for compatibility
pub async fn game_main_c_style(argc: usize, argv: Vec<String>) -> i32 {
    // Convert Vec<String> to C-style argc/argv for the real game_main function
    let c_strings: Vec<std::ffi::CString> = argv
        .iter()
        .map(|s| {
            std::ffi::CString::new(s.as_str())
                .unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
        })
        .collect();
    let mut c_argv: Vec<*mut i8> = c_strings.iter().map(|cs| cs.as_ptr() as *mut i8).collect();
    c_argv.push(std::ptr::null_mut()); // Null terminate

    match game_main(argc as i32, c_argv.as_mut_ptr()).await {
        Ok(()) => 0,
        Err(e) => {
            error!("Fatal error in game_main: {}", e);
            1
        }
    }
}

/// Convenience function to run game main from regular environment args
pub async fn run_from_env() -> i32 {
    let args: Vec<String> = env::args().collect();
    game_main_c_style(args.len(), args).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_game_main_minimal() {
        let args = vec!["game.exe".to_string()];

        // This should at least initialize without crashing
        // Note: We can't run the full game loop in tests as it would run forever
        // So we just test initialization
        let mut engine = create_game_engine();
        let result = engine.init(args).await;

        assert!(result.is_ok(), "Game engine initialization should succeed");

        // Clean shutdown
        let shutdown_result = engine.shutdown().await;
        assert!(
            shutdown_result.is_ok(),
            "Game engine shutdown should succeed"
        );
    }

    #[tokio::test]
    async fn test_game_main_with_args() {
        let args = vec![
            "game.exe".to_string(),
            "-windowed".to_string(),
            "-fps".to_string(),
            "60".to_string(),
        ];

        let mut engine = create_game_engine();
        let result = engine.init(args).await;

        assert!(
            result.is_ok(),
            "Game engine initialization with args should succeed"
        );
        assert_eq!(engine.get_frames_per_second_limit(), 60);

        // Clean shutdown
        let shutdown_result = engine.shutdown().await;
        assert!(
            shutdown_result.is_ok(),
            "Game engine shutdown should succeed"
        );
    }
}
