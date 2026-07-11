/*!
 * Standalone Demo - Command & Conquer Generals Zero Hour Rust Port
 *
 * This standalone example demonstrates the architectural patterns and
 * concepts that have been implemented in the GameLogic 2025 Rust conversion.
 *
 * This file is completely standalone and doesn't depend on the main gamelogic
 * crate, allowing it to compile and run independently.
 */

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex, RwLock};

// Type definitions similar to those in the actual conversion
type ObjectID = u32;
type PlayerId = u8;
type Real = f32;
type Coord3D = [f32; 3];

// Error handling similar to the actual conversion
#[derive(Debug)]
enum GameLogicError {
    ObjectNotFound(ObjectID),
    InvalidPosition,
    SystemError(String),
}

type GameLogicResult<T> = Result<T, GameLogicError>;

// Simplified game object similar to the converted Object struct
#[derive(Debug)]
struct GameObject {
    id: ObjectID,
    position: Coord3D,
    health: Real,
    max_health: Real,
    owner: PlayerId,
    object_type: String,
}

impl GameObject {
    fn new(id: ObjectID, position: Coord3D, owner: PlayerId, object_type: String) -> Self {
        Self {
            id,
            position,
            health: 100.0,
            max_health: 100.0,
            owner,
            object_type,
        }
    }

    fn get_position(&self) -> &Coord3D {
        &self.position
    }

    fn get_health(&self) -> Real {
        self.health
    }

    fn take_damage(&mut self, amount: Real) -> GameLogicResult<()> {
        if amount < 0.0 {
            return Err(GameLogicError::InvalidPosition);
        }

        self.health = (self.health - amount).max(0.0);
        Ok(())
    }

    fn is_alive(&self) -> bool {
        self.health > 0.0
    }
}

// Game state manager similar to the actual GameLogic struct
#[derive(Debug)]
struct GameState {
    objects: HashMap<ObjectID, GameObject>,
    next_id: ObjectID,
    frame_count: u64,
}

impl GameState {
    fn new() -> Self {
        Self {
            objects: HashMap::new(),
            next_id: 1,
            frame_count: 0,
        }
    }

    fn create_object(
        &mut self,
        position: Coord3D,
        owner: PlayerId,
        object_type: String,
    ) -> ObjectID {
        let id = self.next_id;
        self.next_id += 1;

        let obj = GameObject::new(id, position, owner, object_type);
        self.objects.insert(id, obj);

        println!(
            "Created object {} at position [{:.1}, {:.1}, {:.1}]",
            id, position[0], position[1], position[2]
        );
        id
    }

    fn get_object(&self, id: ObjectID) -> GameLogicResult<&GameObject> {
        self.objects
            .get(&id)
            .ok_or(GameLogicError::ObjectNotFound(id))
    }

    fn get_object_mut(&mut self, id: ObjectID) -> GameLogicResult<&mut GameObject> {
        self.objects
            .get_mut(&id)
            .ok_or(GameLogicError::ObjectNotFound(id))
    }

    fn update_frame(&mut self) {
        self.frame_count += 1;

        // Remove dead objects
        self.objects.retain(|_, obj| obj.is_alive());

        if self.frame_count.is_multiple_of(60) {
            println!(
                "Frame {}: {} objects alive",
                self.frame_count,
                self.objects.len()
            );
        }
    }

    fn save_state(&self) -> String {
        format!(
            "GameState(frame={}, objects={})",
            self.frame_count,
            self.objects.len()
        )
    }

    fn get_objects_near(&self, center: &Coord3D, radius: Real) -> Vec<ObjectID> {
        self.objects
            .iter()
            .filter(|(_, obj)| {
                let pos = obj.get_position();
                let dx = pos[0] - center[0];
                let dy = pos[1] - center[1];
                let dz = pos[2] - center[2];
                (dx * dx + dy * dy + dz * dz).sqrt() <= radius
            })
            .map(|(id, _)| *id)
            .collect()
    }
}

// Thread-safe game manager demonstrating Arc<Mutex<T>> patterns
#[derive(Clone)]
struct GameManager {
    state: Arc<RwLock<GameState>>,
    running: Arc<Mutex<bool>>,
}

impl GameManager {
    fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(GameState::new())),
            running: Arc::new(Mutex::new(false)),
        }
    }

    fn start(&self) -> GameLogicResult<()> {
        if let Ok(mut running) = self.running.lock() {
            *running = true;
            println!("Game manager started");
            Ok(())
        } else {
            Err(GameLogicError::SystemError(
                "Failed to acquire lock".to_string(),
            ))
        }
    }

    fn stop(&self) -> GameLogicResult<()> {
        if let Ok(mut running) = self.running.lock() {
            *running = false;
            println!("Game manager stopped");
            Ok(())
        } else {
            Err(GameLogicError::SystemError(
                "Failed to acquire lock".to_string(),
            ))
        }
    }

    fn create_object(
        &self,
        position: Coord3D,
        owner: PlayerId,
        object_type: String,
    ) -> GameLogicResult<ObjectID> {
        if let Ok(mut state) = self.state.write() {
            Ok(state.create_object(position, owner, object_type))
        } else {
            Err(GameLogicError::SystemError(
                "Failed to acquire write lock".to_string(),
            ))
        }
    }

    fn damage_object(&self, id: ObjectID, damage: Real) -> GameLogicResult<()> {
        if let Ok(mut state) = self.state.write() {
            let obj = state.get_object_mut(id)?;
            obj.take_damage(damage)?;
            println!(
                "Object {} took {} damage, health now {:.1}",
                id, damage, obj.health
            );
            Ok(())
        } else {
            Err(GameLogicError::SystemError(
                "Failed to acquire write lock".to_string(),
            ))
        }
    }

    fn update(&self) -> GameLogicResult<()> {
        if let Ok(mut state) = self.state.write() {
            state.update_frame();
            Ok(())
        } else {
            Err(GameLogicError::SystemError(
                "Failed to acquire write lock".to_string(),
            ))
        }
    }

    fn get_state_info(&self) -> GameLogicResult<String> {
        if let Ok(state) = self.state.read() {
            Ok(state.save_state())
        } else {
            Err(GameLogicError::SystemError(
                "Failed to acquire read lock".to_string(),
            ))
        }
    }
}

