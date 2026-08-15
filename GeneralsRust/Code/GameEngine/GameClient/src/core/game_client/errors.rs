// GameClient errors and startup-movie action helpers.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

/// Error types for GameClient operations
#[derive(Debug, thiserror::Error)]
pub enum GameClientError {
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Subsystem error: {0}")]
    SubsystemError(String),

    #[error("Drawable not found: {0:?}")]
    DrawableNotFound(DrawableId),

    #[error("Resource loading failed: {0}")]
    ResourceLoadingFailed(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Memory allocation failed")]
    OutOfMemory,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Generic error: {0}")]
    GenericError(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupMovieAction {
    PlayLogo(&'static str),
    PlaySizzle(&'static str),
    FinalizeStartup,
}

fn startup_movie_action(
    play_intro: bool,
    after_intro: bool,
    play_sizzle: bool,
    startup_sizzle_pending: bool,
    low_res_movies: bool,
) -> Option<StartupMovieAction> {
    if play_intro {
        return Some(StartupMovieAction::PlayLogo(if low_res_movies {
            "EALogoMovie640"
        } else {
            "EALogoMovie"
        }));
    }

    if !after_intro {
        return None;
    }

    if startup_sizzle_pending && play_sizzle {
        return Some(StartupMovieAction::PlaySizzle(if low_res_movies {
            "Sizzle640"
        } else {
            "Sizzle"
        }));
    }

    Some(StartupMovieAction::FinalizeStartup)
}

impl From<Box<dyn std::error::Error>> for GameClientError {
    fn from(error: Box<dyn std::error::Error>) -> Self {
        // Convert the error to a string and create a Send + Sync box
        let error_string = error.to_string();
        let sendable_error: Box<dyn std::error::Error + Send + Sync> =
            Box::new(std::io::Error::other(error_string));
        GameClientError::GenericError(sendable_error)
    }
}
