//! Typed rejection at a requested native world boundary, before its effects.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NativeBoundaryFailure {
    InstalledWorld(String),
    RetainedCandidate(String),
}
impl std::fmt::Display for NativeBoundaryFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InstalledWorld(reason) | Self::RetainedCandidate(reason) => {
                formatter.write_str(reason)
            }
        }
    }
}
impl std::error::Error for NativeBoundaryFailure {}

impl CnCGameEngine {
    pub(super) fn require_native_world_boundary(
        &mut self,
        operation: &str,
    ) -> std::result::Result<(), NativeBoundaryFailure> {
        let failure = if let Some(reason) = self.game_logic.object_retirement_error() {
            Some(NativeBoundaryFailure::InstalledWorld(reason.to_string()))
        } else {
            self.rejected_native_worlds
                .validate_admission()
                .err()
                .map(NativeBoundaryFailure::RetainedCandidate)
        };
        match failure {
            None => Ok(()),
            Some(failure) => {
                error!("Rejecting {operation}: {failure}; retaining failed native owner");
                // Normal Exiting may hide the loading overlay. It performs no
                // gameplay reset, menu admission, or native destructor retry.
                self.transition_to_state(GameState::Exiting);
                Err(failure)
            }
        }
    }
}
