//! Connection state management and transitions
//!
//! This module provides state machine functionality for managing
//! connection lifecycle states and valid transitions.

use crate::error::{NetworkError, NetworkResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Extended connection state with detailed substates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum DetailedConnectionState {
    // Disconnected states
    /// Initial state - never connected
    Uninitialized = 0,
    /// Previously connected, now disconnected
    Disconnected = 1,
    /// Disconnected due to error
    DisconnectedError = 2,
    /// Intentionally disconnected
    DisconnectedIntentional = 3,

    // Connecting states
    /// Starting connection process
    ConnectingInitiate = 10,
    /// DNS resolution in progress
    ConnectingResolving = 11,
    /// TCP connection establishment
    ConnectingTcp = 12,
    /// Protocol handshake
    ConnectingHandshake = 13,

    // Connected states
    /// Basic connection established
    Connected = 20,
    /// Authentication in progress
    Authenticating = 21,
    /// Successfully authenticated
    Authenticated = 22,
    /// Loading game resources
    Loading = 23,
    /// Ready for game
    Ready = 24,
    /// Actively in game
    InGame = 25,

    // Disconnecting states
    /// Starting graceful disconnect
    DisconnectingGraceful = 30,
    /// Waiting for acknowledgments
    DisconnectingWaitAck = 31,
    /// Cleaning up resources
    DisconnectingCleanup = 32,
    /// Forced disconnect
    DisconnectingForced = 33,

    // Error states
    /// Network error occurred
    ErrorNetwork = 40,
    /// Authentication failed
    ErrorAuthentication = 41,
    /// Protocol error
    ErrorProtocol = 42,
    /// Timeout error
    ErrorTimeout = 43,
    /// Resource exhaustion
    ErrorResources = 44,
}

impl DetailedConnectionState {
    /// Check if state represents a connected condition
    pub fn is_connected(&self) -> bool {
        matches!(
            self,
            Self::Connected
                | Self::Authenticating
                | Self::Authenticated
                | Self::Loading
                | Self::Ready
                | Self::InGame
        )
    }

    /// Check if state represents a disconnected condition
    pub fn is_disconnected(&self) -> bool {
        matches!(
            self,
            Self::Uninitialized
                | Self::Disconnected
                | Self::DisconnectedError
                | Self::DisconnectedIntentional
        )
    }

    /// Check if state represents an error condition
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            Self::DisconnectedError
                | Self::ErrorNetwork
                | Self::ErrorAuthentication
                | Self::ErrorProtocol
                | Self::ErrorTimeout
                | Self::ErrorResources
        )
    }

    /// Check if state allows game activity
    pub fn allows_game_activity(&self) -> bool {
        matches!(self, Self::Ready | Self::InGame)
    }

    /// Check if state is transitioning
    pub fn is_transitioning(&self) -> bool {
        matches!(
            self,
            Self::ConnectingInitiate
                | Self::ConnectingResolving
                | Self::ConnectingTcp
                | Self::ConnectingHandshake
                | Self::Authenticating
                | Self::Loading
                | Self::DisconnectingGraceful
                | Self::DisconnectingWaitAck
                | Self::DisconnectingCleanup
                | Self::DisconnectingForced
        )
    }

    /// Get state category
    pub fn category(&self) -> StateCategory {
        match self {
            Self::Uninitialized
            | Self::Disconnected
            | Self::DisconnectedError
            | Self::DisconnectedIntentional => StateCategory::Disconnected,

            Self::ConnectingInitiate
            | Self::ConnectingResolving
            | Self::ConnectingTcp
            | Self::ConnectingHandshake => StateCategory::Connecting,

            Self::Connected
            | Self::Authenticating
            | Self::Authenticated
            | Self::Loading
            | Self::Ready
            | Self::InGame => StateCategory::Connected,

            Self::DisconnectingGraceful
            | Self::DisconnectingWaitAck
            | Self::DisconnectingCleanup
            | Self::DisconnectingForced => StateCategory::Disconnecting,

            Self::ErrorNetwork
            | Self::ErrorAuthentication
            | Self::ErrorProtocol
            | Self::ErrorTimeout
            | Self::ErrorResources => StateCategory::Error,
        }
    }
}

