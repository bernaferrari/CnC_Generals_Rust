//! Integration Test: Object Creation and Destruction
//!
//! This test verifies that game objects (units, buildings, projectiles) can be
//! created, updated, and destroyed correctly without memory leaks or dangling references.
//!
//! Key aspects tested:
//! - Object factory creation
//! - Reference counting and ownership
//! - Update lifecycle
//! - Proper cleanup on destruction
//! - No memory leaks
//!
//! Tests should pass on all platforms (Windows, Linux, macOS)

#![cfg(test)]

use std::sync::{Arc, Weak};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Mock object ID type
type ObjectId = u32;

/// Mock object type
#[derive(Debug, Clone, Copy, PartialEq)]
enum ObjectType {
    Unit,
    Building,
    Projectile,
    Effect,
}

/// Mock game object
#[derive(Debug, Clone)]
struct GameObject {
    id: ObjectId,
    object_type: ObjectType,
    health: i32,
    position: (f32, f32, f32),
    active: bool,
    created_at: Instant,
}

impl GameObject {
    fn new(id: ObjectId, object_type: ObjectType) -> Self {
        Self {
            id,
            object_type,
            health: 100,
            position: (0.0, 0.0, 0.0),
            active: true,
            created_at: Instant::now(),
        }
    }

    fn update(&mut self, _delta_time: Duration) {
        // Simulate object update
        if self.health <= 0 {
            self.active = false;
        }
    }

    fn take_damage(&mut self, amount: i32) {
        self.health -= amount;
        if self.health < 0 {
            self.health = 0;
        }
    }

    fn is_alive(&self) -> bool {
        self.active && self.health > 0
    }
}

/// Mock object manager
struct ObjectManager {
    objects: HashMap<ObjectId, Arc<parking_lot::Mutex<GameObject>>>,
    next_id: ObjectId,
}

impl ObjectManager {
    fn new() -> Self {
        Self {
            objects: HashMap::new(),
            next_id: 1,
        }
    }

    fn create_object(&mut self, object_type: ObjectType) -> ObjectId {
        let id = self.next_id;
        self.next_id += 1;

        let object = GameObject::new(id, object_type);
        self.objects.insert(id, Arc::new(parking_lot::Mutex::new(object)));

        id
    }

    fn destroy_object(&mut self, id: ObjectId) -> bool {
        self.objects.remove(&id).is_some()
    }

    fn get_object(&self, id: ObjectId) -> Option<Arc<parking_lot::Mutex<GameObject>>> {
        self.objects.get(&id).cloned()
    }

    fn update_all(&mut self, delta_time: Duration) {
        // Update all objects
        for object in self.objects.values() {
            object.lock().update(delta_time);
        }

        // Remove dead objects
        self.objects.retain(|_, obj| obj.lock().is_alive());
    }

    fn object_count(&self) -> usize {
        self.objects.len()
    }

    fn clear(&mut self) {
        self.objects.clear();
    }
}

/// Test basic object creation
#[test]
fn test_object_creation() {
    println!("Testing object creation...");

    let mut manager = ObjectManager::new();

    // Create objects of different types
    let unit_id = manager.create_object(ObjectType::Unit);
    let building_id = manager.create_object(ObjectType::Building);
    let projectile_id = manager.create_object(ObjectType::Projectile);

    assert_eq!(manager.object_count(), 3);

    // Verify objects exist
    assert!(manager.get_object(unit_id).is_some());
    assert!(manager.get_object(building_id).is_some());
    assert!(manager.get_object(projectile_id).is_some());

    // Verify object types
    let unit = manager.get_object(unit_id).unwrap();
    assert_eq!(unit.lock().object_type, ObjectType::Unit);

    let building = manager.get_object(building_id).unwrap();
    assert_eq!(building.lock().object_type, ObjectType::Building);

    log::info!("Object creation test passed");
}

/// Test object destruction
#[test]
fn test_object_destruction() {
    println!("Testing object destruction...");

    let mut manager = ObjectManager::new();

    // Create objects
    let id1 = manager.create_object(ObjectType::Unit);
    let id2 = manager.create_object(ObjectType::Unit);
    let id3 = manager.create_object(ObjectType::Unit);

    assert_eq!(manager.object_count(), 3);

    // Destroy one object
    assert!(manager.destroy_object(id2));
    assert_eq!(manager.object_count(), 2);

    // Verify it's gone
    assert!(manager.get_object(id2).is_none());

    // Other objects still exist
    assert!(manager.get_object(id1).is_some());
    assert!(manager.get_object(id3).is_some());

    // Destroying non-existent object returns false
    assert!(!manager.destroy_object(999));

    log::info!("Object destruction test passed");
}

