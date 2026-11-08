// GameMain.rs
// The main entry point for the game
// Author: Converted from Michael S. Booth's C++ implementation, April 2001

use std::env;

use log::{error, info};

use crate::common::game_engine::{clear_game_engine, create_game_engine, set_game_engine};

struct GameEngineLifecycleGuard;

impl Drop for GameEngineLifecycleGuard {
    fn drop(&mut self) {
        clear_game_engine();
    }
}

/// Runs the game with owned command-line arguments.
///
/// The number of arguments is derived from the vector; no independent count or
/// C pointer array is accepted. The future owns the strings even before it is
/// first polled.
///
/// Raw C arguments cannot be passed to the safe entry point:
///
/// ```compile_fail
/// use game_engine::common::game_main::game_main;
/// let _future = game_main(1, std::ptr::null_mut());
/// ```
///
/// Nor can a count be supplied separately from the owned arguments:
///
/// ```compile_fail
/// use game_engine::common::game_main::game_main;
/// let _future = game_main(100, vec!["generals".to_owned()]);
/// ```
pub async fn game_main(
    args: Vec<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!(
        "GameMain: Starting Command & Conquer Generals Zero Hour - Rust Edition (argc={})",
        args.len()
    );

    info!("GameMain: Arguments: {:?}", args);

    // Initialize the game engine using factory function (matching C++ CreateGameEngine()).
    // The global slot mirrors TheGameEngine in the original C++ startup path.
    let the_game_engine = set_game_engine(create_game_engine());
    let _engine_lifecycle_guard = GameEngineLifecycleGuard;

    // Initialize the engine with command line arguments (matching C++ TheGameEngine->init())
    let init_result = {
        let mut game_engine = the_game_engine.lock();
        game_engine.init(args).await
    };
    match init_result {
        Ok(()) => {
            info!("GameMain: Engine initialization completed successfully");
        }
        Err(e) => {
            error!("GameMain: Engine initialization failed: {}", e);
            return Err(Box::new(e));
        }
    }

    // Run the main game loop (matching C++ TheGameEngine->execute())
    let execute_result = {
        let mut game_engine = the_game_engine.lock();
        game_engine.execute().await
    };
    match execute_result {
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

/// Run the game with owned arguments and return a process-style exit code.
pub async fn game_main_with_exit_code(args: Vec<String>) -> i32 {
    match game_main(args).await {
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
    game_main_with_exit_code(args).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::common_services::CommonServicesScope;
    use crate::common::game_engine::{
        clear_game_client_factory, clear_game_engine, get_game_engine,
        register_successful_game_client_fixture,
    };

    #[test]
    fn owned_arguments_survive_until_future_is_polled() {
        match crate::test_process::run_bounded(
            module_path!(),
            "owned_arguments_survive_until_future_is_polled",
        ) {
            crate::test_process::TestProcess::ParentVerified => return,
            crate::test_process::TestProcess::Child => {}
        }
        let _scope = CommonServicesScope::new();
        let init_calls = register_successful_game_client_fixture();
        let future = {
            let source = String::from("game\0name");
            game_main(vec![
                source.clone(),
                "-max-runtime".to_owned(),
                "0".to_owned(),
            ])
        };
        assert!(pollster::block_on(future).is_ok());
        assert_eq!(init_calls.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(get_game_engine().is_none());
        clear_game_client_factory();
    }

    #[test]
    fn empty_argument_vector_is_safe() {
        match crate::test_process::run_bounded(module_path!(), "empty_argument_vector_is_safe") {
            crate::test_process::TestProcess::ParentVerified => return,
            crate::test_process::TestProcess::Child => {}
        }
        let _scope = CommonServicesScope::new();
        clear_game_client_factory();
        let error = pollster::block_on(game_main(Vec::new())).unwrap_err();
        assert!(error.to_string().contains("No game client factory registered"));
        assert!(get_game_engine().is_none());
    }

    #[test]
    fn test_game_main_minimal() {
        match crate::test_process::run_bounded(module_path!(), "test_game_main_minimal") {
            crate::test_process::TestProcess::ParentVerified => return,
            crate::test_process::TestProcess::Child => {}
        }
        let _scope = CommonServicesScope::new();
        pollster::block_on(async {
            let init_calls = register_successful_game_client_fixture();
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
            assert_eq!(
                init_calls.load(std::sync::atomic::Ordering::Relaxed),
                1,
                "the registered fixture must receive actual GameEngine initialization"
            );
            clear_game_client_factory();
        });
    }

    #[test]
    fn test_game_main_with_args() {
        match crate::test_process::run_bounded(module_path!(), "test_game_main_with_args") {
            crate::test_process::TestProcess::ParentVerified => return,
            crate::test_process::TestProcess::Child => {}
        }
        let _scope = CommonServicesScope::new();
        pollster::block_on(async {
            let init_calls = register_successful_game_client_fixture();
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
            assert_eq!(
                init_calls.load(std::sync::atomic::Ordering::Relaxed),
                1,
                "the registered fixture must receive actual GameEngine initialization"
            );
            clear_game_client_factory();
        });
    }

    #[test]
    fn test_game_engine_global_slot_can_be_cleared() {
        match crate::test_process::run_bounded(
            module_path!(),
            "test_game_engine_global_slot_can_be_cleared",
        ) {
            crate::test_process::TestProcess::ParentVerified => return,
            crate::test_process::TestProcess::Child => {}
        }
        let _scope = CommonServicesScope::new();
        clear_game_engine();
        let _engine = crate::common::game_engine::set_game_engine(create_game_engine());
        assert!(get_game_engine().is_some());
        clear_game_engine();
        assert!(get_game_engine().is_none());
    }
}