impl Default for DetailedConnectionState {
    fn default() -> Self {
        Self::Uninitialized
    }
}

impl fmt::Display for DetailedConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Uninitialized => "Uninitialized",
            Self::Disconnected => "Disconnected",
            Self::DisconnectedError => "DisconnectedError",
            Self::DisconnectedIntentional => "DisconnectedIntentional",
            Self::ConnectingInitiate => "ConnectingInitiate",
            Self::ConnectingResolving => "ConnectingResolving",
            Self::ConnectingTcp => "ConnectingTcp",
            Self::ConnectingHandshake => "ConnectingHandshake",
            Self::Connected => "Connected",
            Self::Authenticating => "Authenticating",
            Self::Authenticated => "Authenticated",
            Self::Loading => "Loading",
            Self::Ready => "Ready",
            Self::InGame => "InGame",
            Self::DisconnectingGraceful => "DisconnectingGraceful",
            Self::DisconnectingWaitAck => "DisconnectingWaitAck",
            Self::DisconnectingCleanup => "DisconnectingCleanup",
            Self::DisconnectingForced => "DisconnectingForced",
            Self::ErrorNetwork => "ErrorNetwork",
            Self::ErrorAuthentication => "ErrorAuthentication",
            Self::ErrorProtocol => "ErrorProtocol",
            Self::ErrorTimeout => "ErrorTimeout",
            Self::ErrorResources => "ErrorResources",
        };
        write!(f, "{}", name)
    }
}

/// State category for grouping related states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateCategory {
    /// Connection is not established and not attempting to connect
    Disconnected,
    /// Currently attempting to establish a connection
    Connecting,
    /// Connection is active and functional
    Connected,
    /// Currently terminating an existing connection
    Disconnecting,
    /// Connection encountered an error state
    Error,
}

/// State transition reason
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionReason {
    /// User initiated action
    UserAction,
    /// Network event
    NetworkEvent,
    /// Protocol requirement
    Protocol,
    /// Timeout occurred
    Timeout,
    /// Error condition
    Error(String),
    /// System shutdown
    Shutdown,
    /// Resource cleanup
    Cleanup,
}

impl fmt::Display for TransitionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserAction => write!(f, "User Action"),
            Self::NetworkEvent => write!(f, "Network Event"),
            Self::Protocol => write!(f, "Protocol"),
            Self::Timeout => write!(f, "Timeout"),
            Self::Error(msg) => write!(f, "Error: {}", msg),
            Self::Shutdown => write!(f, "Shutdown"),
            Self::Cleanup => write!(f, "Cleanup"),
        }
    }
}

/// State transition record
#[derive(Debug, Clone)]
pub struct StateTransition {
    /// Previous state
    pub from_state: DetailedConnectionState,
    /// New state
    pub to_state: DetailedConnectionState,
    /// Reason for transition
    pub reason: TransitionReason,
    /// When transition occurred
    pub timestamp: DateTime<Utc>,
    /// Additional context
    pub context: Option<String>,
}

impl StateTransition {
    /// Create new state transition
    pub fn new(
        from_state: DetailedConnectionState,
        to_state: DetailedConnectionState,
        reason: TransitionReason,
    ) -> Self {
        Self {
            from_state,
            to_state,
            reason,
            timestamp: Utc::now(),
            context: None,
        }
    }

    /// Add context to transition
    pub fn with_context<S: Into<String>>(mut self, context: S) -> Self {
        self.context = Some(context.into());
        self
    }
}

/// Connection state machine
pub struct ConnectionStateMachine {
    /// Current state
    current_state: DetailedConnectionState,
    /// State entry time
    state_entered: DateTime<Utc>,
    /// Transition history (limited size)
    transition_history: Vec<StateTransition>,
    /// Maximum history size
    max_history: usize,
}

impl ConnectionStateMachine {
    /// Create new state machine
    pub fn new() -> Self {
        Self {
            current_state: DetailedConnectionState::Uninitialized,
            state_entered: Utc::now(),
            transition_history: Vec::new(),
            max_history: 100,
        }
    }