fn main() {
    println!("GameLogic 2025 - Standalone Architecture Demo");
    println!("==============================================");

    let game = GameManager::new();

    // Start the game
    if let Err(e) = game.start() {
        eprintln!("Failed to start game: {:?}", e);
        return;
    }

    // Create some objects
    println!("\n1. Creating game objects:");
    let tank_id = game
        .create_object([100.0, 100.0, 0.0], 1, "Tank".to_string())
        .unwrap();
    let infantry_id = game
        .create_object([105.0, 95.0, 0.0], 1, "Infantry".to_string())
        .unwrap();
    let enemy_tank_id = game
        .create_object([200.0, 200.0, 0.0], 2, "EnemyTank".to_string())
        .unwrap();

    // Show initial state
    println!("\n2. Initial game state:");
    println!("  {}", game.get_state_info().unwrap());

    // Simulate combat
    println!("\n3. Simulating combat:");
    game.damage_object(tank_id, 25.0).unwrap();
    game.damage_object(infantry_id, 80.0).unwrap();
    game.damage_object(enemy_tank_id, 120.0).unwrap(); // This should destroy it

    // Update game state
    println!("\n4. Updating game state:");
    for i in 0..5 {
        game.update().unwrap();
        if i == 2 {
            println!("  Mid-update state: {}", game.get_state_info().unwrap());
        }
    }

    // Final state
    println!("\n5. Final game state:");
    println!("  {}", game.get_state_info().unwrap());

    // Demonstrate spatial queries
    println!("\n6. Spatial query demo:");
    if let Ok(state) = game.state.read() {
        let near_objects = state.get_objects_near(&[100.0, 100.0, 0.0], 20.0);
        println!(
            "  Objects near [100, 100, 0] within radius 20: {:?}",
            near_objects
        );
    }

    // Stop the game
    game.stop().unwrap();

    println!("\n=== Architecture Patterns Demonstrated ===");
    println!("✓ Type system conversion (C++ typedefs → Rust type aliases)");
    println!("✓ Error handling (C++ exceptions → Rust Result<T, E>)");
    println!("✓ Memory management (C++ pointers → Rust Arc<Mutex<T>>)");
    println!("✓ Thread safety (C++ locks → Rust RwLock/Mutex)");
    println!("✓ Collections (C++ STL → Rust HashMap/Vec)");
    println!("✓ Object lifecycle management");
    println!("✓ State serialization patterns");
    println!("✓ Spatial data operations");

    println!("\nThis demonstrates the foundational architecture that");
    println!("the full GameLogic 2025 conversion is built upon.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_object_creation() {
        let obj = GameObject::new(1, [0.0, 0.0, 0.0], 1, "Test".to_string());
        assert_eq!(obj.id, 1);
        assert_eq!(obj.health, 100.0);
        assert!(obj.is_alive());
    }

    #[test]
    fn test_game_object_damage() {
        let mut obj = GameObject::new(1, [0.0, 0.0, 0.0], 1, "Test".to_string());
        obj.take_damage(50.0).unwrap();
        assert_eq!(obj.health, 50.0);
        assert!(obj.is_alive());

        obj.take_damage(60.0).unwrap();
        assert_eq!(obj.health, 0.0);
        assert!(!obj.is_alive());
    }

    #[test]
    fn test_game_state_management() {
        let mut state = GameState::new();
        let id1 = state.create_object([0.0, 0.0, 0.0], 1, "Test1".to_string());
        let id2 = state.create_object([10.0, 10.0, 0.0], 2, "Test2".to_string());

        assert!(state.get_object(id1).is_ok());
        assert!(state.get_object(id2).is_ok());

        // Test spatial query
        let near = state.get_objects_near(&[0.0, 0.0, 0.0], 5.0);
        assert_eq!(near.len(), 1);
        assert_eq!(near[0], id1);
    }

    #[test]
    fn test_thread_safe_game_manager() {
        let game = GameManager::new();
        game.start().unwrap();

        let id = game
            .create_object([0.0, 0.0, 0.0], 1, "Test".to_string())
            .unwrap();
        game.damage_object(id, 25.0).unwrap();
        game.update().unwrap();

        let state_info = game.get_state_info().unwrap();
        assert!(state_info.contains("GameState"));

        game.stop().unwrap();
    }
}