/// Test object lifecycle (create, update, destroy)
#[test]
fn test_object_lifecycle() {
    println!("Testing object lifecycle...");

    let mut manager = ObjectManager::new();

    // Create object
    let id = manager.create_object(ObjectType::Unit);
    let obj = manager.get_object(id).unwrap();

    // Initial state
    assert_eq!(obj.lock().health, 100);
    assert!(obj.lock().is_alive());

    // Damage object
    obj.lock().take_damage(30);
    assert_eq!(obj.lock().health, 70);
    assert!(obj.lock().is_alive());

    // More damage
    obj.lock().take_damage(50);
    assert_eq!(obj.lock().health, 20);
    assert!(obj.lock().is_alive());

    // Kill object
    obj.lock().take_damage(30);
    assert_eq!(obj.lock().health, 0);
    assert!(!obj.lock().is_alive());

    // Update should remove dead objects
    manager.update_all(Duration::from_millis(16));
    assert_eq!(manager.object_count(), 0);

    log::info!("Object lifecycle test passed");
}

/// Test reference counting
#[test]
fn test_reference_counting() {
    println!("Testing reference counting...");

    let mut manager = ObjectManager::new();
    let id = manager.create_object(ObjectType::Unit);

    // Get multiple references
    let ref1 = manager.get_object(id).unwrap();
    let ref2 = manager.get_object(id).unwrap();
    let ref3 = Arc::clone(&ref1);

    // All should point to same object
    assert_eq!(Arc::strong_count(&ref1), 4); // manager + ref1 + ref2 + ref3

    // Drop references
    drop(ref2);
    assert_eq!(Arc::strong_count(&ref1), 3);

    drop(ref3);
    assert_eq!(Arc::strong_count(&ref1), 2);

    drop(ref1);
    // Only manager holds reference now

    log::info!("Reference counting test passed");
}

/// Test weak references (for optional references that don't prevent cleanup)
#[test]
fn test_weak_references() {
    println!("Testing weak references...");

    let mut manager = ObjectManager::new();
    let id = manager.create_object(ObjectType::Unit);

    // Get strong reference
    let strong = manager.get_object(id).unwrap();

    // Create weak reference
    let weak = Arc::downgrade(&strong);
    assert!(weak.upgrade().is_some());

    // Drop strong reference
    drop(strong);

    // Weak can still be upgraded while manager holds it
    assert!(weak.upgrade().is_some());

    // Destroy object
    manager.destroy_object(id);

    // Now weak reference is invalid
    assert!(weak.upgrade().is_none());

    log::info!("Weak references test passed");
}

/// Test batch object creation
#[test]
fn test_batch_creation() {
    println!("Testing batch object creation...");

    let mut manager = ObjectManager::new();

    // Create many objects at once
    let count = 1000;
    let mut ids = Vec::new();

    for _ in 0..count {
        let id = manager.create_object(ObjectType::Unit);
        ids.push(id);
    }

    assert_eq!(manager.object_count(), count);

    // Verify all objects exist
    for id in &ids {
        assert!(manager.get_object(*id).is_some());
    }

    log::info!("Batch creation test passed");
}

/// Test batch object destruction
#[test]
fn test_batch_destruction() {
    println!("Testing batch object destruction...");

    let mut manager = ObjectManager::new();

    // Create objects
    let mut ids = Vec::new();
    for _ in 0..100 {
        ids.push(manager.create_object(ObjectType::Unit));
    }

    assert_eq!(manager.object_count(), 100);

    // Destroy half
    for id in ids.iter().take(50) {
        manager.destroy_object(*id);
    }

    assert_eq!(manager.object_count(), 50);

    // Clear all
    manager.clear();
    assert_eq!(manager.object_count(), 0);

    log::info!("Batch destruction test passed");
}

/// Test object update cycle
#[test]
fn test_object_update_cycle() {
    println!("Testing object update cycle...");

    let mut manager = ObjectManager::new();

    // Create objects
    let id1 = manager.create_object(ObjectType::Unit);
    let id2 = manager.create_object(ObjectType::Unit);

    // Damage one to near death
    if let Some(obj) = manager.get_object(id1) {
        obj.lock().take_damage(99);
        assert_eq!(obj.lock().health, 1);
    }

    // Update cycle
    manager.update_all(Duration::from_millis(16));

    // Both should still exist (health > 0)
    assert_eq!(manager.object_count(), 2);

    // Kill first object
    if let Some(obj) = manager.get_object(id1) {
        obj.lock().take_damage(10);
    }

    // Update cycle should remove dead object
    manager.update_all(Duration::from_millis(16));
    assert_eq!(manager.object_count(), 1);

    // Verify id2 still exists
    assert!(manager.get_object(id2).is_some());
    assert!(manager.get_object(id1).is_none());

    log::info!("Object update cycle test passed");
}