    /// Get current state
    pub fn current_state(&self) -> DetailedConnectionState {
        self.current_state
    }

    /// Get time in current state
    pub fn time_in_state(&self) -> Duration {
        Utc::now()
            .signed_duration_since(self.state_entered)
            .to_std()
            .unwrap_or_default()
    }

    /// Attempt to transition to new state
    pub fn transition_to(
        &mut self,
        new_state: DetailedConnectionState,
        reason: TransitionReason,
    ) -> NetworkResult<()> {
        // Validate transition
        if !self.is_valid_transition(self.current_state, new_state) {
            return Err(NetworkError::invalid_state(format!(
                "invalid state transition: {} -> {}",
                self.current_state, new_state
            )));
        }

        // Record transition
        let transition = StateTransition::new(self.current_state, new_state, reason);

        // Update state
        let old_state = self.current_state;
        self.current_state = new_state;
        self.state_entered = Utc::now();

        // Add to history
        self.transition_history.push(transition);
        if self.transition_history.len() > self.max_history {
            self.transition_history.remove(0);
        }

        debug!(
            "State transition: {} -> {} (reason: {})",
            old_state,
            new_state,
            self.transition_history.last().unwrap().reason
        );

        Ok(())
    }

    /// Force transition (bypasses validation)
    pub fn force_transition_to(
        &mut self,
        new_state: DetailedConnectionState,
        reason: TransitionReason,
    ) {
        warn!(
            "Forced state transition: {} -> {} (reason: {})",
            self.current_state, new_state, reason
        );

        let transition = StateTransition::new(self.current_state, new_state, reason)
            .with_context("forced transition");

        self.current_state = new_state;
        self.state_entered = Utc::now();
        self.transition_history.push(transition);

        if self.transition_history.len() > self.max_history {
            self.transition_history.remove(0);
        }
    }

