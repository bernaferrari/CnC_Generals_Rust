//! Integration Test: AI State Machine Transitions
//!
//! This test verifies that AI units transition correctly between states:
//! - Idle state
//! - Moving state
//! - Attacking state
//! - Retreating state
//! - Guarding state
//! - State transition logic
//! - Event-driven state changes
//!
//! Tests should pass on all platforms (Windows, Linux, macOS)

#![cfg(test)]

/// AI States matching C&C Generals
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiState {
    Idle,
    Moving,
    Attacking,
    Retreating,
    Guarding,
    Dead,
}

/// AI Events that trigger state transitions
#[derive(Debug, Clone, Copy, PartialEq)]
enum AiEvent {
    MoveCommand,
    AttackCommand,
    EnemySpotted,
    LowHealth,
    NoEnemies,
    ReachedDestination,
    Died,
}

/// AI State Machine
#[derive(Debug)]
struct AiStateMachine {
    current_state: AiState,
    previous_state: Option<AiState>,
    state_time: f32,
    transitions: u32,
}

impl AiStateMachine {
    fn new() -> Self {
        Self {
            current_state: AiState::Idle,
            previous_state: None,
            state_time: 0.0,
            transitions: 0,
        }
    }

    fn transition_to(&mut self, new_state: AiState) {
        if self.current_state != new_state {
            self.previous_state = Some(self.current_state);
            self.current_state = new_state;
            self.state_time = 0.0;
            self.transitions += 1;
        }
    }

    fn handle_event(&mut self, event: AiEvent) {
        match (self.current_state, event) {
            // From Idle
            (AiState::Idle, AiEvent::MoveCommand) => self.transition_to(AiState::Moving),
            (AiState::Idle, AiEvent::AttackCommand) => self.transition_to(AiState::Attacking),
            (AiState::Idle, AiEvent::EnemySpotted) => self.transition_to(AiState::Attacking),
            (AiState::Idle, AiEvent::Died) => self.transition_to(AiState::Dead),

            // From Moving
            (AiState::Moving, AiEvent::ReachedDestination) => self.transition_to(AiState::Idle),
            (AiState::Moving, AiEvent::AttackCommand) => self.transition_to(AiState::Attacking),
            (AiState::Moving, AiEvent::EnemySpotted) => self.transition_to(AiState::Attacking),
            (AiState::Moving, AiEvent::LowHealth) => self.transition_to(AiState::Retreating),
            (AiState::Moving, AiEvent::Died) => self.transition_to(AiState::Dead),

            // From Attacking
            (AiState::Attacking, AiEvent::NoEnemies) => self.transition_to(AiState::Idle),
            (AiState::Attacking, AiEvent::LowHealth) => self.transition_to(AiState::Retreating),
            (AiState::Attacking, AiEvent::MoveCommand) => self.transition_to(AiState::Moving),
            (AiState::Attacking, AiEvent::Died) => self.transition_to(AiState::Dead),

            // From Retreating
            (AiState::Retreating, AiEvent::ReachedDestination) => self.transition_to(AiState::Idle),
            (AiState::Retreating, AiEvent::Died) => self.transition_to(AiState::Dead),

            // From Guarding
            (AiState::Guarding, AiEvent::EnemySpotted) => self.transition_to(AiState::Attacking),
            (AiState::Guarding, AiEvent::MoveCommand) => self.transition_to(AiState::Moving),
            (AiState::Guarding, AiEvent::Died) => self.transition_to(AiState::Dead),

            // Dead is final state
            (AiState::Dead, _) => {},

            // Ignore invalid transitions
            _ => {},
        }
    }

    fn update(&mut self, delta_time: f32) {
        self.state_time += delta_time;
    }

    fn get_state(&self) -> AiState {
        self.current_state
    }
}

#[test]
fn test_initial_state() {
    println!("Testing initial AI state...");

    let ai = AiStateMachine::new();

    assert_eq!(ai.get_state(), AiState::Idle);
    assert_eq!(ai.previous_state, None);
    assert_eq!(ai.transitions, 0);

    log::info!("Initial state test passed");
}