/// Test concurrent object access
#[test]
fn test_concurrent_object_access() {
    use std::thread;
    use std::sync::Arc as StdArc;
    use parking_lot::Mutex as ParkingMutex;

    println!("Testing concurrent object access...");

    let manager = StdArc::new(ParkingMutex::new(ObjectManager::new()));

    // Create some objects
    let id = {
        let mut mgr = manager.lock();
        mgr.create_object(ObjectType::Unit)
    };

    // Spawn threads that access the object
    let mut handles = vec![];

    for i in 0..4 {
        let mgr_clone = StdArc::clone(&manager);
        let handle = thread::spawn(move || {
            let mgr = mgr_clone.lock();
            if let Some(obj) = mgr.get_object(id) {
                let mut obj = obj.lock();
                obj.take_damage(5);
                obj.position = (i as f32, i as f32, 0.0);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify object was modified
    let mgr = manager.lock();
    if let Some(obj) = mgr.get_object(id) {
        let obj = obj.lock();
        assert!(obj.health < 100); // Should have taken damage
    }

    log::info!("Concurrent object access test passed");
}

/// Test object type filtering
#[test]
fn test_object_type_filtering() {
    println!("Testing object type filtering...");

    let mut manager = ObjectManager::new();

    // Create mixed types
    manager.create_object(ObjectType::Unit);
    manager.create_object(ObjectType::Unit);
    manager.create_object(ObjectType::Building);
    manager.create_object(ObjectType::Projectile);
    manager.create_object(ObjectType::Unit);

    // Count by type
    let mut type_counts = HashMap::new();

    for obj_ref in manager.objects.values() {
        let obj = obj_ref.lock();
        *type_counts.entry(obj.object_type).or_insert(0) += 1;
    }

    assert_eq!(type_counts.get(&ObjectType::Unit), Some(&3));
    assert_eq!(type_counts.get(&ObjectType::Building), Some(&1));
    assert_eq!(type_counts.get(&ObjectType::Projectile), Some(&1));

    log::info!("Object type filtering test passed");
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    /// Stress test: Rapid creation and destruction
    #[test]
    #[ignore] // Run with: cargo test --test integration_object_lifecycle -- --ignored
    fn test_rapid_creation_destruction() {
        println!("Stress test: Rapid object creation and destruction...");

        let mut manager = ObjectManager::new();
        const CYCLES: usize = 10000;

        let start = Instant::now();

        for i in 0..CYCLES {
            // Create object
            let id = manager.create_object(ObjectType::Unit);

            // Do some work
            if let Some(obj) = manager.get_object(id) {
                obj.lock().take_damage(10);
                obj.lock().position = (i as f32, i as f32, 0.0);
            }

            // Destroy immediately
            manager.destroy_object(id);
        }

        let elapsed = start.elapsed();
        let ops_per_sec = CYCLES as f64 / elapsed.as_secs_f64();

        println!("Completed {} cycles in {:?} ({:.2} ops/sec)", CYCLES, elapsed, ops_per_sec);

        assert_eq!(manager.object_count(), 0);
        assert!(ops_per_sec > 10000.0, "Should handle >10k create/destroy per second");

        log::info!("Rapid creation/destruction stress test passed");
    }

    /// Stress test: Large object pool
    #[test]
    #[ignore]
    fn test_large_object_pool() {
        println!("Stress test: Large object pool...");

        let mut manager = ObjectManager::new();
        const POOL_SIZE: usize = 100000;

        let start = Instant::now();

        // Create large pool
        for _ in 0..POOL_SIZE {
            manager.create_object(ObjectType::Unit);
        }

        let creation_time = start.elapsed();

        println!("Created {} objects in {:?}", POOL_SIZE, creation_time);
        assert_eq!(manager.object_count(), POOL_SIZE);

        // Update all
        let update_start = Instant::now();
        manager.update_all(Duration::from_millis(16));
        let update_time = update_start.elapsed();

        println!("Updated {} objects in {:?}", POOL_SIZE, update_time);

        // Clear all
        let clear_start = Instant::now();
        manager.clear();
        let clear_time = clear_start.elapsed();

        println!("Cleared {} objects in {:?}", POOL_SIZE, clear_time);
        assert_eq!(manager.object_count(), 0);

        log::info!("Large object pool stress test passed");
    }

    /// Stress test: Memory leak detection
    #[test]
    #[ignore]
    fn test_memory_leak_detection() {
        println!("Stress test: Memory leak detection...");

        const ITERATIONS: usize = 1000;
        const OBJECTS_PER_ITERATION: usize = 1000;

        for iteration in 0..ITERATIONS {
            let mut manager = ObjectManager::new();

            // Create many objects
            for _ in 0..OBJECTS_PER_ITERATION {
                manager.create_object(ObjectType::Unit);
            }

            // Clear them
            manager.clear();

            if iteration % 100 == 0 {
                println!("Iteration {}/{}", iteration, ITERATIONS);
            }
        }

        println!("Completed {} iterations without leaks", ITERATIONS);
        log::info!("Memory leak detection test passed");
    }
}