    /// Check if transition is valid
    fn is_valid_transition(
        &self,
        from: DetailedConnectionState,
        to: DetailedConnectionState,
    ) -> bool {
        use DetailedConnectionState::*;

        // Allow any transition to error states
        if to.is_error() {
            return true;
        }

        // Allow transitions to disconnecting states from most states
        if matches!(
            to,
            DisconnectingGraceful | DisconnectingWaitAck | DisconnectingForced
        ) && from.is_connected()
        {
            return true;
        }

        match (from, to) {
            // Initial connections
            (Uninitialized, ConnectingInitiate) => true,
            (Disconnected, ConnectingInitiate) => true,
            (DisconnectedError, ConnectingInitiate) => true,
            (DisconnectedIntentional, ConnectingInitiate) => true,

            // Connection sequence
            (ConnectingInitiate, ConnectingResolving) => true,
            (ConnectingResolving, ConnectingTcp) => true,
            (ConnectingTcp, ConnectingHandshake) => true,
            (ConnectingHandshake, Connected) => true,

            // Authentication flow
            (Connected, Authenticating) => true,
            (Authenticating, Authenticated) => true,

            // Game preparation
            (Authenticated, Loading) => true,
            (Loading, Ready) => true,
            (Ready, InGame) => true,

            // Game state changes
            (InGame, Ready) => true,
            (Ready, Loading) => true,

            // Disconnection sequence
            (DisconnectingGraceful, DisconnectingWaitAck) => true,
            (DisconnectingWaitAck, DisconnectingCleanup) => true,
            (DisconnectingCleanup, Disconnected) => true,
            (DisconnectingForced, Disconnected) => true,

            // Error recovery
            (ErrorNetwork, ConnectingInitiate) => true,
            (ErrorTimeout, ConnectingInitiate) => true,
            (ErrorAuthentication, Disconnected) => true,
            (ErrorProtocol, Disconnected) => true,
            (ErrorResources, Disconnected) => true,

            // Same state (no-op)
            (a, b) if a == b => true,

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Get transition history
    pub fn get_history(&self) -> &[StateTransition] {
        &self.transition_history
    }

    /// Get recent transitions (last N)
    pub fn get_recent_transitions(&self, count: usize) -> &[StateTransition] {
        let start = self.transition_history.len().saturating_sub(count);
        &self.transition_history[start..]
    }

    /// Check if state has been stable for minimum duration
    pub fn is_stable(&self, min_duration: Duration) -> bool {
        self.time_in_state() >= min_duration
    }

    /// Get state statistics
    pub fn get_state_stats(&self) -> StateStats {
        let mut state_counts = std::collections::HashMap::new();
        let mut total_transitions = 0;
        let mut error_transitions = 0;

        for transition in &self.transition_history {
            *state_counts.entry(transition.to_state).or_insert(0) += 1;
            total_transitions += 1;

            if matches!(transition.reason, TransitionReason::Error(_)) {
                error_transitions += 1;
            }
        }

        StateStats {
            current_state: self.current_state,
            time_in_current_state: self.time_in_state(),
            total_transitions,
            error_transitions,
            most_common_state: state_counts
                .iter()
                .max_by_key(|(_, count)| *count)
                .map(|(state, _)| *state),
        }
    }

    /// Reset state machine
    pub fn reset(&mut self) {
        info!("Resetting connection state machine");

        self.current_state = DetailedConnectionState::Uninitialized;
        self.state_entered = Utc::now();
        self.transition_history.clear();
    }
}

impl Default for ConnectionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// State machine statistics
#[derive(Debug, Clone)]
pub struct StateStats {
    /// Current state of the connection
    pub current_state: DetailedConnectionState,
    /// Duration spent in the current state
    pub time_in_current_state: Duration,
    /// Total number of state transitions recorded
    pub total_transitions: usize,
    /// Number of transitions caused by errors
    pub error_transitions: usize,
    /// State that has been visited most frequently
    pub most_common_state: Option<DetailedConnectionState>,
}

/// State machine events for external monitoring
#[derive(Debug, Clone)]
pub enum StateEvent {
    /// State was entered
    StateEntered {
        /// The new state that was entered
        state: DetailedConnectionState,
        /// The previous state before transition
        previous: DetailedConnectionState,
        /// Reason for the state transition
        reason: TransitionReason,
    },
    /// State transition failed
    TransitionFailed {
        /// State transition was attempted from
        from: DetailedConnectionState,
        /// State transition was attempted to
        to: DetailedConnectionState,
        /// Reason why the transition failed
        reason: String,
    },
    /// State has been stable for a duration
    StateStable {
        /// The stable state
        state: DetailedConnectionState,
        /// How long the state has been stable
        duration: Duration,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_properties() {
        assert!(DetailedConnectionState::InGame.is_connected());
        assert!(DetailedConnectionState::Disconnected.is_disconnected());
        assert!(DetailedConnectionState::ErrorNetwork.is_error());
        assert!(DetailedConnectionState::Ready.allows_game_activity());
        assert!(DetailedConnectionState::ConnectingTcp.is_transitioning());
    }

    #[test]
    fn test_state_categories() {
        assert_eq!(
            DetailedConnectionState::InGame.category(),
            StateCategory::Connected
        );
        assert_eq!(
            DetailedConnectionState::ConnectingTcp.category(),
            StateCategory::Connecting
        );
        assert_eq!(
            DetailedConnectionState::ErrorNetwork.category(),
            StateCategory::Error
        );
    }

    #[test]
    fn test_state_machine_creation() {
        let sm = ConnectionStateMachine::new();
        assert_eq!(sm.current_state(), DetailedConnectionState::Uninitialized);
        assert!(sm.time_in_state() < Duration::from_millis(100));
    }

    #[test]
    fn test_valid_transitions() {
        let mut sm = ConnectionStateMachine::new();

        // Valid connection sequence
        assert!(sm
            .transition_to(
                DetailedConnectionState::ConnectingInitiate,
                TransitionReason::UserAction
            )
            .is_ok());

        assert!(sm
            .transition_to(
                DetailedConnectionState::ConnectingResolving,
                TransitionReason::Protocol
            )
            .is_ok());

        assert!(sm
            .transition_to(
                DetailedConnectionState::ConnectingTcp,
                TransitionReason::Protocol
            )
            .is_ok());

        assert_eq!(sm.current_state(), DetailedConnectionState::ConnectingTcp);
        assert_eq!(sm.transition_history.len(), 3);
    }

    #[test]
    fn test_invalid_transitions() {
        let mut sm = ConnectionStateMachine::new();

        // Invalid: can't go directly from Uninitialized to InGame
        assert!(sm
            .transition_to(
                DetailedConnectionState::InGame,
                TransitionReason::UserAction
            )
            .is_err());

        // State should remain unchanged
        assert_eq!(sm.current_state(), DetailedConnectionState::Uninitialized);
        assert_eq!(sm.transition_history.len(), 0);
    }

    #[test]
    fn test_error_transitions() {
        let mut sm = ConnectionStateMachine::new();

        // Transition to connecting state following the valid sequence
        sm.transition_to(
            DetailedConnectionState::ConnectingInitiate,
            TransitionReason::UserAction,
        )
        .unwrap();
        sm.transition_to(
            DetailedConnectionState::ConnectingResolving,
            TransitionReason::Protocol,
        )
        .unwrap();
        sm.transition_to(
            DetailedConnectionState::ConnectingTcp,
            TransitionReason::Protocol,
        )
        .unwrap();

        // Error transitions should always be allowed
        assert!(sm
            .transition_to(
                DetailedConnectionState::ErrorNetwork,
                TransitionReason::Error("Connection failed".to_string())
            )
            .is_ok());

        assert_eq!(sm.current_state(), DetailedConnectionState::ErrorNetwork);
    }

    #[test]
    fn test_force_transition() {
        let mut sm = ConnectionStateMachine::new();

        // This would normally be invalid
        sm.force_transition_to(
            DetailedConnectionState::InGame,
            TransitionReason::Error("Test force".to_string()),
        );

        assert_eq!(sm.current_state(), DetailedConnectionState::InGame);
        assert_eq!(sm.transition_history.len(), 1);
    }

    #[test]
    fn test_state_stats() {
        let mut sm = ConnectionStateMachine::new();

        // Make several transitions
        sm.transition_to(
            DetailedConnectionState::ConnectingInitiate,
            TransitionReason::UserAction,
        )
        .unwrap();
        sm.transition_to(
            DetailedConnectionState::ConnectingResolving,
            TransitionReason::Protocol,
        )
        .unwrap();
        sm.transition_to(
            DetailedConnectionState::ConnectingTcp,
            TransitionReason::Protocol,
        )
        .unwrap();
        sm.transition_to(
            DetailedConnectionState::ConnectingHandshake,
            TransitionReason::Protocol,
        )
        .unwrap();
        sm.transition_to(
            DetailedConnectionState::Connected,
            TransitionReason::Protocol,
        )
        .unwrap();

        let stats = sm.get_state_stats();
        assert_eq!(stats.current_state, DetailedConnectionState::Connected);
        assert_eq!(stats.total_transitions, 5);
        assert_eq!(stats.error_transitions, 0);
    }

    #[test]
    fn test_transition_history() {
        let mut sm = ConnectionStateMachine::new();

        sm.transition_to(
            DetailedConnectionState::ConnectingInitiate,
            TransitionReason::UserAction,
        )
        .unwrap();
        sm.transition_to(
            DetailedConnectionState::ConnectingResolving,
            TransitionReason::Protocol,
        )
        .unwrap();
        sm.transition_to(
            DetailedConnectionState::ConnectingTcp,
            TransitionReason::Protocol,
        )
        .unwrap();
        sm.transition_to(
            DetailedConnectionState::ConnectingHandshake,
            TransitionReason::Protocol,
        )
        .unwrap();
        sm.transition_to(
            DetailedConnectionState::Connected,
            TransitionReason::Protocol,
        )
        .unwrap();

        let history = sm.get_history();
        assert_eq!(history.len(), 5);
        assert_eq!(
            history[0].to_state,
            DetailedConnectionState::ConnectingInitiate
        );
        assert_eq!(history[4].to_state, DetailedConnectionState::Connected);

        let recent = sm.get_recent_transitions(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].to_state, DetailedConnectionState::Connected);
    }
}