#[test]
fn test_idle_to_moving() {
    println!("Testing Idle -> Moving transition...");

    let mut ai = AiStateMachine::new();
    assert_eq!(ai.get_state(), AiState::Idle);

    ai.handle_event(AiEvent::MoveCommand);
    assert_eq!(ai.get_state(), AiState::Moving);
    assert_eq!(ai.previous_state, Some(AiState::Idle));
    assert_eq!(ai.transitions, 1);

    log::info!("Idle to Moving test passed");
}

#[test]
fn test_idle_to_attacking() {
    println!("Testing Idle -> Attacking transition...");

    let mut ai = AiStateMachine::new();

    ai.handle_event(AiEvent::EnemySpotted);
    assert_eq!(ai.get_state(), AiState::Attacking);

    log::info!("Idle to Attacking test passed");
}

#[test]
fn test_attacking_to_retreating() {
    println!("Testing Attacking -> Retreating transition...");

    let mut ai = AiStateMachine::new();

    ai.handle_event(AiEvent::AttackCommand);
    assert_eq!(ai.get_state(), AiState::Attacking);

    ai.handle_event(AiEvent::LowHealth);
    assert_eq!(ai.get_state(), AiState::Retreating);

    log::info!("Attacking to Retreating test passed");
}

#[test]
fn test_death_is_final() {
    println!("Testing death is final state...");

    let mut ai = AiStateMachine::new();

    ai.handle_event(AiEvent::Died);
    assert_eq!(ai.get_state(), AiState::Dead);

    // Try to transition out of death
    ai.handle_event(AiEvent::MoveCommand);
    assert_eq!(ai.get_state(), AiState::Dead, "Should remain dead");

    ai.handle_event(AiEvent::AttackCommand);
    assert_eq!(ai.get_state(), AiState::Dead, "Should remain dead");

    log::info!("Death is final test passed");
}

#[test]
fn test_moving_to_destination() {
    println!("Testing Moving -> Idle on destination...");

    let mut ai = AiStateMachine::new();

    ai.handle_event(AiEvent::MoveCommand);
    assert_eq!(ai.get_state(), AiState::Moving);

    ai.handle_event(AiEvent::ReachedDestination);
    assert_eq!(ai.get_state(), AiState::Idle);

    log::info!("Moving to destination test passed");
}

#[test]
fn test_state_time_tracking() {
    println!("Testing state time tracking...");

    let mut ai = AiStateMachine::new();

    ai.update(0.5);
    assert_eq!(ai.state_time, 0.5);

    ai.update(0.3);
    assert_eq!(ai.state_time, 0.8);

    // Transition should reset time
    ai.handle_event(AiEvent::MoveCommand);
    assert_eq!(ai.state_time, 0.0);

    log::info!("State time tracking test passed");
}

#[test]
fn test_transition_counting() {
    println!("Testing transition counting...");

    let mut ai = AiStateMachine::new();
    assert_eq!(ai.transitions, 0);

    ai.handle_event(AiEvent::MoveCommand);
    assert_eq!(ai.transitions, 1);

    ai.handle_event(AiEvent::EnemySpotted);
    assert_eq!(ai.transitions, 2);

    ai.handle_event(AiEvent::LowHealth);
    assert_eq!(ai.transitions, 3);

    log::info!("Transition counting test passed");
}

#[test]
fn test_complex_state_sequence() {
    println!("Testing complex state sequence...");

    let mut ai = AiStateMachine::new();

    // Idle
    assert_eq!(ai.get_state(), AiState::Idle);

    // Move out
    ai.handle_event(AiEvent::MoveCommand);
    assert_eq!(ai.get_state(), AiState::Moving);

    // Spot enemy while moving
    ai.handle_event(AiEvent::EnemySpotted);
    assert_eq!(ai.get_state(), AiState::Attacking);

    // Get damaged
    ai.handle_event(AiEvent::LowHealth);
    assert_eq!(ai.get_state(), AiState::Retreating);

    // Reach safe position
    ai.handle_event(AiEvent::ReachedDestination);
    assert_eq!(ai.get_state(), AiState::Idle);

    assert_eq!(ai.transitions, 4);

    log::info!("Complex state sequence test passed");
}

#[test]
fn test_invalid_transitions_ignored() {
    println!("Testing invalid transitions are ignored...");

    let mut ai = AiStateMachine::new();

    // Idle, try to reach destination (invalid)
    ai.handle_event(AiEvent::ReachedDestination);
    assert_eq!(ai.get_state(), AiState::Idle, "Should remain idle");
    assert_eq!(ai.transitions, 0);

    // Try NoEnemies while idle (invalid)
    ai.handle_event(AiEvent::NoEnemies);
    assert_eq!(ai.get_state(), AiState::Idle, "Should remain idle");

    log::info!("Invalid transitions test passed");
}

#[test]
fn test_multiple_units_independent_states() {
    println!("Testing multiple units with independent states...");

    let mut unit1 = AiStateMachine::new();
    let mut unit2 = AiStateMachine::new();
    let mut unit3 = AiStateMachine::new();

    unit1.handle_event(AiEvent::MoveCommand);
    unit2.handle_event(AiEvent::AttackCommand);
    unit3.handle_event(AiEvent::MoveCommand);

    assert_eq!(unit1.get_state(), AiState::Moving);
    assert_eq!(unit2.get_state(), AiState::Attacking);
    assert_eq!(unit3.get_state(), AiState::Moving);

    // Change unit2
    unit2.handle_event(AiEvent::LowHealth);
    assert_eq!(unit2.get_state(), AiState::Retreating);

    // Others unchanged
    assert_eq!(unit1.get_state(), AiState::Moving);
    assert_eq!(unit3.get_state(), AiState::Moving);

    log::info!("Multiple units independence test passed");
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    #[test]
    #[ignore] // Run with: cargo test --test integration_ai_statemachine -- --ignored
    fn test_rapid_state_transitions() {
        println!("Stress test: Rapid state transitions...");

        let mut ai = AiStateMachine::new();

        const NUM_TRANSITIONS: usize = 100000;
        let events = vec![
            AiEvent::MoveCommand,
            AiEvent::ReachedDestination,
            AiEvent::EnemySpotted,
            AiEvent::NoEnemies,
            AiEvent::AttackCommand,
            AiEvent::MoveCommand,
            AiEvent::ReachedDestination,
        ];

        let start = std::time::Instant::now();

        for i in 0..NUM_TRANSITIONS {
            let event = events[i % events.len()];
            ai.handle_event(event);
            ai.update(0.016); // 60 FPS
        }

        let elapsed = start.elapsed();
        let transitions_per_sec = NUM_TRANSITIONS as f64 / elapsed.as_secs_f64();

        println!("Processed {} transitions in {:?} ({:.0} trans/sec)",
            NUM_TRANSITIONS, elapsed, transitions_per_sec);

        assert!(transitions_per_sec > 100000.0, "Should handle >100k transitions/sec");

        log::info!("Rapid state transitions stress test passed");
    }

    #[test]
    #[ignore]
    fn test_many_concurrent_state_machines() {
        println!("Stress test: Many concurrent state machines...");

        const NUM_UNITS: usize = 10000;
        let mut units: Vec<AiStateMachine> = (0..NUM_UNITS)
            .map(|_| AiStateMachine::new())
            .collect();

        let events = vec![
            AiEvent::MoveCommand,
            AiEvent::AttackCommand,
            AiEvent::EnemySpotted,
        ];

        let start = std::time::Instant::now();

        // Update all units
        for (i, unit) in units.iter_mut().enumerate() {
            let event = events[i % events.len()];
            unit.handle_event(event);
            unit.update(0.016);
        }

        let elapsed = start.elapsed();
        let updates_per_sec = NUM_UNITS as f64 / elapsed.as_secs_f64();

        println!("Updated {} units in {:?} ({:.0} updates/sec)",
            NUM_UNITS, elapsed, updates_per_sec);

        assert!(updates_per_sec > 10000.0, "Should update >10k units/sec");

        log::info!("Many concurrent state machines stress test passed");
    }
}
